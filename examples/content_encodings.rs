//! The content codecs used alone, outside any message: quoted-printable,
//! base64, RFC 2047 encoded words, RFC 2231 parameters and flowed text.

use std::borrow::Cow;

use mime::codec::{base64, flowed, quoted_printable, rfc2047, rfc2231};

fn main() {
    let mut out = Vec::new();
    let clean = quoted_printable::decode(b"caf=C3=A9 cr=\r\n=C3=A8me", &mut out);
    println!(
        "quoted-printable: {:?} (clean: {clean})",
        String::from_utf8_lossy(&out)
    );

    let mut out = Vec::new();
    base64::encode(b"hello, world", b"\r\n", &mut out);
    println!("base64: {:?}", String::from_utf8_lossy(&out));

    println!(
        "encoded words: {:?}",
        rfc2047::decode_text(b"=?utf-8?q?caf=C3=A9?= au lait")
    );

    let params = rfc2231::join(vec![
        rfc2231::MimeRawParam::new(b"name*0*", Cow::Borrowed(b"utf-8''%E2%98%95")),
        rfc2231::MimeRawParam::new(b"name*1", Cow::Borrowed(b".txt")),
    ]);
    println!("parameter: {:?}", params[0].value);

    println!("flowed: {:?}", flowed::decode("Hello \r\nworld\r\n", false));
}
