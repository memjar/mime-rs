//! The builder and the writer: what they refuse, how they encode and fold
//! header fields, how they choose transfer encodings and boundaries, and
//! that what they write reads back as written, through this crate and
//! through mail-parser 1.0.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

use mime::{
    build::{MimeBuildError, MimeBuildOptions, MimeLineEnding},
    builder::{MimeAttachment, MimeBuilder},
    header::{
        MimeHeader, MimeHeaderKind, MimeHeaderName,
        marker::{DATE, FROM, MESSAGE_ID, SUBJECT, TO},
    },
    mime::{Mime, MimeBody},
    tree::cst::MimeCst,
    value::{
        MimeHeaderValue,
        address::{MimeAddressList, MimeMailbox},
        content_type::MimeContentType,
        datetime::MimeDateTime,
        message_id::MimeMessageId,
    },
};

fn date() -> MimeDateTime {
    MimeDateTime {
        year: 2026,
        month: 10,
        day: 31,
        hour: 12,
        minute: 0,
        second: 0,
        offset: 60,
    }
}

fn draft() -> MimeBuilder {
    let mut builder = MimeBuilder::default();
    builder
        .header::<DATE>(date())
        .unwrap()
        .header::<FROM>(MimeMailbox::named("Alice", "alice@example.com").into())
        .unwrap()
        .header::<TO>(MimeMailbox::address("bob@example.com").into())
        .unwrap()
        .header::<MESSAGE_ID>(vec![MimeMessageId::new("1", "example.com").unwrap().into()])
        .unwrap();
    builder
}

fn bytes(builder: MimeBuilder) -> Vec<u8> {
    builder.write(&MimeBuildOptions::default()).unwrap()
}

/// Every line of `bytes`, line endings removed, asserting they are CRLF.
fn lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines: Vec<&[u8]> = bytes.split(|byte| *byte == b'\n').collect();
    assert_eq!(lines.pop(), Some(&b""[..]), "ends with a line ending");

    lines
        .into_iter()
        .map(|line| line.strip_suffix(b"\r").expect("CRLF line ending"))
        .collect()
}

/// Asserts the RFC 5322 2.1.1 and 2.2.3 folding rules on a header block.
fn assert_folded(header: &[u8]) {
    for line in lines(header) {
        assert!(
            line.len() <= 76,
            "long line {:?}",
            String::from_utf8_lossy(line)
        );

        if line
            .first()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
        {
            assert!(
                line.iter().any(|byte| !matches!(byte, b' ' | b'\t')),
                "blank continuation"
            );
        }

        assert!(
            !line.ends_with(b" ") && !line.ends_with(b"\t"),
            "blank before a fold {:?}",
            String::from_utf8_lossy(line)
        );
    }
}

fn header_block(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("header block end");
    &bytes[..end + 2]
}

#[test]
fn a_date_given_as_text_is_refused() {
    let mut builder = MimeBuilder::default();
    let error = builder
        .header_value(MimeHeaderKind::Date, MimeHeaderValue::Text("today".into()))
        .unwrap_err();

    assert!(matches!(
        error,
        MimeBuildError::Form {
            kind: MimeHeaderKind::Date,
            ..
        }
    ));
}

#[test]
fn fields_the_spec_allows_once_and_fields_the_builder_writes_are_refused() {
    let mut builder = draft();

    assert_eq!(
        builder.header::<DATE>(date()).unwrap_err(),
        MimeBuildError::Repeated(MimeHeaderKind::Date)
    );
    assert_eq!(
        builder
            .header_value(
                MimeHeaderKind::MimeVersion,
                MimeHeaderValue::Text("1.0".into())
            )
            .unwrap_err(),
        MimeBuildError::Reserved(MimeHeaderKind::MimeVersion)
    );
    assert!(matches!(
        builder.header::<MESSAGE_ID>(vec!["no domain".into()]),
        Err(MimeBuildError::Repeated(_))
    ));
    assert!(matches!(
        MimeBuilder::default().header::<MESSAGE_ID>(vec!["no domain".into()]),
        Err(MimeBuildError::MessageId(_))
    ));
    assert!(matches!(
        builder.raw_header("X-Smuggled", b" a\r\nBcc: eve@example.com"),
        Err(MimeBuildError::RawValue(_))
    ));
    assert!(matches!(
        builder.raw_header("Bad Name", b" a"),
        Err(MimeBuildError::FieldName(_))
    ));
}

