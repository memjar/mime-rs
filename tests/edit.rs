//! Edits rewrite only what they name: every other field, entity, delimiter
//! line, preamble and epilogue keeps its bytes, and every leaf they do not
//! write still borrows the input.

#![cfg(all(feature = "parser", feature = "base64", feature = "encoding"))]

use std::borrow::Cow;

use mime::{
    build::MimeBuildOptions,
    header::marker::{BCC, DATE, FROM, MESSAGE_ID, SUBJECT, TO},
    mime::{Mime, MimeBody},
    tree::{
        body::MimeBodyNode, cst::MimeCst, edit::MimeEditError, entity::MimeEntity,
        header::MimeHeaderItem,
    },
    value::{
        MimeHeaderValue,
        address::{MimeAddress, MimeMailbox},
        content_type::MimeContentType,
        datetime::MimeDateTime,
        message_id::MimeMessageId,
        transfer_encoding::MimeTransferEncoding,
    },
};

const DRAFT: &str = concat!(
    "From: Alice <alice@example.com>\r\n",
    "To:   \"Doe, J.\" <j@x.test>,\r\n",
    "  (team) t@x.test\r\n",
    "Subject: Quarterly\r\n",
    "\treport\r\n",
    "Bcc: eve@x.test\r\n",
    "X-Odd  :  spaced   value\r\n",
    "Bcc: (hidden)\r\n",
    " mallory@x.test\r\n",
    "MIME-Version: 1.0\r\n",
    "Content-Type: multipart/mixed; boundary=\"b1\"\r\n",
    "\r\n",
    "preamble\r\n",
    "--b1\r\n",
    "Content-Type: text/plain; charset=utf-8\r\n",
    "\r\n",
    "Hello\r\n",
    "--b1   \r\n",
    "Content-Type: application/octet-stream\r\n",
    "Content-Transfer-Encoding: base64\r\n",
    "\r\n",
    "AAECAwQF\r\n",
    "--b1\r\n",
    "Content-Type: text/plain\r\n",
    "\r\n",
    "third\r\n",
    "--b1--\r\n",
    "epilogue\r\n",
);

fn opts() -> MimeBuildOptions {
    MimeBuildOptions::default()
}

/// Asserts `after` is `before` with `old` replaced by `new` once, and
/// nothing else.
fn assert_only(before: &[u8], after: &[u8], old: &str, new: &str) {
    let expected = String::from_utf8_lossy(before).replacen(old, new, 1);
    assert_eq!(String::from_utf8_lossy(after), expected);
}

/// Every leaf of an entity that an edit did not write, as a list of what
/// is owned.
fn owned(entity: &MimeEntity<'_>, path: &str, out: &mut Vec<String>) {
    for (index, item) in entity.header.items.iter().enumerate() {
        let owned = match item {
            MimeHeaderItem::Field(field) => field.is_owned(),
            MimeHeaderItem::Opaque(line) => matches!(line, Cow::Owned(_)),
        };

        if owned {
            out.push(format!("{path}.header[{index}]"));
        }
    }

    match &entity.body {
        MimeBodyNode::Leaf(Cow::Owned(_)) => out.push(format!("{path}.body")),
        MimeBodyNode::Multipart(multipart) => {
            for (index, part) in multipart.parts.iter().enumerate() {
                if matches!(part.delimiter, Cow::Owned(_)) {
                    out.push(format!("{path}.delimiter[{index}]"));
                }

                owned(&part.entity, &format!("{path}.part[{index}]"), out);
            }

            if matches!(multipart.close, Cow::Owned(_)) {
                out.push(format!("{path}.close"));
            }
        }
        MimeBodyNode::Message(nested) => owned(nested, &format!("{path}.message"), out),
        MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => {}
    }
}

fn owned_leaves(cst: &MimeCst<'_>) -> Vec<String> {
    let mut out = Vec::new();
    owned(&cst.root, "root", &mut out);
    out
}

