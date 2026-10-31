//! Builds a message strictly: typed fields through the markers, a text
//! and an HTML body, an inline image and an attachment, laid out as
//! mail-builder lays them out, with no clock and no randomness.

use mime::{
    build::MimeBuildOptions,
    builder::{MimeAttachment, MimeBuilder},
    header::marker::{DATE, FROM, MESSAGE_ID, SUBJECT, TO},
    value::{
        address::MimeMailbox, content_type::MimeContentType, datetime::MimeDateTime,
        message_id::MimeMessageId,
    },
};

fn main() {
    let mut builder = MimeBuilder::default();

    builder
        .header::<DATE>(MimeDateTime::from_unix_seconds(1_793_448_000, 60))
        .unwrap()
        .header::<FROM>(MimeMailbox::named("Jöhn Doe", "john@example.com").into())
        .unwrap()
        .header::<TO>(MimeMailbox::address("jane@example.com").into())
        .unwrap()
        .header::<SUBJECT>("Rapport trimestriel".into())
        .unwrap()
        .header::<MESSAGE_ID>(vec![
            MimeMessageId::new("1793448000.1", "example.com")
                .unwrap()
                .into(),
        ])
        .unwrap()
        .text("Bonjour,\nle rapport est joint.\n")
        .html("<p>Bonjour,</p><p><img src=\"cid:logo@example.com\"></p>");

    builder
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
        .unwrap()
        .attachment(MimeAttachment {
            content_type: MimeContentType {
                top: "application".into(),
                sub: "pdf".into(),
                params: vec![],
            },
            filename: Some("Rapport été.pdf".into()),
            content: b"%PDF-1.7".to_vec(),
            ..Default::default()
        })
        .unwrap();

    // NOTE: a field given a value of another form is refused.
    assert!(
        builder
            .header_value(
                mime::header::MimeHeaderKind::Date,
                mime::value::MimeHeaderValue::Text("today".into()),
            )
            .is_err()
    );

    let bytes = builder.write(&MimeBuildOptions::default()).unwrap();
    println!("{}", String::from_utf8_lossy(&bytes));
}
