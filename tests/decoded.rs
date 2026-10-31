//! Decoded headers, content, decode problems and body lists of every
//! fixture against its frozen answers, unless tests/corpus/divergence.tsv
//! holds a row for the fixture and path.
//!
//! Only fixtures whose structure agrees part for part are compared here;
//! the structure sweep (tests/corpus.rs) accounts for the others.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

mod common;

use mime::{
    codec::base64,
    header::marker::{DATE, FROM, SUBJECT},
    tree::{cst::MimeCst, entity::MimeEntity},
};
use serde_json::Value;

use crate::common::{divergences, fixtures, frozen_shapes, line_endings, shapes};

const CORPORA: [(&str, usize); 4] = [
    ("dovecot-malformed", 24),
    ("dovecot-thirdparty", 20),
    ("stalwart-regressions", 75),
    ("stalwart-rfc", 10),
];

fn index_of(entities: &[&MimeEntity<'_>], part: &MimeEntity<'_>) -> usize {
    entities
        .iter()
        .position(|entity| std::ptr::eq(*entity, part))
        .expect("an entity of the tree")
}

fn header<'v>(part: &'v Value, name: &str) -> Option<&'v Value> {
    part["headers"]
        .as_array()?
        .iter()
        .find(|header| {
            header["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .map(|header| &header["value"])
}

fn compare(
    name: &str,
    form: &str,
    input: &[u8],
    answer: &Value,
    failures: &mut Vec<(String, String, String)>,
) {
    let cst = MimeCst::parse(input);
    let entities = cst.entities();
    let parts = answer["parts"].as_array().expect("parts");
    let mut fail = |path: String, detail: String| failures.push((name.to_string(), path, detail));

    for (index, (ours, theirs)) in entities.iter().zip(parts).enumerate() {
        if let Some(Value::Object(subject)) = header(theirs, "subject")
            && let Some(expected) = subject.get("text").and_then(Value::as_str)
        {
            let got = ours.header::<SUBJECT>();

            if got.as_deref() != Some(expected) {
                fail(
                    format!("{form}.part[{index}].subject"),
                    format!("ours {got:?}, theirs {expected:?}"),
                );
            }
        }

        if let Some(Value::Object(date)) = header(theirs, "date")
            && let Some(expected) = date.get("date_time")
        {
            let got = ours.header::<DATE>();
            let same = got.is_some_and(|date| {
                date.year as u64 == expected["year"].as_u64().unwrap_or_default()
                    && date.month as u64 == expected["month"].as_u64().unwrap_or_default()
                    && date.day as u64 == expected["day"].as_u64().unwrap_or_default()
                    && date.hour as u64 == expected["hour"].as_u64().unwrap_or_default()
                    && date.minute as u64 == expected["minute"].as_u64().unwrap_or_default()
            });

            if !same {
                fail(
                    format!("{form}.part[{index}].date"),
                    format!("ours {got:?}, theirs {expected}"),
                );
            }
        }

        if let Some(Value::Object(from)) = header(theirs, "from")
            && let Some(Value::Array(expected)) = from.get("address")
        {
            let theirs: Vec<Option<&str>> = expected
                .iter()
                .flat_map(|item| match &item["group"] {
                    Value::Object(group) => {
                        group["mailboxes"].as_array().cloned().unwrap_or_default()
                    }
                    _ => vec![item["mailbox"].clone()],
                })
                .map(|mailbox| mailbox["address"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
                .iter()
                .map(|address| address.as_deref().map(|_| ""))
                .collect();
            let got: Vec<Option<String>> = ours
                .header::<FROM>()
                .map(|list| {
                    list.mailboxes()
                        .map(|mailbox| mailbox.address.clone())
                        .collect()
                })
                .unwrap_or_default();

            if got.len() != theirs.len() {
                fail(
                    format!("{form}.part[{index}].from"),
                    format!("ours {got:?}, theirs {expected:?}"),
                );
            }
        }

        if let Some(expected) = theirs["text"].as_str() {
            let got = ours.text();

            if got.as_deref() != Some(expected) {
                fail(
                    format!("{form}.part[{index}].text"),
                    format!("ours {got:?}, theirs {expected:?}"),
                );
            }
        } else if let Some(expected) = theirs["binary"].as_str() {
            let mut got = Vec::new();
            base64::encode_b(&ours.decoded().unwrap_or_default(), &mut got);

            if got != expected.as_bytes() {
                fail(
                    format!("{form}.part[{index}].binary"),
                    format!("ours {}, theirs {expected}", String::from_utf8_lossy(&got)),
                );
            }
        }

        if let Some(expected) = theirs["decode_problems"].as_array() {
            let mut theirs: Vec<&str> = expected.iter().filter_map(Value::as_str).collect();
            theirs.sort_unstable();

            let problems = if theirs.iter().any(|problem| problem.ends_with("charset"))
                || ours.text().is_some()
            {
                ours.text_checked().map(|(_, problems)| problems)
            } else {
                ours.decoded_checked().map(|(_, problems)| problems)
            }
            .unwrap_or_default();

            let mut got = Vec::new();

            if problems.malformed_charset {
                got.push("malformed_charset");
            }

            if problems.malformed_encoding {
                got.push("malformed_transfer_encoding");
            }

            if problems.unknown_charset {
                got.push("unknown_charset");
            }

            if problems.unknown_encoding {
                got.push("unknown_transfer_encoding");
            }

            got.sort_unstable();

            // NOTE: mail-parser checks the charset of text parts only.
            let kind = theirs_kind(&parts[index]);
            if kind != "text" && kind != "html" {
                got.retain(|problem| !problem.ends_with("charset"));
            }

            if got != theirs {
                fail(
                    format!("{form}.part[{index}].problems"),
                    format!("ours {got:?}, theirs {theirs:?}"),
                );
            }
        }
    }

    let lists = cst.root.body_lists();
    let message = &answer["messages"][0];

    for (list, ours) in [
        ("text_body", &lists.text),
        ("html_body", &lists.html),
        ("attachments", &lists.attachments),
    ] {
        let got: Vec<u64> = ours
            .iter()
            .map(|part| index_of(&entities, part) as u64)
            .collect();
        let theirs: Vec<u64> = message[list]
            .as_array()
            .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();

        if got != theirs {
            fail(
                format!("{form}.{list}"),
                format!("ours {got:?}, theirs {theirs:?}"),
            );
        }
    }

    if let Some(expected) = message["has_attachments"].as_bool()
        && lists.has_attachment() != expected
    {
        fail(
            format!("{form}.has_attachments"),
            format!("ours {}, theirs {expected}", lists.has_attachment()),
        );
    }
}

fn theirs_kind(part: &Value) -> &str {
    part["kind"].as_str().unwrap_or_default()
}

#[test]
fn decoded_content_agrees_with_the_frozen_answers() {
    let divergences = divergences();
    let mut failures = Vec::new();

    for (corpus, count) in CORPORA {
        for fixture in fixtures(corpus, count) {
            let (lf, crlf) = line_endings(&fixture.bytes);

            for (form, input, answer) in [("lf", &lf, &fixture.lf), ("crlf", &crlf, &fixture.crlf)]
            {
                let Some(answer) = answer else {
                    continue;
                };

                if shapes(&MimeCst::parse(input)) != frozen_shapes(answer) {
                    continue;
                }

                compare(&fixture.name, form, input, answer, &mut failures);
            }
        }
    }

    let failures: Vec<String> = failures
        .into_iter()
        .filter(|(name, path, _)| !divergences.contains(&(name.clone(), path.clone())))
        .map(|(name, path, detail)| format!("{name} {path}: {detail}"))
        .collect();

    assert!(
        failures.is_empty(),
        "{} decoded differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_text_prefix_reads_only_what_it_needs() {
    let line = "Caf=C3=A9 au lait, une ligne assez longue pour remplir la page.\r\n";
    let message = format!(
        "Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n{}",
        line.repeat(2000)
    );
    let cst = MimeCst::parse(message.as_bytes());

    assert_eq!(cst.root.text_prefix(8).as_deref(), Some("Café au "));
    assert_eq!(
        cst.root
            .text_prefix(1_000_000)
            .map(|text| text.chars().count()),
        cst.root.text().map(|text| text.chars().count())
    );
}

#[test]
fn a_folded_value_unfolds_keeping_its_blanks() {
    let cst = MimeCst::parse(b"Subject: a\r\n  b\n\tc\r\n\r\n");
    let field = cst.root.header.get("subject").unwrap();
    assert_eq!(field.unfolded().as_ref(), b" a  b\tc");
}