#[test]
fn a_subject_changed_on_a_folded_message_touches_its_field_alone() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    cst.header_mut(0)
        .unwrap()
        .set::<SUBJECT>("Annual report".into(), &opts())
        .unwrap();

    assert_only(
        DRAFT.as_bytes(),
        &cst.to_bytes(),
        "Subject: Quarterly\r\n\treport\r\n",
        "Subject: Annual report\r\n",
    );
    assert_eq!(owned_leaves(&cst), ["root.header[2]"]);
}

#[test]
fn every_bcc_field_goes_with_its_continuation_lines() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    assert_eq!(cst.header_mut(0).unwrap().remove_all("bcc"), Ok(2));

    let expected = DRAFT
        .replace("Bcc: eve@x.test\r\n", "")
        .replace("Bcc: (hidden)\r\n mallory@x.test\r\n", "");
    assert_eq!(String::from_utf8_lossy(&cst.to_bytes()), expected);
    assert!(owned_leaves(&cst).is_empty());
    assert!(cst.root.header::<BCC>().is_none());
}

#[test]
fn a_new_field_goes_where_it_belongs_and_nothing_moves() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    let header = cst.header_mut(0).unwrap();

    header
        .insert::<BCC>(MimeMailbox::address("trent@x.test").into(), &opts())
        .unwrap();
    header
        .insert_value(
            &mime::header::MimeHeaderName::new("Received"),
            &MimeHeaderValue::Raw(b" from relay.x.test".to_vec()),
            &opts(),
        )
        .unwrap();
    header
        .insert::<DATE>(MimeDateTime::from_unix_seconds(0, 0), &opts())
        .unwrap();

    let names: Vec<String> = cst
        .root
        .header
        .fields()
        .map(|field| field.name_str().into_owned())
        .collect();

    assert_eq!(
        names,
        [
            "Received",
            "From",
            "To",
            "Subject",
            "Bcc",
            "X-Odd",
            "Bcc",
            "Bcc",
            "MIME-Version",
            "Content-Type",
            "Date"
        ]
    );
    assert_eq!(owned_leaves(&cst).len(), 3);
}

#[test]
fn a_recipient_appended_keeps_the_other_items_bytes() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    let header = cst.header_mut(0).unwrap();
    let item = MimeAddress::Mailbox(MimeMailbox::address("k@x.test"));
    header.push_address("to", &item, &opts()).unwrap();

    assert_only(
        DRAFT.as_bytes(),
        &cst.to_bytes(),
        "  (team) t@x.test\r\n",
        "  (team) t@x.test, k@x.test\r\n",
    );

    let to = cst.root.header::<TO>().unwrap();
    let addresses: Vec<_> = to
        .mailboxes()
        .filter_map(|m| m.address.as_deref())
        .collect();
    assert_eq!(addresses, ["j@x.test", "t@x.test", "k@x.test"]);
}

#[test]
fn an_item_is_replaced_or_removed_with_its_comma() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    let header = cst.header_mut(0).unwrap();

    let item = MimeAddress::Mailbox(MimeMailbox::named("Bob", "b@x.test"));
    header.set_address("to", 1, &item, &opts()).unwrap();
    assert!(
        String::from_utf8_lossy(&cst.to_bytes())
            .contains("To:   \"Doe, J.\" <j@x.test>,\r\n  Bob <b@x.test>\r\n")
    );

    let header = cst.header_mut(0).unwrap();
    header.remove_address("to", 0).unwrap();
    assert!(String::from_utf8_lossy(&cst.to_bytes()).contains("To:   Bob <b@x.test>\r\n"));

    let header = cst.header_mut(0).unwrap();
    assert_eq!(
        header.remove_address("to", 5),
        Err(MimeEditError::NoSuchItem(5))
    );
    header.remove_address("to", 0).unwrap();
    assert!(cst.root.header.get("to").is_none());
}

