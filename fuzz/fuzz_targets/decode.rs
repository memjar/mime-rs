#![no_main]

//! Coverage-guided fuzz target for the content codecs. Two oracles:
//! decoding arbitrary bytes never panics, and what the encoders write
//! decodes back to their input exactly.

use libfuzzer_sys::fuzz_target;
use mime::codec::{base64, charset, flowed, quoted_printable, rfc2047};
use simd_cpu::SimdCpu;

fuzz_target!(|data: &[u8]| {
    let cpu = SimdCpu::detect();
    let mut out = Vec::new();
    base64::decode(cpu, data, &mut out);
    out.clear();
    quoted_printable::decode(cpu, data, &mut out);
    out.clear();
    quoted_printable::decode_q(data, &mut out);

    let _ = rfc2047::decode_text(data);
    let _ = charset::transcode_body(Some("iso-2022-jp"), data);
    let _ = charset::transcode("windows-1252", data);

    for eol in [&b"\r\n"[..], b"\n"] {
        let mut encoded = Vec::new();
        base64::encode(cpu, data, eol, &mut encoded);
        let mut decoded = Vec::new();
        assert!(base64::decode(cpu, &encoded, &mut decoded));
        assert_eq!(decoded, data, "base64");

        let mut encoded = Vec::new();
        quoted_printable::encode(cpu, data, false, eol, &mut encoded);
        let mut decoded = Vec::new();
        assert!(quoted_printable::decode(cpu, &encoded, &mut decoded));
        assert_eq!(decoded, data, "quoted-printable");

        let mut encoded = Vec::new();
        quoted_printable::encode(cpu, data, true, eol, &mut encoded);
        let mut decoded = Vec::new();
        assert!(quoted_printable::decode(cpu, &encoded, &mut decoded));
        assert!(!encoded.split(|byte| *byte == b'\n').any(|line| line.starts_with(b"-")));
    }

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = flowed::decode(text, true);
        let _ = flowed::encode(text, 72, "\r\n");
    }
});