#[test]
fn a_display_name_with_a_comma_and_an_accent_reads_back() {
    let mut builder = draft();
    builder
        .header::<SUBJECT>("Hi".into())
        .unwrap()
        .header::<mime::header::marker::CC>(
            MimeMailbox::named("Doe, Jöhn", "john@example.com").into(),
        )
        .unwrap()
        .text("hello");

    let bytes = bytes(builder);
    let cst = MimeCst::parse(&bytes);
    let cc = cst.root.header::<mime::header::marker::CC>().unwrap();

    assert_eq!(
        cc.mailboxes().collect::<Vec<_>>(),
        [&MimeMailbox::named("Doe, Jöhn", "john@example.com")]
    );

    let theirs = mail_parser::MessageParser::new().parse(&bytes).unwrap();
    let theirs = theirs.cc().unwrap().first().unwrap();
    assert_eq!(theirs.name(), Some("Doe, Jöhn"));
    assert_eq!(theirs.address(), Some("john@example.com"));
}

#[test]
fn a_long_subject_folds_by_the_rules_and_reads_back() {
    let words = [
        "naïve",
        "café",
        "plain",
        "日本語",
        "a",
        "longerwordhere",
        "ünïcödé",
    ];
    let subject: String = (0..45)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ");

    for name in [
        "Subject",
        "X-A-Much-Longer-Field-Name-For-Another-Starting-Column",
    ] {
        let mut builder = draft();
        builder.raw_header("X-Pad", b" pad").unwrap();
        builder.text("body");

        let mut mime = builder.build();
        mime.headers.push(MimeHeader {
            name: MimeHeaderName::new(name),
            value: MimeHeaderValue::Text(subject.clone()),
        });

        let bytes = mime.to_bytes(&MimeBuildOptions::default()).unwrap();
        assert_folded(header_block(&bytes));

        let cst = MimeCst::parse(&bytes);
        let field = cst.root.header.get(name).unwrap();
        assert_eq!(
            field.read_as(mime::header::spec::MimeHeaderForm::Text),
            MimeHeaderValue::Text(subject.clone())
        );

        if name == "Subject" {
            let theirs = mail_parser::MessageParser::new().parse(&bytes).unwrap();
            assert_eq!(theirs.subject(), Some(subject.as_str()));
        }
    }
}

#[test]
fn a_forwarded_message_built_by_the_same_builder_keeps_its_parts() {
    let mut inner = draft();
    inner
        .header::<SUBJECT>("Inner".into())
        .unwrap()
        .text("inner text")
        .html("<p>inner</p>");
    let inner = inner.build();

    let mut outer = draft();
    outer.header::<SUBJECT>("Fwd: Inner".into()).unwrap();
    outer.text("see attached");
    outer.part(Mime {
        headers: vec![MimeHeader {
            name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
            value: MimeHeaderValue::ContentType(MimeContentType {
                top: "message".into(),
                sub: "rfc822".into(),
                params: vec![],
            }),
        }],
        body: MimeBody::Message(Box::new(inner)),
    });

    let bytes = bytes(outer);
    let cst = MimeCst::parse(&bytes);
    let root = cst.root.content_type();
    let outer_boundary = root.param("boundary").unwrap();

    let entities = cst.entities();
    let nested = entities
        .iter()
        .find(|entity| entity.content_type().is("multipart", "alternative"))
        .unwrap();
    let inner_boundary = nested.content_type().param("boundary").unwrap();

    assert_ne!(outer_boundary, inner_boundary);
    assert!(cst.anomalies().is_empty(), "{:?}", cst.anomalies());
    assert_eq!(entities.len(), 6);

    let theirs = mail_parser::MessageParser::new().parse(&bytes).unwrap();
    assert_eq!(theirs.parts().len(), entities.len());
    let attached = theirs.attachments().next().unwrap().nested().unwrap();
    assert_eq!(attached.body_text(0).as_deref(), Some("inner text"));
}

#[test]
fn building_is_deterministic() {
    let make = || {
        let mut builder = draft();
        builder
            .header::<SUBJECT>("Same".into())
            .unwrap()
            .text("t")
            .html("<b>h</b>")
            .attachment(MimeAttachment {
                content_type: MimeContentType {
                    top: "application".into(),
                    sub: "pdf".into(),
                    params: vec![],
                },
                filename: Some("ré.pdf".into()),
                content: vec![0, 1, 2, 255],
                ..Default::default()
            })
            .unwrap();
        bytes(builder)
    };

    assert_eq!(make(), make());
}