#[test]
fn one_part_replaced_owns_its_body_and_its_transfer_encoding_alone() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    let content: Vec<u8> = (0..=255u8).collect();
    cst.replace_content(2, &content, None, &opts()).unwrap();

    assert_eq!(
        owned_leaves(&cst),
        ["root.part[1].header[1]", "root.part[1].body"]
    );

    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    assert_eq!(
        reread.entities()[2].decoded().unwrap().as_ref(),
        content.as_slice()
    );
    assert!(reread.anomalies().is_empty());

    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    cst.replace_content(1, "café\n".as_bytes(), None, &opts())
        .unwrap();
    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    assert_eq!(
        reread.entities()[1].transfer_encoding(),
        MimeTransferEncoding::QuotedPrintable
    );
    assert_eq!(reread.entities()[1].text().as_deref(), Some("café\r\n"));
    assert_eq!(
        owned_leaves(&cst),
        ["root.part[0].header[1]", "root.part[0].body"]
    );
}

fn part(text: &str) -> Mime {
    Mime {
        headers: vec![],
        body: MimeBody::Text(text.into()),
    }
}

#[test]
fn parts_are_inserted_removed_and_moved_within_their_multipart() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    cst.insert_part(0, 1, &part("inserted"), &opts()).unwrap();

    let bytes = cst.to_bytes();
    assert_only(
        DRAFT.as_bytes(),
        &bytes,
        "--b1   \r\n",
        "--b1   \r\n\r\ninserted\r\n--b1   \r\n",
    );

    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    let removed = cst.remove_part(0, 1, &opts()).unwrap();
    assert_eq!(removed.content_type().param("boundary"), None);
    assert_only(
        DRAFT.as_bytes(),
        &cst.to_bytes(),
        "\r\n--b1   \r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\nAAECAwQF\r\n",
        "\r\n",
    );

    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    cst.move_part(0, 2, 0, &opts()).unwrap();
    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    let texts: Vec<_> = reread
        .entities()
        .iter()
        .skip(1)
        .map(|entity| entity.decoded().unwrap().into_owned())
        .collect();
    assert_eq!(texts[0], b"third");
    assert_eq!(texts[1], b"Hello");
    assert!(reread.anomalies().is_empty());
    assert!(
        String::from_utf8_lossy(&bytes)
            .contains("preamble\r\n--b1\r\nContent-Type: text/plain\r\n\r\nthird")
    );
}

#[test]
fn an_inserted_part_holding_the_boundary_regenerates_it() {
    let mut cst = MimeCst::parse(DRAFT.as_bytes());
    cst.insert_part(0, 9, &part("--b1\r\nnot a delimiter\r\n"), &opts())
        .unwrap();

    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    let boundary = reread.root.content_type().param("boundary").unwrap();

    assert_ne!(boundary, "b1");
    assert_eq!(reread.entities().len(), 5);
    assert!(reread.anomalies().is_empty());
    assert_eq!(
        reread.entities()[4].text().as_deref(),
        Some("--b1\r\nnot a delimiter\r\n")
    );
    assert!(String::from_utf8_lossy(&bytes).contains("preamble\r\n"));
}

#[test]
fn an_attachment_added_to_a_plain_draft_wraps_it() {
    let draft = concat!(
        "From: Alice <alice@example.com>\r\n",
        "To: bob@example.com\r\n",
        "Subject: Hi\r\n",
        "Date: Sat, 31 Oct 2026 12:00:00 +0100\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "\r\n",
        "Hello Bob\r\n",
    );

    let mut cst = MimeCst::parse(draft.as_bytes());
    let attachment = Mime {
        headers: vec![mime::header::MimeHeader {
            name: mime::header::MimeHeaderName::new("Content-Type"),
            value: MimeHeaderValue::ContentType(MimeContentType {
                top: "application".into(),
                sub: "pdf".into(),
                params: vec![],
            }),
        }],
        body: MimeBody::Binary(b"%PDF-1.7".to_vec()),
    };

    cst.attach(0, &attachment, &opts()).unwrap();

    assert!(cst.root.content_type().is("multipart", "mixed"));

    let entities = cst.entities();
    assert_eq!(entities.len(), 3);
    assert_eq!(entities[1].bytes(), Some(&b"Hello Bob\r\n"[..]));
    assert!(matches!(
        entities[1].body,
        MimeBodyNode::Leaf(Cow::Borrowed(_))
    ));

    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    assert!(reread.anomalies().is_empty());
    assert_eq!(reread.root.header::<SUBJECT>().as_deref(), Some("Hi"));
    assert!(reread.root.header::<FROM>().is_some());
    assert!(reread.root.header::<DATE>().is_some());
    assert_eq!(
        reread.entities()[1].text().as_deref(),
        Some("Hello Bob\r\n")
    );
    assert_eq!(
        reread.entities()[2].decoded().unwrap().as_ref(),
        b"%PDF-1.7"
    );

    let mut cst = MimeCst::parse(&bytes);
    cst.remove_part(0, 1, &opts()).unwrap();
    cst.unwrap(0, &opts()).unwrap();
    let bytes = cst.to_bytes();
    assert!(
        String::from_utf8_lossy(&bytes)
            .ends_with("Content-Type: text/plain; charset=utf-8\r\n\r\nHello Bob\r\n")
    );
}

