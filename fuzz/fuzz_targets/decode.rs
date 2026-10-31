#![no_main]

//! Coverage-guided fuzz target for the content codecs. Two oracles:
//! decoding arbitrary bytes never panics, and what the encoders write
//! decodes back to their input exactly.

use libfuzzer_sys::fuzz_target;
use mime::codec::{base64, charset, flowed, quoted_printable, rfc2047};

fuzz_target!(|data: &[u8]| {
    let mut out = Vec::new();
    base64::decode(data, &mut out);
    out.clear();
    quoted_printable::decode(data, &mut out);
    out.clear();
    quoted_printable::decode_q(data, &mut out);

    let _ = rfc2047::decode_text(data);
    let _ = charset::transcode_body(Some("iso-2022-jp"), data);
    let _ = charset::transcode("windows-1252", data);

    for eol in [&b"\r\n"[..], b"\n"] {
        let mut encoded = Vec::new();
        base64::encode(data, eol, &mut encoded);
        let mut decoded = Vec::new();
        assert!(base64::decode(&encoded, &mut decoded));
        assert_eq!(decoded, data, "base64");

        let mut encoded = Vec::new();
        quoted_printable::encode(data, false, eol, &mut encoded);
        let mut decoded = Vec::new();
        assert!(quoted_printable::decode(&encoded, &mut decoded));
        assert_eq!(decoded, data, "quoted-printable");
    }

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = flowed::decode(text, true);
        let _ = flowed::encode(text, 72, "\r\n");
    }
});