#[test]
fn transfer_encodings_follow_the_content_and_the_options() {
    let encoding_of = |text: &str, opts: &MimeBuildOptions| {
        let mut builder = draft();
        builder.text(text);
        let bytes = builder.write(opts).unwrap();
        let cst = MimeCst::parse(&bytes);
        assert_eq!(
            cst.root.text().as_deref(),
            Some(normalised(text, opts).as_str())
        );
        cst.root.transfer_encoding().to_string()
    };

    let strict = MimeBuildOptions::default();
    let eight = MimeBuildOptions {
        allow_8bit: true,
        ..Default::default()
    };

    assert_eq!(encoding_of("plain ascii\nlines\n", &strict), "7bit");
    assert_eq!(encoding_of("café crème\n", &strict), "quoted-printable");
    assert_eq!(encoding_of("café crème\n", &eight), "8bit");
    assert_eq!(
        encoding_of("日本語のテキストです。日本語のテキストです。\n", &strict),
        "base64"
    );
    assert_eq!(encoding_of(&"x".repeat(1200), &strict), "quoted-printable");
    assert_eq!(encoding_of("bare\rcr", &eight), "quoted-printable");
}

fn normalised(text: &str, opts: &MimeBuildOptions) -> String {
    let eol = match opts.line_ending {
        MimeLineEnding::Crlf => "\r\n",
        MimeLineEnding::Lf => "\n",
    };

    text.replace("\r\n", "\n").replace('\n', eol)
}

#[test]
fn binary_content_is_never_altered() {
    let content: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
    let mut builder = draft();
    builder
        .text("see attached")
        .attachment(MimeAttachment {
            content_type: MimeContentType {
                top: "application".into(),
                sub: "octet-stream".into(),
                params: vec![],
            },
            filename: Some("blob.bin".into()),
            content: content.clone(),
            ..Default::default()
        })
        .unwrap();

    for opts in [
        MimeBuildOptions::default(),
        MimeBuildOptions {
            allow_binary: true,
            ..Default::default()
        },
    ] {
        let bytes = draft_with(&content, &opts);
        let cst = MimeCst::parse(&bytes);
        let lists = cst.root.body_lists();
        assert_eq!(lists.attachments.len(), 1);
        assert_eq!(
            lists.attachments[0].decoded().unwrap().as_ref(),
            content.as_slice()
        );
    }
}

fn draft_with(content: &[u8], opts: &MimeBuildOptions) -> Vec<u8> {
    let mut builder = draft();
    builder
        .text("see attached")
        .attachment(MimeAttachment {
            content_type: MimeContentType {
                top: "application".into(),
                sub: "octet-stream".into(),
                params: vec![],
            },
            filename: Some("blob.bin".into()),
            content: content.to_vec(),
            ..Default::default()
        })
        .unwrap();
    builder.write(opts).unwrap()
}

#[test]
fn a_long_non_ascii_filename_goes_by_rfc_2231_continuations() {
    let filename = "Rapport annuel détaillé de l'exercice 2026, version définitive (relue).pdf";
    let mut builder = draft();
    builder
        .text("t")
        .attachment(MimeAttachment {
            content_type: MimeContentType {
                top: "application".into(),
                sub: "pdf".into(),
                params: vec![],
            },
            filename: Some(filename.into()),
            content: b"%PDF".to_vec(),
            ..Default::default()
        })
        .unwrap();

    let opts = MimeBuildOptions {
        legacy_name: true,
        ..Default::default()
    };
    let bytes = builder.write(&opts).unwrap();
    let cst = MimeCst::parse(&bytes);
    let attachment = cst.root.body_lists().attachments[0];

    assert_eq!(attachment.attachment_name().as_deref(), Some(filename));
    assert_eq!(
        attachment.content_type().param("name").as_deref(),
        Some(filename)
    );

    let theirs = mail_parser::MessageParser::new().parse(&bytes).unwrap();
    assert_eq!(
        theirs.attachments().next().unwrap().attachment_name(),
        Some(filename)
    );
}