#[test]
fn a_downgrade_re_encodes_eight_bit_leaves_and_reports_utf8_fields() {
    let message = concat!(
        "Subject: Grüße\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=b\r\n",
        "Content-Transfer-Encoding: 8bit\r\n",
        "\r\n",
        "--b\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "Content-Transfer-Encoding: 8bit\r\n",
        "\r\n",
        "Grüße\r\n",
        "--b\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "plain ascii\r\n",
        "--b\r\n",
        "Content-Type: message/rfc822\r\n",
        "Content-Transfer-Encoding: 8bit\r\n",
        "\r\n",
        "Subject: inner\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "Content-Transfer-Encoding: 8bit\r\n",
        "\r\n",
        "Ünïcödé\r\n",
        "--b--\r\n",
    );

    let mut cst = MimeCst::parse(message.as_bytes());
    let report = cst.downgrade_7bit(&opts()).unwrap();

    assert_eq!(report.utf8_fields, [(0, String::from("Subject"))]);
    assert!(report.sealed.is_empty());

    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    let entities = reread.entities();

    assert!(bytes.lines_are_seven_bit_but_the_subject());
    assert_eq!(entities[1].text().as_deref(), Some("Grüße"));
    assert_eq!(entities[2].bytes(), Some(&b"plain ascii"[..]));
    assert!(matches!(entities[3].body, MimeBodyNode::Message(_)));
    assert_eq!(entities[4].text().as_deref(), Some("Ünïcödé"));
    assert_eq!(
        entities[0].transfer_encoding(),
        MimeTransferEncoding::SevenBit
    );
    assert_eq!(
        entities[3].transfer_encoding(),
        MimeTransferEncoding::SevenBit
    );
}

trait SevenBit {
    fn lines_are_seven_bit_but_the_subject(&self) -> bool;
}

impl SevenBit for Vec<u8> {
    fn lines_are_seven_bit_but_the_subject(&self) -> bool {
        self.split(|byte| *byte == b'\n')
            .filter(|line| !line.starts_with(b"Subject:"))
            .all(|line| line.is_ascii())
    }
}

#[test]
fn filling_the_required_fields_is_idempotent() {
    let draft = "From: a@x.test\r\nContent-Type: text/plain\r\n\r\nbody\r\n";
    let date = MimeDateTime::from_unix_seconds(1_793_448_000, 60);
    let id = MimeMessageId::new("1", "x.test").unwrap();

    let mut cst = MimeCst::parse(draft.as_bytes());
    cst.fill_required(&date, &id, &opts()).unwrap();
    let once = cst.to_bytes();

    assert!(
        String::from_utf8_lossy(&once).starts_with(
            draft
                .trim_end_matches("\r\nbody\r\n")
                .trim_end_matches("\r\n")
        )
    );

    let mut again = MimeCst::parse(&once);
    again.fill_required(&date, &id, &opts()).unwrap();
    assert_eq!(again.to_bytes(), once);

    let reread = MimeCst::parse(&once);
    assert_eq!(reread.root.header::<DATE>(), Some(date));
    assert_eq!(
        reread.root.header::<MESSAGE_ID>(),
        Some(vec!["1@x.test".into()])
    );
    assert_eq!(
        reread
            .root
            .header::<mime::header::marker::MIME_VERSION>()
            .as_deref(),
        Some("1.0")
    );
    assert_eq!(reread.root.bytes(), Some(&b"body\r\n"[..]));
}

