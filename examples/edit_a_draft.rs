//! Edits a draft in place: its Subject set, a recipient appended, an
//! attachment added. Every byte the edits do not name is kept, and every
//! leaf they do not write still borrows the input.

use mime::{
    build::MimeBuildOptions,
    header::{MimeHeader, MimeHeaderName, marker::SUBJECT},
    mime::{Mime, MimeBody},
    tree::cst::MimeCst,
    value::{
        MimeHeaderValue,
        address::{MimeAddress, MimeMailbox},
        content_type::MimeContentType,
    },
};

fn main() {
    let draft = concat!(
        "From: Alice <alice@example.com>\r\n",
        "To: \"Doe, J.\" <j@example.com>,\r\n",
        "  (team) team@example.com\r\n",
        "Subject: Draft\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "\r\n",
        "Hello\r\n",
    );

    let opts = MimeBuildOptions::default();
    let mut cst = MimeCst::parse(draft.as_bytes());

    let header = cst.header_mut(0).unwrap();
    header.set::<SUBJECT>("Final".into(), &opts).unwrap();
    header
        .push_address(
            "To",
            &MimeAddress::Mailbox(MimeMailbox::address("k@example.com")),
            &opts,
        )
        .unwrap();

    let attachment = Mime {
        headers: vec![MimeHeader {
            name: MimeHeaderName::new("Content-Type"),
            value: MimeHeaderValue::ContentType(MimeContentType {
                top: "application".into(),
                sub: "pdf".into(),
                params: vec![],
            }),
        }],
        body: MimeBody::Binary(b"%PDF-1.7".to_vec()),
    };
    cst.attach(0, &attachment, &opts).unwrap();

    println!("{}", String::from_utf8_lossy(&cst.to_bytes()));
}
