//! The validator: every rule with a message breaking it and one keeping
//! it, the proof only a valid message earns, the transport needs, and the
//! vendored corpus classified with asserted counts.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

mod common;

use mime::{
    header::MimeHeaderKind,
    tree::{anomaly::MimeAnomaly, cst::MimeCst},
    validator::{MimeSeverity, MimeValidateRule},
};

use crate::common::fixtures;

/// A conformant message to break one rule at a time.
const GOOD: &str = concat!(
    "Date: Sat, 31 Oct 2026 12:00:00 +0100\r\n",
    "From: Alice <alice@example.com>\r\n",
    "To: bob@example.com\r\n",
    "Message-ID: <1@example.com>\r\n",
    "Subject: Hi\r\n",
    "MIME-Version: 1.0\r\n",
    "Content-Type: text/plain; charset=utf-8\r\n",
    "\r\n",
    "Hello\r\n",
);

fn rules(message: &str) -> Vec<(MimeValidateRule, MimeSeverity)> {
    MimeCst::parse(message.as_bytes())
        .check()
        .problems
        .into_iter()
        .map(|problem| (problem.rule, problem.severity))
        .collect()
}

fn breaks(message: &str, rule: MimeValidateRule, severity: MimeSeverity) {
    let found = rules(message);
    assert!(
        found.contains(&(rule.clone(), severity)),
        "{rule:?} not in {found:?}"
    );
}

#[test]
fn a_good_message_breaks_nothing_and_earns_its_proof() {
    assert_eq!(rules(GOOD), []);

    let valid = MimeCst::parse(GOOD.as_bytes()).validate().unwrap();
    assert!(valid.output().problems.is_empty());
    assert_eq!(valid.into_inner().to_bytes(), GOOD.as_bytes());

    let valid = MimeCst::parse(GOOD.as_bytes()).decode().validate().unwrap();
    assert_eq!(valid.headers.len(), 7);
}

#[test]
fn a_missing_message_id_is_an_advisory_and_keeps_the_proof() {
    let message = GOOD.replace("Message-ID: <1@example.com>\r\n", "");
    breaks(
        &message,
        MimeValidateRule::Missing(MimeHeaderKind::MessageId),
        MimeSeverity::Advisory,
    );

    let valid = MimeCst::parse(message.as_bytes()).validate().unwrap();
    assert_eq!(valid.output().problems.len(), 1);
    assert_eq!(valid.output().problems[0].rule.rfc(), "RFC 5322 3.6.4");
}

#[test]
fn the_message_fields() {
    use MimeSeverity::{Advisory, Violation};

    breaks(
        &GOOD.replace("Date: Sat, 31 Oct 2026 12:00:00 +0100\r\n", ""),
        MimeValidateRule::Missing(MimeHeaderKind::Date),
        Violation,
    );
    breaks(
        &GOOD.replace("Subject: Hi\r\n", "Subject: Hi\r\nSubject: Again\r\n"),
        MimeValidateRule::Repeated(MimeHeaderKind::Subject),
        Violation,
    );
    breaks(
        &GOOD.replace(
            "From: Alice <alice@example.com>",
            "From: a@x.test, b@x.test",
        ),
        MimeValidateRule::SenderRequired,
        Violation,
    );
    assert!(
        !rules(&GOOD.replace(
            "From: Alice <alice@example.com>",
            "From: a@x.test, b@x.test\r\nSender: a@x.test"
        ))
        .iter()
        .any(|(rule, _)| *rule == MimeValidateRule::SenderRequired)
    );
    breaks(
        &GOOD.replace("Subject: Hi\r\n", "Subject: Hi\r\nResent-To: c@x.test\r\n"),
        MimeValidateRule::ResentIncomplete,
        Violation,
    );
    breaks(
        &GOOD.replace(
            "Subject: Hi\r\n",
            "Subject: Hi\r\nReceived: from x by y; Sat, 31 Oct 2026 12:00:00 +0100\r\n",
        ),
        MimeValidateRule::TraceOrder,
        Advisory,
    );
    assert_eq!(
        rules(&format!(
            "Received: from x by y; Sat, 31 Oct 2026 12:00:00 +0100\r\n{GOOD}"
        )),
        []
    );
}