const SIGNED: &str = concat!(
    "From: a@x.test\r\n",
    "Subject: signed\r\n",
    "MIME-Version: 1.0\r\n",
    "Content-Type: multipart/signed; protocol=\"application/pgp-signature\"; micalg=pgp-sha256; boundary=s\r\n",
    "\r\n",
    "--s\r\n",
    "Content-Type: text/plain\r\n",
    "\r\n",
    "signed text\r\n",
    "--s\r\n",
    "Content-Type: application/pgp-signature\r\n",
    "\r\n",
    "-----BEGIN PGP SIGNATURE-----\r\n",
    "--s--\r\n",
);

#[test]
fn protected_content_is_sealed() {
    let mut cst = MimeCst::parse(SIGNED.as_bytes());

    assert_eq!(cst.protector(1), Some(0));
    assert_eq!(cst.protector(2), None);
    assert_eq!(cst.entity_mut(1).unwrap_err(), MimeEditError::Sealed(0));
    assert_eq!(cst.header_mut(1).unwrap_err(), MimeEditError::Sealed(0));
    assert_eq!(cst.entity_mut(0).unwrap_err(), MimeEditError::Sealed(0));
    assert_eq!(cst.to_bytes(), SIGNED.as_bytes());

    cst.header_mut(0)
        .unwrap()
        .set::<SUBJECT>("still signed".into(), &opts())
        .unwrap();
    assert_only(
        SIGNED.as_bytes(),
        &cst.to_bytes(),
        "Subject: signed\r\n",
        "Subject: still signed\r\n",
    );

    cst.unwrap_protected(0).unwrap();
    cst.replace_content(0, b"edited", None, &opts()).unwrap();

    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    assert_eq!(reread.entities().len(), 1);
    assert_eq!(reread.root.text().as_deref(), Some("edited"));
    assert_eq!(
        reread.root.header::<SUBJECT>().as_deref(),
        Some("still signed")
    );
}

#[test]
fn a_downgrade_leaves_sealed_content_and_says_so() {
    let signed = SIGNED.replace("signed text", "signéd text");
    let mut cst = MimeCst::parse(signed.as_bytes());
    let report = cst.downgrade_7bit(&opts()).unwrap();

    assert_eq!(report.sealed, [1]);
    assert_eq!(cst.to_bytes(), signed.as_bytes());
}

mod properties {
    use std::{fs, path::PathBuf};

    use mime::{
        build::MimeBuildOptions,
        header::{MimeHeaderName, marker::SUBJECT},
        mime::{Mime, MimeBody},
        tree::cst::MimeCst,
        value::{
            MimeHeaderValue,
            address::{MimeAddress, MimeMailbox},
        },
    };
    use proptest::{collection::vec, prelude::*};

    fn corpus() -> Vec<Vec<u8>> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
        let mut messages = Vec::new();

        for dir in fs::read_dir(root).unwrap().flatten() {
            if dir.path().is_dir() {
                for file in fs::read_dir(dir.path()).unwrap().flatten() {
                    if file.path().extension().is_some_and(|ext| ext == "eml") {
                        messages.push(fs::read(file.path()).unwrap());
                    }
                }
            }
        }

