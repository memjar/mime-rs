//! The closed vocabularies and the spec dispatch: every kind parses from
//! its own spelling in any case and answers for itself, every form names
//! itself, and a value of every form reads back the same once written.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

use mime::{
    build::MimeBuildOptions,
    header::{MimeHeader, MimeHeaderKind, MimeHeaderName, spec::MimeHeaderForm},
    mime::{Mime, MimeBody},
    tree::cst::MimeCst,
    value::{
        MimeHeaderValue,
        address::{MimeAddress, MimeAddressList, MimeGroup, MimeMailbox},
        content_type::{MimeContentType, MimeDisposition, MimeParam},
        datetime::MimeDateTime,
        transfer_encoding::MimeTransferEncoding,
    },
};

#[test]
fn every_kind_parses_from_its_spelling_and_answers_for_itself() {
    for kind in MimeHeaderKind::ALL {
        let spelling: &str = kind;

        assert_eq!(spelling.parse::<MimeHeaderKind>(), Ok(*kind));
        assert_eq!(spelling.to_ascii_lowercase().parse(), Ok(*kind));
        assert_eq!(spelling.to_ascii_uppercase().parse(), Ok(*kind));
        assert_eq!(kind.spec().kind, *kind);
        assert_eq!(MimeHeaderName::new(spelling), MimeHeaderName::Known(*kind));
    }

    assert_eq!(MimeHeaderKind::ALL.len(), 40);
    assert!("Sub ject".parse::<MimeHeaderKind>().is_err());
}

#[test]
fn every_form_names_itself() {
    for form in MimeHeaderForm::ALL {
        let spelling: &str = form;
        assert_eq!(spelling.parse::<MimeHeaderForm>(), Ok(*form));
        assert_eq!(spelling.to_ascii_lowercase().parse(), Ok(*form));
    }

    assert_eq!(&*MimeHeaderForm::Text, "asText");
    assert_eq!(&*MimeTransferEncoding::QuotedPrintable, "quoted-printable");
}

/// One value of each form, and the field it is written under.
fn samples() -> Vec<(&'static str, MimeHeaderForm, MimeHeaderValue)> {
    let mailbox = MimeMailbox::named("Zoë \"Z\" Doe", "zoe@example.com");

    vec![
        (
            "X-Raw",
            MimeHeaderForm::Raw,
            MimeHeaderValue::Raw(b" a  (b)\r\n c".to_vec()),
        ),
        (
            "Subject",
            MimeHeaderForm::Text,
            MimeHeaderValue::Text("Ça va, très bien".into()),
        ),
        (
            "To",
            MimeHeaderForm::GroupedAddresses,
            MimeHeaderValue::Addresses(MimeAddressList {
                items: vec![
                    MimeAddress::Mailbox(mailbox.clone()),
                    MimeAddress::Group(MimeGroup {
                        name: Some("Team".into()),
                        mailboxes: vec![MimeMailbox::address("a@example.com")],
                    }),
                ],
            }),
        ),
        (
            "Cc",
            MimeHeaderForm::Addresses,
            MimeHeaderValue::Addresses(MimeAddressList::from(mailbox)),
        ),
        (
            "References",
            MimeHeaderForm::MessageIds,
            MimeHeaderValue::MessageIds(vec!["a@b".into(), "c.d@e".into()]),
        ),
        (
            "Date",
            MimeHeaderForm::Date,
            MimeHeaderValue::Date(MimeDateTime {
                year: 1969,
                month: 2,
                day: 13,
                hour: 23,
                minute: 32,
                second: 0,
                offset: -300,
            }),
        ),
        (
            "List-Post",
            MimeHeaderForm::Urls,
            MimeHeaderValue::Urls(vec![
                "mailto:list@example.com".into(),
                "https://x.test/".into(),
            ]),
        ),
        (
            "Keywords",
            MimeHeaderForm::CommaList,
            MimeHeaderValue::CommaList(vec!["one".into(), "deux, trois".into(), "été".into()]),
        ),
        (
            "Content-Type",
            MimeHeaderForm::ContentType,
            MimeHeaderValue::ContentType(MimeContentType {
                top: "text".into(),
                sub: "plain".into(),
                params: vec![
                    MimeParam {
                        name: "charset".into(),
                        value: "utf-8".into(),
                    },
                    MimeParam {
                        name: "format".into(),
                        value: "fixed".into(),
                    },
                ],
            }),
        ),
        (
            "Content-Disposition",
            MimeHeaderForm::Disposition,
            MimeHeaderValue::Disposition(MimeDisposition {
                kind: "attachment".into(),
                params: vec![MimeParam {
                    name: "filename".into(),
                    value: "naïve résumé ".repeat(8),
                }],
            }),
        ),
        (
            "Content-Transfer-Encoding",
            MimeHeaderForm::TransferEncoding,
            MimeHeaderValue::TransferEncoding(MimeTransferEncoding::SevenBit),
        ),
    ]
}

#[test]
fn every_form_reads_back_once_written() {
    for (name, form, value) in samples() {
        let mime = Mime {
            headers: vec![MimeHeader {
                name: MimeHeaderName::new(name),
                value: value.clone(),
            }],
            body: MimeBody::Text("x".into()),
        };

        let bytes = mime.to_bytes(&MimeBuildOptions::default()).unwrap();
        let cst = MimeCst::parse(&bytes);
        let field = cst.root.header.get(name).unwrap();
        let read = field.read_as(form);

        let expected = match (&value, form) {
            (MimeHeaderValue::Raw(raw), _) => MimeHeaderValue::Raw(raw.clone()),
            (MimeHeaderValue::Addresses(list), MimeHeaderForm::Addresses) => {
                MimeHeaderValue::Addresses(list.mailboxes().cloned().collect())
            }
            _ => value.clone(),
        };

        assert_eq!(read, expected, "{name}");
        assert_eq!(form.parse(field.value()), expected, "{name} standing alone");
    }
}