#[test]
fn the_field_grammar() {
    use MimeSeverity::{Advisory, Violation};

    breaks(
        &GOOD.replace("Sat, 31 Oct 2026 12:00:00 +0100", "yesterday at noon"),
        MimeValidateRule::Unreadable(mime::header::spec::MimeHeaderForm::Date),
        Violation,
    );
    breaks(
        &GOOD.replace(
            "Sat, 31 Oct 2026 12:00:00 +0100",
            "Sat, 31 Oct 26 12:00:00 EST",
        ),
        MimeValidateRule::Obsolete(mime::header::spec::MimeHeaderForm::Date),
        Advisory,
    );
    breaks(
        &GOOD.replace("bob@example.com", "bob"),
        MimeValidateRule::Address,
        Violation,
    );

    for address in ["@example.com", "bob@", "bob@\"example.com\""] {
        breaks(
            &GOOD.replace("bob@example.com", address),
            MimeValidateRule::Address,
            Violation,
        );
    }

    for date in [
        "Sat, 31 Oct 2026 12:0:00 +0100",
        "Sat, 31 Oct 2026 12:00:00 +01",
        "Sat, 31 Oct 2026 12:00:00 +0199",
        "Sat, 31 Oct 2026 12:00:00 im-8b",
        "Sat, 31 Oct 2026 12:00:00",
        "Sat 31 Oct 2026 12:00:00 +0100",
        "Sat, 31 Oct 0001 12:00:00 +0100",
    ] {
        breaks(
            &GOOD.replace("Sat, 31 Oct 2026 12:00:00 +0100", date),
            MimeValidateRule::Unreadable(mime::header::spec::MimeHeaderForm::Date),
            Violation,
        );
    }

    // NOTE: a year past any integer still reads without overflow.
    breaks(
        &GOOD.replace("2026", "99999999999999999999999"),
        MimeValidateRule::Unreadable(mime::header::spec::MimeHeaderForm::Date),
        Violation,
    );

    for date in [
        "Sat, 31 Oct 2026 12:00:00 +0100 (CET)",
        "31 Oct 2026 12:00 -0000",
    ] {
        assert_eq!(
            rules(&GOOD.replace("Sat, 31 Oct 2026 12:00:00 +0100", date)),
            [],
            "{date}"
        );
    }
    breaks(
        &GOOD.replace("<1@example.com>", "<no-domain>"),
        MimeValidateRule::MessageId,
        Violation,
    );
    breaks(
        &GOOD.replace("Subject: Hi", &format!("Subject: {}", "x".repeat(1000))),
        MimeValidateRule::LineTooLong,
        Violation,
    );
    breaks(
        &GOOD.replace("Subject: Hi", &format!("Subject: {}", "word ".repeat(20))),
        MimeValidateRule::LineLong,
        Advisory,
    );
    breaks(
        &GOOD.replace("Subject: Hi", "Subject: H\ri"),
        MimeValidateRule::StrayControl,
        Violation,
    );
    breaks(
        &GOOD.replace(
            "Alice <alice@example.com>",
            "\"=?utf-8?q?Alice?=\" <alice@example.com>",
        ),
        MimeValidateRule::EncodedWord,
        Violation,
    );
    breaks(
        &GOOD.replace("charset=utf-8", "name=\"=?utf-8?q?a.txt?=\""),
        MimeValidateRule::EncodedWord,
        Violation,
    );
    breaks(
        &GOOD.replace("\r\n", "\n"),
        MimeValidateRule::LfLineEndings,
        Advisory,
    );
}

#[test]
fn eight_bit_fields_need_utf8_or_are_violations() {
    let utf8 = GOOD.replace("Subject: Hi", "Subject: Grüße");
    let output = MimeCst::parse(utf8.as_bytes()).check();
    assert!(output.is_valid());
    assert!(output.transport.utf8_fields);

    let mut latin1 = GOOD.replace("Subject: Hi", "Subject: Gr").into_bytes();
    let at = latin1.windows(4).position(|w| w == b": Gr").unwrap() + 4;
    latin1.insert(at, 0xfc);
    let output = MimeCst::parse(&latin1).check();
    assert!(
        output
            .violations()
            .any(|problem| problem.rule == MimeValidateRule::Raw8bit)
    );
}

