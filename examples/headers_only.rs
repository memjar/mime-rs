//! Reads the header block of a message alone: the parse stops at it, so
//! its cost follows the header block whatever the attachments weigh, and
//! a value standing alone reads in any form.

use mime::{
    header::{marker::SUBJECT, spec::MimeHeaderForm},
    tree::{cst::MimeCst, parse::MimeParseOptions},
};

fn main() {
    let mut raw = String::from(concat!(
        "Subject: Big one\r\n",
        "Content-Type: multipart/mixed; boundary=b\r\n",
        "\r\n",
        "--b\r\n",
        "Content-Type: application/octet-stream\r\n",
        "Content-Transfer-Encoding: base64\r\n",
        "\r\n",
    ));
    raw.push_str(&"QUJDRA==\r\n".repeat(100_000));
    raw.push_str("--b--\r\n");

    let opts = MimeParseOptions {
        structure_depth: Some(0),
        ..Default::default()
    };
    let mut cst = MimeCst::parse_with(raw.as_bytes(), opts);
    println!("Subject: {:?}", cst.root.header::<SUBJECT>());

    // NOTE: the body is split only when asked.
    cst.expand(MimeParseOptions::default());
    println!("entities: {}", cst.entities().len());

    let value = MimeHeaderForm::GroupedAddresses.parse(b" Friends: a@x.test, b@x.test;");
    println!("standing alone: {value:?}");
}