#[test]
fn inline_resources_go_in_a_related_multipart_with_the_html() {
    let mut builder = draft();
    builder
        .text("text")
        .html("<img src=\"cid:logo@example.com\">")
        .inline(MimeAttachment {
            content_type: MimeContentType {
                top: "image".into(),
                sub: "png".into(),
                params: vec![],
            },
            content_id: Some("logo@example.com".into()),
            content: vec![0x89, b'P', b'N', b'G'],
            ..Default::default()
        })
        .unwrap();

    let bytes = bytes(builder);
    let cst = MimeCst::parse(&bytes);
    let kinds: Vec<String> = cst
        .entities()
        .iter()
        .map(|entity| {
            let content_type = entity.content_type();
            format!(
                "{}/{}",
                String::from_utf8_lossy(content_type.top),
                String::from_utf8_lossy(content_type.sub)
            )
        })
        .collect();

    assert_eq!(
        kinds,
        [
            "multipart/alternative",
            "text/plain",
            "multipart/related",
            "text/html",
            "image/png"
        ]
    );
    assert_eq!(
        cst.entities()[2].content_type().param("type").as_deref(),
        Some("text/html")
    );
    assert!(matches!(
        MimeBuilder::default().inline(MimeAttachment::default()),
        Err(MimeBuildError::InlineWithoutId)
    ));
}

#[test]
fn a_root_with_content_fields_gets_a_mime_version() {
    let mut builder = draft();
    builder.text("x");
    let bytes = bytes(builder);
    let cst = MimeCst::parse(&bytes);
    assert_eq!(
        cst.root
            .header::<mime::header::marker::MIME_VERSION>()
            .as_deref(),
        Some("1.0")
    );
}

#[test]
fn utf8_headers_are_written_raw_when_allowed() {
    let mut builder = draft();
    builder.header::<SUBJECT>("Grüße".into()).unwrap().text("x");
    let opts = MimeBuildOptions {
        allow_utf8_headers: true,
        ..Default::default()
    };
    let bytes = builder.write(&opts).unwrap();

    assert!(String::from_utf8_lossy(&bytes).contains("Subject: Grüße\r\n"));
    assert_eq!(
        MimeCst::parse(&bytes).root.header::<SUBJECT>().as_deref(),
        Some("Grüße")
    );
}

#[test]
fn a_model_mismatching_its_body_is_refused() {
    let mime = Mime {
        headers: vec![MimeHeader {
            name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
            value: MimeHeaderValue::ContentType(MimeContentType {
                top: "text".into(),
                sub: "plain".into(),
                params: vec![],
            }),
        }],
        body: MimeBody::Multipart(vec![]),
    };

    assert_eq!(
        mime.to_bytes(&MimeBuildOptions::default()),
        Err(MimeBuildError::Structure)
    );
    let _ = MimeAddressList::default();
}

mod properties {
    use mime::{
        build::{MimeBuildOptions, MimeLineEnding},
        header::{MimeHeader, MimeHeaderKind, MimeHeaderName},
        mime::{Mime, MimeBody},
        tree::cst::MimeCst,
        value::{
            MimeHeaderValue,
            address::{MimeAddress, MimeAddressList, MimeGroup, MimeMailbox},
            content_type::MimeContentType,
        },
    };
    use proptest::{collection::vec, option, prelude::*};

    /// Text mixing what a writer must take care of: blanks, specials,
    /// quotes, encoded-word lookalikes, non-ASCII, controls.
    fn text() -> impl Strategy<Value = String> {
        "[ a-zA-Z0-9éß日本\t=?\"(),;:<>@\\\\\u{1}\r\n.-]{0,90}"
    }

    fn mailbox() -> impl Strategy<Value = MimeMailbox> {
        (option::of(text()), "[a-z]{1,8}", "[a-z]{1,8}\\.test").prop_map(|(name, local, domain)| {
            MimeMailbox {
                name,
                address: Some(format!("{local}@{domain}")),
            }
        })
    }

    fn addresses() -> impl Strategy<Value = MimeAddressList> {
        vec(
            prop_oneof![
                3 => mailbox().prop_map(MimeAddress::Mailbox),
                1 => ("[A-Za-z]{1,10}", vec(mailbox(), 0..3)).prop_map(|(name, mailboxes)| {
                    MimeAddress::Group(MimeGroup {
                        name: Some(name),
                        mailboxes,
                    })
                }),
            ],
            1..4,
        )
        .prop_map(|items| MimeAddressList { items })
    }

