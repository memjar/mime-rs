//! Builds a reply (In-Reply-To, References, the base subject) and a
//! forward attaching the original as `message/rfc822`.

use mime::{
    build::MimeBuildOptions,
    builder::MimeBuilder,
    header::{
        MimeHeader, MimeHeaderName,
        marker::{FROM, IN_REPLY_TO, MESSAGE_ID, REFERENCES, SUBJECT, TO},
    },
    mime::{Mime, MimeBody},
    tree::cst::MimeCst,
    value::{MimeHeaderValue, content_type::MimeContentType, subject::base_subject},
};

fn main() {
    let original = concat!(
        "From: Alice <alice@example.com>\r\n",
        "To: bob@example.com\r\n",
        "Subject: Re: Fwd: Lunch on Friday\r\n",
        "Message-ID: <2@example.com>\r\n",
        "References: <1@example.com>\r\n",
        "\r\n",
        "Noon?\r\n",
    );
    let cst = MimeCst::parse(original.as_bytes());
    let root = &cst.root;

    let subject = root.header::<SUBJECT>().unwrap_or_default();
    let id = root.header::<MESSAGE_ID>().unwrap_or_default();
    let mut references = root.header::<REFERENCES>().unwrap_or_default();
    references.extend(id.clone());

    let mut reply = MimeBuilder::default();
    reply
        .header::<TO>(root.header::<FROM>().unwrap())
        .unwrap()
        .header::<SUBJECT>(format!("Re: {}", base_subject(&subject)))
        .unwrap()
        .header::<IN_REPLY_TO>(id)
        .unwrap()
        .header::<REFERENCES>(references)
        .unwrap()
        .text("> Noon?\nNoon is fine.\n");
    println!(
        "{}",
        String::from_utf8_lossy(&reply.write(&MimeBuildOptions::default()).unwrap())
    );

    let mut forward = MimeBuilder::default();
    forward
        .header::<SUBJECT>(format!("Fwd: {}", base_subject(&subject)))
        .unwrap()
        .text("See below.\n")
        .part(Mime {
            headers: vec![MimeHeader {
                name: MimeHeaderName::new("Content-Type"),
                value: MimeHeaderValue::ContentType(MimeContentType {
                    top: "message".into(),
                    sub: "rfc822".into(),
                    params: vec![],
                }),
            }],
            body: MimeBody::Message(Box::new(cst.decode())),
        });
    println!(
        "{}",
        String::from_utf8_lossy(&forward.write(&MimeBuildOptions::default()).unwrap())
    );
}
