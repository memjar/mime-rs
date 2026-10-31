//! Decoded header values of every fixture against its frozen answers, field
//! by field, wherever this crate's default form and mail-parser's reading
//! are the same form: text, addresses, dates, identifiers and content
//! types. A difference counts unless tests/corpus/divergence.tsv holds a
//! row for the fixture and path.
//!
//! Only fixtures whose structure agrees part for part are compared.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

mod common;

use mime::{
    header::MimeHeaderKind,
    tree::{cst::MimeCst, header::MimeHeaderField},
    value::MimeHeaderValue,
};
use serde_json::Value;

use crate::common::{divergences, fixtures, frozen_shapes, line_endings, shapes};

const CORPORA: [(&str, usize); 4] = [
    ("dovecot-malformed", 24),
    ("dovecot-thirdparty", 20),
    ("stalwart-regressions", 75),
    ("stalwart-rfc", 10),
];

/// Our reading of a field, in the shape of mail-parser's JSON.
fn ours(field: &MimeHeaderField<'_>) -> Option<Value> {
    let kind: MimeHeaderKind = field.name_str().parse().ok()?;

    match field.read() {
        MimeHeaderValue::Text(text) if !matches!(kind, MimeHeaderKind::MimeVersion) => {
            Some(Value::from(text))
        }
        MimeHeaderValue::Addresses(list) => Some(Value::Array(
            list.mailboxes()
                .map(|mailbox| {
                    serde_json::json!({
                        "name": mailbox.name,
                        "address": mailbox.address,
                    })
                })
                .collect(),
        )),
        MimeHeaderValue::Date(date) => Some(serde_json::json!([
            date.year,
            date.month,
            date.day,
            date.hour,
            date.minute,
            date.second,
            date.offset,
        ])),
        MimeHeaderValue::MessageIds(ids) => Some(Value::from(ids)),
        MimeHeaderValue::ContentType(content_type) => Some(serde_json::json!({
            "type": content_type.top.to_ascii_lowercase(),
            "subtype": content_type.sub.to_ascii_lowercase(),
            "attributes": content_type
                .params
                .iter()
                .map(|param| (param.name.to_ascii_lowercase(), param.value.clone()))
                .collect::<Vec<_>>(),
        })),
        _ => None,
    }
}

/// mail-parser's reading of a field, reduced to the shape of [`ours`].
fn theirs(value: &Value) -> Option<Value> {
    if let Some(text) = value.get("text") {
        return Some(text.clone());
    }

    if let Some(list) = value.get("text_list").and_then(Value::as_array) {
        return Some(Value::Array(list.clone()));
    }

    if let Some(items) = value.get("address").and_then(Value::as_array) {
        let mailboxes = items
            .iter()
            .flat_map(|item| match &item["group"] {
                Value::Object(group) => group["mailboxes"].as_array().cloned().unwrap_or_default(),
                _ => vec![item["mailbox"].clone()],
            })
            .map(|mailbox| {
                serde_json::json!({
                    "name": mailbox.get("name").cloned().unwrap_or(Value::Null),
                    "address": mailbox.get("address").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();

        return Some(Value::Array(mailboxes));
    }

    if let Some(date) = value.get("date_time") {
        let minutes = date["tz_hour"].as_i64()? * 60 + date["tz_minute"].as_i64()?;
        let offset = if date["tz_before_gmt"].as_bool()? {
            -minutes
        } else {
            minutes
        };

        return Some(serde_json::json!([
            date["year"],
            date["month"],
            date["day"],
            date["hour"],
            date["minute"],
            date["second"],
            offset,
        ]));
    }

    if let Some(content_type) = value.get("content_type") {
        let attributes: Vec<(String, String)> = content_type["attributes"]
            .as_array()
            .map(|pairs| {
                pairs
                    .iter()
                    .filter_map(|pair| {
                        Some((
                            pair[0].as_str()?.to_ascii_lowercase(),
                            pair[1].as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();

        return Some(serde_json::json!({
            "type": content_type["type"].as_str()?.to_ascii_lowercase(),
            "subtype": content_type["subtype"].as_str().unwrap_or_default().to_ascii_lowercase(),
            "attributes": attributes,
        }));
    }

    None
}

#[test]
fn decoded_headers_agree_with_the_frozen_answers() {
    let divergences = divergences();
    let mut failures = Vec::new();
    let mut compared = 0;

    for (corpus, count) in CORPORA {
        for fixture in fixtures(corpus, count) {
            let (lf, crlf) = line_endings(&fixture.bytes);

            for (form, input, answer) in [("lf", &lf, &fixture.lf), ("crlf", &crlf, &fixture.crlf)]
            {
                let Some(answer) = answer else {
                    continue;
                };

                let cst = MimeCst::parse(input);

                if shapes(&cst) != frozen_shapes(answer) {
                    continue;
                }

                let parts = answer["parts"].as_array().expect("parts");

                for (index, (entity, part)) in cst.entities().iter().zip(parts).enumerate() {
                    let theirs_headers = part["headers"].as_array().cloned().unwrap_or_default();

                    for (field, header) in entity.header.fields().zip(&theirs_headers) {
                        let (Some(ours), Some(theirs)) = (ours(field), theirs(&header["value"]))
                        else {
                            continue;
                        };

                        compared += 1;

                        if ours == theirs {
                            continue;
                        }

                        let path = format!(
                            "{form}.part[{index}].{}",
                            field.name_str().to_ascii_lowercase()
                        );

                        if !divergences.contains(&(fixture.name.clone(), path.clone())) {
                            failures.push(format!(
                                "{} {path}: ours {ours}, theirs {theirs}",
                                fixture.name
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(compared > 1000, "only {compared} fields compared");
    assert!(
        failures.is_empty(),
        "{} header differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