    fn content_type(top: &str, sub: &str) -> MimeHeader {
        MimeHeader {
            name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
            value: MimeHeaderValue::ContentType(MimeContentType {
                top: top.into(),
                sub: sub.into(),
                params: vec![],
            }),
        }
    }

    fn body() -> impl Strategy<Value = Mime> {
        let leaf = prop_oneof![
            "[ a-zA-Zé日\t\r\n.=-]{0,300}".prop_map(|text| Mime {
                headers: vec![],
                body: MimeBody::Text(text),
            }),
            vec(any::<u8>(), 0..300).prop_map(|bytes| Mime {
                headers: vec![content_type("application", "octet-stream")],
                body: MimeBody::Binary(bytes),
            }),
        ];

        leaf.prop_recursive(3, 12, 3, |inner| {
            prop_oneof![
                vec(inner.clone(), 1..4).prop_map(|parts| Mime {
                    headers: vec![content_type("multipart", "mixed")],
                    body: MimeBody::Multipart(parts),
                }),
                inner.prop_map(|nested| Mime {
                    headers: vec![content_type("message", "rfc822")],
                    body: MimeBody::Message(Box::new(nested)),
                }),
            ]
        })
    }

    fn message() -> impl Strategy<Value = Mime> {
        (
            text(),
            addresses(),
            addresses(),
            vec("[a-zA-Z é]{1,12}", 0..4),
            body(),
        )
            .prop_map(|(subject, from, to, keywords, body)| {
                let mut headers = vec![
                    MimeHeader {
                        name: MimeHeaderName::Known(MimeHeaderKind::Date),
                        value: MimeHeaderValue::Date(
                            mime::value::datetime::MimeDateTime::from_unix_seconds(
                                1_793_448_000,
                                60,
                            ),
                        ),
                    },
                    MimeHeader {
                        name: MimeHeaderName::Known(MimeHeaderKind::Subject),
                        value: MimeHeaderValue::Text(subject),
                    },
                    MimeHeader {
                        name: MimeHeaderName::Known(MimeHeaderKind::From),
                        value: MimeHeaderValue::Addresses(from),
                    },
                    MimeHeader {
                        name: MimeHeaderName::Known(MimeHeaderKind::To),
                        value: MimeHeaderValue::Addresses(to),
                    },
                ];

                if !keywords.is_empty() {
                    headers.push(MimeHeader {
                        name: MimeHeaderName::Known(MimeHeaderKind::Keywords),
                        value: MimeHeaderValue::CommaList(keywords),
                    });
                }

                headers.extend(body.headers);
                Mime {
                    headers,
                    body: body.body,
                }
            })
    }

    /// What the reader gives back of written text: the ends trimmed.
    fn trimmed(text: &str) -> String {
        text.trim_matches([' ', '\t']).to_string()
    }

    fn normalised_addresses(list: &MimeAddressList) -> MimeAddressList {
        let mailbox = |mailbox: &MimeMailbox| MimeMailbox {
            name: mailbox
                .name
                .as_deref()
                .map(trimmed)
                .filter(|name| !name.is_empty()),
            address: mailbox.address.clone(),
        };

        MimeAddressList {
            items: list
                .items
                .iter()
                .map(|item| match item {
                    MimeAddress::Mailbox(m) => MimeAddress::Mailbox(mailbox(m)),
                    MimeAddress::Group(group) => MimeAddress::Group(MimeGroup {
                        name: group.name.clone(),
                        mailboxes: group.mailboxes.iter().map(mailbox).collect(),
                    }),
                })
                .collect(),
        }
    }

    fn value(mime: &Mime, kind: MimeHeaderKind) -> Option<&MimeHeaderValue> {
        mime.header(kind)
    }

    /// Text as written with `eol`: every LF a line break, with the CR before
    /// it under CRLF, with every CR before it under LF.
    fn written(text: &str, eol: &str) -> String {
        let mut out = String::new();
        let mut crs: usize = 0;

        for c in text.chars() {
            match c {
                '\r' => crs += 1,
                '\n' => {
                    let kept = if eol == "\n" {
                        0
                    } else {
                        crs.saturating_sub(1)
                    };
                    out.extend(std::iter::repeat_n('\r', kept));
                    out.push_str(eol);
                    crs = 0;
                }
                _ => {
                    out.extend(std::iter::repeat_n('\r', crs));
                    out.push(c);
                    crs = 0;
                }
            }
        }

        out.extend(std::iter::repeat_n('\r', crs));
        out
    }