#[test]
fn the_mime_structure() {
    use MimeSeverity::Violation;

    breaks(
        &GOOD.replace("MIME-Version: 1.0\r\n", ""),
        MimeValidateRule::MimeVersion,
        Violation,
    );

    let multipart = |content_type: &str, encoding: &str| {
        GOOD.replace(
            "Content-Type: text/plain; charset=utf-8\r\n\r\nHello\r\n",
            &format!(
                "Content-Type: {content_type}\r\n{encoding}\r\n--b\r\n\r\none\r\n--b\r\n\r\ntwo\r\n--b--\r\n"
            ),
        )
    };

    assert_eq!(rules(&multipart("multipart/mixed; boundary=b", "")), []);
    breaks(
        &multipart("multipart/mixed", ""),
        MimeValidateRule::Boundary,
        Violation,
    );
    breaks(
        &multipart("multipart/mixed; boundary=\"b \"", ""),
        MimeValidateRule::Boundary,
        Violation,
    );
    breaks(
        &multipart(
            "multipart/mixed; boundary=b",
            "Content-Transfer-Encoding: base64\r\n",
        ),
        MimeValidateRule::CompositeEncoding,
        Violation,
    );
    breaks(
        &multipart("multipart/related; boundary=b", ""),
        MimeValidateRule::RelatedType,
        Violation,
    );
    assert_eq!(
        rules(&multipart(
            "multipart/related; boundary=b; type=text/plain",
            ""
        )),
        []
    );
    breaks(
        &multipart("multipart/signed; boundary=b", ""),
        MimeValidateRule::Signed,
        Violation,
    );
    assert_eq!(
        rules(&multipart(
            "multipart/signed; boundary=b;\r\n protocol=\"application/pgp-signature\"; micalg=pgp-sha256",
            ""
        )),
        []
    );
    breaks(
        &multipart("multipart/encrypted; boundary=b", ""),
        MimeValidateRule::Encrypted,
        Violation,
    );
    breaks(
        &multipart("multipart/report; boundary=b", ""),
        MimeValidateRule::ReportType,
        Violation,
    );

    breaks(
        &GOOD.replace("Hello", "Héllo"),
        MimeValidateRule::NotSevenBit,
        Violation,
    );
    breaks(
        &GOOD
            .replace(
                "charset=utf-8\r\n",
                "charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n",
            )
            .replace("Hello", "a\0b"),
        MimeValidateRule::NotEightBit,
        Violation,
    );
    breaks(
        &GOOD
            .replace(
                "charset=utf-8\r\n",
                "charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n",
            )
            .replace("Hello", "bad =ZZ sequence"),
        MimeValidateRule::QuotedPrintable,
        Violation,
    );
    breaks(
        &GOOD
            .replace(
                "charset=utf-8\r\n",
                "charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n",
            )
            .replace("Hello", "aGVs*bG8="),
        MimeValidateRule::Base64,
        Violation,
    );
    breaks(
        &GOOD
            .replace(
                "Content-Type: text/plain; charset=utf-8",
                "Content-Type: message/partial; id=x; number=1",
            )
            .replace(
                "MIME-Version: 1.0\r\n",
                "MIME-Version: 1.0\r\nContent-Transfer-Encoding: 8bit\r\n",
            ),
        MimeValidateRule::PartialEncoding,
        Violation,
    );

    let ids = GOOD.replace(
        "Content-Type: text/plain; charset=utf-8\r\n\r\nHello\r\n",
        "Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-ID: <x@y>\r\n\r\none\r\n--b\r\nContent-ID: <x@y>\r\n\r\ntwo\r\n--b--\r\n",
    );
    breaks(&ids, MimeValidateRule::ContentIdRepeated, Violation);
}