        messages.sort();
        messages
    }

    #[derive(Clone, Debug)]
    enum Edit {
        Subject(String),
        Insert(String, String),
        Remove(String),
        Push(usize, String),
        Content(usize, Vec<u8>),
        Part(usize, usize, String),
        Drop(usize, usize),
        Move(usize, usize, usize),
        Wrap(usize),
    }

    fn edit() -> impl Strategy<Value = Edit> {
        let name = prop_oneof![
            Just("X-Test".to_string()),
            Just("Subject".to_string()),
            Just("To".to_string()),
            Just("Received".to_string()),
            Just("Bcc".to_string())
        ];

        prop_oneof![
            "[ a-zé日=?]{0,40}".prop_map(Edit::Subject),
            (name.clone(), "[ a-z]{0,20}").prop_map(|(name, value)| Edit::Insert(name, value)),
            name.prop_map(Edit::Remove),
            (any::<usize>(), "[a-z]{1,6}").prop_map(|(i, local)| Edit::Push(i, local)),
            (any::<usize>(), vec(any::<u8>(), 0..200))
                .prop_map(|(i, bytes)| Edit::Content(i, bytes)),
            (any::<usize>(), any::<usize>(), "[ a-z\r\n-]{0,60}")
                .prop_map(|(i, at, text)| Edit::Part(i, at, text)),
            (any::<usize>(), any::<usize>()).prop_map(|(i, at)| Edit::Drop(i, at)),
            (any::<usize>(), any::<usize>(), any::<usize>())
                .prop_map(|(i, from, to)| Edit::Move(i, from, to)),
            any::<usize>().prop_map(Edit::Wrap),
        ]
    }

    /// Applies an edit, returning the index of the entity it named, or
    /// `None` when it was refused.
    fn apply(cst: &mut MimeCst<'_>, edit: &Edit) -> Option<usize> {
        let opts = MimeBuildOptions::default();
        let count = cst.entities().len();

        match edit {
            Edit::Subject(text) => {
                let header = cst.header_mut(0).ok()?;
                header.set::<SUBJECT>(text.clone(), &opts).ok()?;
                Some(0)
            }
            Edit::Insert(name, value) => {
                let header = cst.header_mut(0).ok()?;
                let value = MimeHeaderValue::Raw(format!(" {value}").into_bytes());
                header
                    .insert_value(&MimeHeaderName::new(name), &value, &opts)
                    .ok()?;
                Some(0)
            }
            Edit::Remove(name) => {
                cst.header_mut(0).ok()?.remove_all(name).ok()?;
                Some(0)
            }
            Edit::Push(index, local) => {
                let index = index % count;
                let item = MimeAddress::Mailbox(MimeMailbox::address(format!("{local}@x.test")));
                cst.header_mut(index)
                    .ok()?
                    .push_address("To", &item, &opts)
                    .ok()?;
                Some(index)
            }
            Edit::Content(index, bytes) => {
                let index = index % count;
                cst.replace_content(index, bytes, None, &opts).ok()?;
                Some(index)
            }
            Edit::Part(index, at, text) => {
                let index = index % count;
                let part = Mime {
                    headers: vec![],
                    body: MimeBody::Text(text.clone()),
                };
                cst.insert_part(index, *at, &part, &opts).ok()?;
                Some(index)
            }
            Edit::Drop(index, at) => {
                let index = index % count;
                let parts = cst.entities()[index].body.multipart()?.parts.len();
                cst.remove_part(index, at % parts.max(1), &opts).ok()?;
                Some(index)
            }
            Edit::Move(index, from, to) => {
                let index = index % count;
                let parts = cst.entities()[index].body.multipart()?.parts.len().max(1);
                cst.move_part(index, from % parts, to % parts, &opts).ok()?;
                Some(index)
            }
            Edit::Wrap(index) => {
                let index = index % count;
                cst.wrap(index, "mixed", &opts).ok()?;
                Some(index)
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        #[test]
        fn edits_read_back_as_stated_and_keep_what_they_do_not_name(
            pick in any::<usize>(),
            edits in vec(edit(), 1..6),
        ) {
            let corpus = corpus();
            let input = &corpus[pick % corpus.len()];
            let mut cst = MimeCst::parse(input);

            for edit in &edits {
                let before: Vec<Vec<u8>> = cst.entities().iter().map(|e| e.to_bytes()).collect();
                let header_only = matches!(edit, Edit::Subject(_) | Edit::Insert(..) | Edit::Remove(_));

                let Some(named) = apply(&mut cst, edit) else {
                    continue;
                };

                // NOTE: a field edit on the root changes the root's bytes
                // alone; every entity under it is untouched.
                if header_only && named == 0 {
                    let after: Vec<Vec<u8>> = cst.entities().iter().map(|e| e.to_bytes()).collect();
                    prop_assert_eq!(&after[1..], &before[1..]);
                }

                let bytes = cst.to_bytes();
                let reread = MimeCst::parse(&bytes);
                prop_assert_eq!(reread.to_bytes(), bytes.clone());
                prop_assert_eq!(reread.decode(), cst.decode(), "{:?}", edit);
            }
        }
    }
}

#[test]
fn replaced_text_keeps_a_charset_that_holds_it_and_falls_back_to_utf8() {
    let message = "Content-Type: text/plain; charset=iso-8859-1\r\n\r\nold\r\n";

    let mut cst = MimeCst::parse(message.as_bytes());
    cst.replace_text(0, "café", &opts()).unwrap();
    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    assert_eq!(
        reread.root.content_type().param("charset").as_deref(),
        Some("iso-8859-1")
    );
    assert_eq!(reread.root.text().as_deref(), Some("café"));
    assert_eq!(reread.root.decoded().unwrap().as_ref(), b"caf\xe9");

    let mut cst = MimeCst::parse(message.as_bytes());
    cst.replace_text(0, "日本", &opts()).unwrap();
    let bytes = cst.to_bytes();
    let reread = MimeCst::parse(&bytes);
    assert_eq!(
        reread.root.content_type().param("charset").as_deref(),
        Some("utf-8")
    );
    assert_eq!(reread.root.text().as_deref(), Some("日本"));
}

/// The adversarial review's findings: each edit leaves a tree that reads
/// back as itself, entity for entity.
mod review {
    use std::time::Instant;

    use mime::{
        build::{MimeBuildError, MimeBuildOptions},
        header::marker::CONTENT_TYPE,
        mime::{Mime, MimeBody},
        tree::cst::MimeCst,
        value::content_type::MimeContentType,
    };

    fn opts() -> MimeBuildOptions {
        MimeBuildOptions::default()
    }

    fn assert_reads_back(cst: &MimeCst<'_>) {
        let bytes = cst.to_bytes();
        let reread = MimeCst::parse(&bytes);
        assert_eq!(reread.entities().len(), cst.entities().len());
        assert_eq!(reread.decode(), cst.decode());
    }

    #[test]
    fn replaced_content_cannot_smuggle_a_delimiter() {
        let message =
            "Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\r\none\r\n--b--\r\n";
        let mut cst = MimeCst::parse(message.as_bytes());
        cst.replace_content(1, b"x\r\n--b\r\nX-Injected: yes\r\n\r\nevil", None, &opts())
            .unwrap();
        assert_reads_back(&cst);
    }

    #[test]
    fn a_downgrade_cannot_open_a_delimiter_line() {
        let message = format!(
            "Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\né\r\n{}--b\r\nx\r\n--b\r\n\r\ntwo\r\n--b--\r\n",
            "a".repeat(75)
        );
        let mut cst = MimeCst::parse(message.as_bytes());
        cst.downgrade_7bit(&opts()).unwrap();
        assert_reads_back(&cst);
    }

    #[test]
    fn an_inserted_part_cannot_smuggle_an_enclosing_delimiter() {
        let message = "Content-Type: multipart/mixed; boundary=o\n\n--o\nContent-Type: multipart/mixed; boundary=i\n\n--i\n\none\n--i--\n--o--\n";
        let mut cst = MimeCst::parse(message.as_bytes());
        let part = Mime {
            headers: vec![],
            body: MimeBody::Text("x\n--o\nX-Evil: 1\n\nsmuggled".into()),
        };
        cst.insert_part(1, 9, &part, &opts()).unwrap();
        assert_reads_back(&cst);

        let mut cst = MimeCst::parse(message.as_bytes());
        cst.attach(2, &part, &opts()).unwrap();
        assert_reads_back(&cst);
    }

    #[test]
    fn tokens_holding_a_line_break_are_refused() {
        let mut builder = mime::builder::MimeBuilder::default();
        builder
            .header::<mime::header::marker::LIST_UNSUBSCRIBE>(vec![
                "https://x.test\r\nBcc: evil@x.test".into(),
            ])
            .unwrap();
        assert_eq!(builder.write(&opts()), Err(MimeBuildError::LineBreak));

        let mut builder = mime::builder::MimeBuilder::default();
        builder
            .attachment(mime::builder::MimeAttachment {
                content_type: MimeContentType {
                    top: "text".into(),
                    sub: "plain\r\nBcc: evil@x.test".into(),
                    params: vec![],
                },
                ..Default::default()
            })
            .unwrap();
        assert_eq!(builder.write(&opts()), Err(MimeBuildError::LineBreak));
    }

    #[test]
    fn field_edits_cannot_touch_the_structural_fields() {
        let signed = super::SIGNED;
        let mut cst = MimeCst::parse(signed.as_bytes());
        let header = cst.header_mut(0).unwrap();
        let content_type = MimeContentType {
            top: "multipart".into(),
            sub: "mixed".into(),
            params: vec![],
        };

        assert!(matches!(
            header.insert::<CONTENT_TYPE>(content_type.clone(), &opts()),
            Err(MimeBuildError::Reserved(_))
        ));
        assert!(matches!(
            header.set::<CONTENT_TYPE>(content_type, &opts()),
            Err(MimeBuildError::Reserved(_))
        ));
        assert!(header.remove_all("content-type").is_err());
        assert_eq!(cst.to_bytes(), signed.as_bytes());
    }

    #[test]
    fn writing_stays_linear_on_content_listing_generated_boundaries() {
        let text = |count: usize| {
            (0..count)
                .map(|i| format!("=_mime-rs_{i:016x}\n"))
                .collect::<String>()
        };
        let time = |count: usize| {
            let mime = Mime {
                headers: vec![],
                body: MimeBody::Multipart(vec![Mime {
                    headers: vec![],
                    body: MimeBody::Text(text(count)),
                }]),
            };
            let start = Instant::now();
            mime.to_bytes(&opts()).unwrap();
            start.elapsed().as_secs_f64()
        };

        let (small, large) = (time(4_000), time(8_000));
        assert!(
            large < small * 4.0 + 0.05,
            "doubling took {large}s against {small}s"
        );
    }

    #[test]
    fn a_downgrade_stays_linear_in_parts() {
        let message = |count: usize| {
            let mut message = String::from("Content-Type: multipart/mixed; boundary=b\r\n\r\n");

            for _ in 0..count {
                message.push_str("--b\r\nContent-Transfer-Encoding: 8bit\r\n\r\ncafé\r\n");
            }

            message + "--b--\r\n"
        };
        let time = |count: usize| {
            let message = message(count);
            let mut cst = MimeCst::parse(message.as_bytes());
            let start = Instant::now();
            cst.downgrade_7bit(&opts()).unwrap();
            start.elapsed().as_secs_f64()
        };

        let (small, large) = (time(1_000), time(2_000));
        assert!(
            large < small * 4.0 + 0.05,
            "doubling took {large}s against {small}s"
        );
    }
}

#[test]
fn a_field_ending_in_a_cr_keeps_it_when_a_field_follows() {
    let mut cst = MimeCst::parse(b"0:\n\x03:\r");
    let value = MimeHeaderValue::Raw(b" [\n ".to_vec());
    cst.header_mut(0)
        .unwrap()
        .insert_value(
            &mime::header::MimeHeaderName::new("X-Fuzz"),
            &value,
            &opts(),
        )
        .unwrap();

    let bytes = cst.to_bytes();
    assert_eq!(MimeCst::parse(&bytes).decode(), cst.decode());
}