    /// Asserts the leaves of `read` hold what `built` wrote, line breaks
    /// of text written as `eol`.
    fn assert_bodies(built: &Mime, read: &Mime, eol: &str) {
        match (&built.body, &read.body) {
            (MimeBody::Text(built), MimeBody::Text(read)) => {
                assert_eq!(read, &written(built, eol));
            }
            (MimeBody::Binary(built), MimeBody::Binary(read)) => assert_eq!(read, built),
            (MimeBody::Multipart(built), MimeBody::Multipart(read)) => {
                assert_eq!(built.len(), read.len());

                for (built, read) in built.iter().zip(read) {
                    assert_bodies(built, read, eol);
                }
            }
            (MimeBody::Message(built), MimeBody::Message(read)) => assert_bodies(built, read, eol),
            (built, read) => panic!("{built:?} read back as {read:?}"),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        #[test]
        fn a_model_builds_and_reads_back_as_built(mime in message(), lf: bool, allow_8bit: bool, allow_binary: bool, allow_utf8_headers: bool) {
            let opts = MimeBuildOptions {
                line_ending: if lf { MimeLineEnding::Lf } else { MimeLineEnding::Crlf },
                allow_8bit,
                allow_binary,
                allow_utf8_headers,
                ..Default::default()
            };
            let eol = if lf { "\n" } else { "\r\n" };
            let bytes = mime.to_bytes(&opts).unwrap();

            let cst = MimeCst::parse(&bytes);
            prop_assert!(cst.anomalies().is_empty(), "{:?}", cst.anomalies());

            for line in bytes.split(|byte| *byte == b'\n').filter(|_| !allow_binary) {
                prop_assert!(line.len() <= 999);
                let content = if lf { line } else { &line[..line.len().saturating_sub(1)] };
                prop_assert!(!content.contains(&b'\r'));
            }

            // NOTE: the writer adds no violation the model does not state.
            let stated = mime.check();
            let written = cst.check();

            for problem in written.violations() {
                prop_assert!(
                    stated.violations().any(|known| known.rule == problem.rule),
                    "{problem:?}"
                );
            }

            let read = cst.decode();

            let subject = value(&mime, MimeHeaderKind::Subject)
                .and_then(MimeHeaderValue::as_text)
                .map(|text| MimeHeaderValue::Text(trimmed(text)));
            prop_assert_eq!(value(&read, MimeHeaderKind::Subject), subject.as_ref());

            for kind in [MimeHeaderKind::From, MimeHeaderKind::To] {
                let built = value(&mime, kind).and_then(MimeHeaderValue::as_addresses).unwrap();
                let read = value(&read, kind).and_then(MimeHeaderValue::as_addresses).unwrap();
                prop_assert_eq!(read, &normalised_addresses(built));
            }

            let keywords = value(&mime, MimeHeaderKind::Keywords)
                .and_then(MimeHeaderValue::as_comma_list)
                .map(|items| MimeHeaderValue::CommaList(items.iter().map(|item| trimmed(item)).filter(|item| !item.is_empty()).collect()));
            prop_assert_eq!(value(&read, MimeHeaderKind::Keywords), keywords.as_ref());

            assert_bodies(&mime, &read, eol);

            let again = read.to_bytes(&opts).unwrap();
            prop_assert_eq!(&again, &bytes, "not a fixpoint");

            let theirs = mail_parser::MessageParser::new().parse(&bytes).unwrap();
            prop_assert_eq!(theirs.parts().len(), cst.entities().len());
            prop_assert_eq!(
                theirs.subject().unwrap_or_default(),
                value(&read, MimeHeaderKind::Subject).and_then(MimeHeaderValue::as_text).unwrap_or_default()
            );
        }
    }
}

/// The oracle of fuzz/fuzz_targets/write.rs: the model of any bytes, once
/// written, rebuilds to the same bytes.
fn assert_rebuilds(data: &[u8]) {
    let (&flags, data) = data.split_first().unwrap();
    let opts = MimeBuildOptions {
        line_ending: if flags & 1 == 0 {
            MimeLineEnding::Crlf
        } else {
            MimeLineEnding::Lf
        },
        allow_8bit: flags & 2 != 0,
        allow_binary: flags & 4 != 0,
        allow_utf8_headers: flags & 8 != 0,
        flowed: flags & 16 != 0,
        legacy_name: flags & 32 != 0,
        cpu: None,
    };

    let Ok(first) = MimeCst::parse(data).decode().to_bytes(&opts) else {
        return;
    };

    let once = MimeCst::parse(&first).decode();
    let second = once.to_bytes(&opts).unwrap();

    assert_eq!(
        String::from_utf8_lossy(&second),
        String::from_utf8_lossy(&first),
        "{:?}",
        String::from_utf8_lossy(data)
    );
}

#[test]
fn fuzz_findings_rebuild_stably() {
    let findings: &[&[u8]] = &[
        b"\x00Content-Type: text/plain; format=flowed\n\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \xc3\xa9bbbb\n",
        b"\nTo:<;",
        b"x\r\r",
        b"\x00Content-ID:5>",
        b"Su \r\r\n",
        b":Content-Disposition:e filename=\xd2",
        b"0Content-Disposition:a filename=\xd2",
        b"TContent-Type:n;*=\" ",
        b"S\xff\xfe\xff\xfe",
        b"\x00To: \\ ",
        b"\x00To: <\"\\",
        b"\x00To: @\":",
        b"$Content-Disposition:h filename=\"\xd2 \x0a\x0a\xd2\xd2",
        b"M\x0e\x0aContent-Type: multipart/al boundary=-D74AE24A\xf1E2\x0a\x0a---D74AE24A\xef\xbf\xbdE25\x0a",
        b"\x0aContent-Type:2 2*=''",
        b"S\" \x0d \x0a",
        b"eContent-Type: multipart/i\x09boundary=\"bou\x0d\x0a\x0d\x0a--boun\x0d\x0aContent-Transfer-Encoding:\x09\x0a\x0d\x0a\x0d",
    ];

    for finding in findings {
        assert_rebuilds(finding);
    }
}

mod review {
    use mime::{
        build::{MimeBuildError, MimeBuildOptions},
        builder::{MimeAttachment, MimeBuilder},
        header::{MimeHeader, MimeHeaderKind, MimeHeaderName},
        mime::{Mime, MimeBody},
        tree::cst::MimeCst,
        validator::MimeValidateRule,
        value::{MimeHeaderValue, content_type::MimeContentType},
    };

