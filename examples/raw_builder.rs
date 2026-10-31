//! Writes a model assembled by hand, with raw fields and a raw part as the
//! escape hatch, under options for a local store: LF line endings and
//! 8-bit text.

use mime::{
    build::{MimeBuildOptions, MimeLineEnding},
    header::{MimeHeader, MimeHeaderName},
    mime::{Mime, MimeBody},
    value::MimeHeaderValue,
};

fn main() {
    let mime = Mime {
        headers: vec![
            MimeHeader {
                name: MimeHeaderName::new("X-Vendor-Thing"),
                value: MimeHeaderValue::Raw(b" a; b\n  (folded as given)".to_vec()),
            },
            MimeHeader {
                name: MimeHeaderName::new("Subject"),
                value: MimeHeaderValue::Text("Grüße".into()),
            },
        ],
        body: MimeBody::Text("Schöne Grüße\n".into()),
    };

    let opts = MimeBuildOptions {
        line_ending: MimeLineEnding::Lf,
        allow_8bit: true,
        ..Default::default()
    };

    let bytes = mime.to_bytes(&opts).unwrap();
    println!("{}", String::from_utf8_lossy(&bytes));
}