#[test]
fn a_base64_message_rfc822_breaks_the_composite_rule_and_still_reads() {
    let message = GOOD.replace(
        "Content-Type: text/plain; charset=utf-8\r\n\r\nHello\r\n",
        "Content-Type: message/rfc822\r\nContent-Transfer-Encoding: base64\r\n\r\nU3ViamVjdDogaW5uZXINCg0KYm9keQ==\r\n",
    );

    breaks(
        &message,
        MimeValidateRule::CompositeEncoding,
        MimeSeverity::Violation,
    );
    assert_eq!(rules(&message)[0].0.rfc(), "RFC 2045 6.4");

    let cst = MimeCst::parse(message.as_bytes());
    let nested = cst.root.nested().unwrap();
    assert_eq!(
        nested
            .root
            .header::<mime::header::marker::SUBJECT>()
            .as_deref(),
        Some("inner")
    );
}

#[test]
fn nested_messages_get_advisories_and_anomalies_violations() {
    let message = GOOD.replace(
        "Content-Type: text/plain; charset=utf-8\r\n\r\nHello\r\n",
        "Content-Type: message/rfc822\r\n\r\nSubject: inner\r\n\r\nbody\r\n",
    );

    let found = rules(&message);
    assert!(found.contains(&(
        MimeValidateRule::Missing(MimeHeaderKind::Date),
        MimeSeverity::Advisory
    )));
    assert!(MimeCst::parse(message.as_bytes()).validate().is_ok());

    breaks(
        &GOOD.replace("\r\n\r\nHello\r\n", "\r\nHello\r\n"),
        MimeValidateRule::Anomaly(MimeAnomaly::NoBlankLine),
        MimeSeverity::Violation,
    );
}

#[test]
fn the_transport_needs_are_computed() {
    let eight = GOOD
        .replace(
            "charset=utf-8\r\n",
            "charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n",
        )
        .replace("Hello", "Héllo");
    let output = MimeCst::parse(eight.as_bytes()).check();
    assert!(output.is_valid());
    assert!(output.transport.eight_bit && !output.transport.binary);

    let binary = GOOD
        .replace(
            "charset=utf-8\r\n",
            "charset=utf-8\r\nContent-Transfer-Encoding: binary\r\n",
        )
        .replace("Hello", "a\0b\rc");
    let output = MimeCst::parse(binary.as_bytes()).check();
    assert!(output.is_valid());
    assert!(output.transport.binary);
}

#[test]
fn the_corpus_is_classified() {
    let expected = [
        ("dovecot-malformed", 24, (0, 1, 23)),
        ("dovecot-thirdparty", 20, (0, 3, 17)),
        ("stalwart-issues", 18, (0, 1, 17)),
        ("stalwart-regressions", 75, (0, 0, 75)),
        ("stalwart-rfc", 10, (0, 2, 8)),
        ("stalwart-sieve", 6, (0, 2, 4)),
    ];

    for (corpus, count, counts) in expected {
        let mut got = (0, 0, 0);

        for fixture in fixtures(corpus, count) {
            let output = MimeCst::parse(&fixture.bytes).check();

            if !output.is_valid() {
                got.2 += 1;
            } else if output.problems.is_empty() {
                got.0 += 1;
            } else {
                got.1 += 1;
            }
        }

        assert_eq!(got, counts, "{corpus}: valid, advisories only, violations");
    }
}

#[test]
fn from_names_a_mailbox() {
    for from in ["From: <>", "From:", "From: undisclosed:;"] {
        breaks(
            &GOOD.replace("From: Alice <alice@example.com>", from),
            MimeValidateRule::Address,
            MimeSeverity::Violation,
        );
    }

    let path = format!("Return-Path: <>\r\n{GOOD}");
    assert_eq!(rules(&path), []);
}

#[test]
fn long_lines_under_binary_are_a_transport_need() {
    let message = GOOD
        .replace(
            "charset=utf-8\r\n",
            "charset=utf-8\r\nContent-Transfer-Encoding: binary\r\n",
        )
        .replace("Hello", &"x".repeat(2000));
    let output = MimeCst::parse(message.as_bytes()).check();

    assert!(output.is_valid(), "{:?}", output.problems);
    assert!(output.transport.long_lines);
}