    #[test]
    fn text_under_a_multipart_type_stays_a_leaf() {
        let mime = Mime {
            headers: vec![MimeHeader {
                name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
                value: MimeHeaderValue::ContentType(MimeContentType {
                    top: "multipart".into(),
                    sub: "mixed".into(),
                    params: vec![mime::value::content_type::MimeParam {
                        name: "boundary".into(),
                        value: "x".into(),
                    }],
                }),
            }],
            body: MimeBody::Text("--x\r\nContent-Type: text/html\r\n\r\n<b>hi</b>\r\n--x--".into()),
        };

        let bytes = mime.to_bytes(&MimeBuildOptions::default()).unwrap();
        assert_eq!(MimeCst::parse(&bytes).entities().len(), 1);
    }

    #[test]
    fn an_attached_message_takes_an_identity_encoding() {
        let mut builder = MimeBuilder::default();
        builder
            .text("see attached")
            .attachment(MimeAttachment {
                content_type: MimeContentType {
                    top: "message".into(),
                    sub: "rfc822".into(),
                    params: vec![],
                },
                content: b"Subject: inner\r\n\r\nbody\r\n".to_vec(),
                ..Default::default()
            })
            .unwrap();

        let bytes = builder.write(&MimeBuildOptions::default()).unwrap();
        let output = MimeCst::parse(&bytes).check();
        assert!(
            !output
                .problems
                .iter()
                .any(|p| p.rule == MimeValidateRule::CompositeEncoding)
        );
    }

    #[test]
    fn the_raw_escape_hatch_keeps_to_the_reserved_fields_and_the_line_limit() {
        let mut builder = MimeBuilder::default();
        assert_eq!(
            builder.raw_header("MIME-Version", b" 2.0").unwrap_err(),
            MimeBuildError::Reserved(MimeHeaderKind::MimeVersion)
        );

        let mut value = b" ".to_vec();
        value.extend(std::iter::repeat_n(b'x', 2000));
        builder.raw_header("X-Long", &value).unwrap();
        assert_eq!(
            builder.write(&MimeBuildOptions::default()),
            Err(MimeBuildError::LineTooLong)
        );
    }
}
