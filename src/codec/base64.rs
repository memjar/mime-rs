//! # Base64
//!
//! The RFC 2045 6.8 base64 transfer encoding and the RFC 2047 4.1 B
//! encoding of encoded words, over the base64-codec crate.
//!
//! The crate decodes as mail needs: line breaks and blanks are skipped,
//! any other byte outside the alphabet is skipped and reported, and so are
//! data after padding and a lone trailing symbol. Bodies run the backend of
//! the token their caller gives, encoded words the one the application
//! installed.

use alloc::vec::Vec;

use base64_codec::{
    codec::{Base64Codec, Base64Eol},
    cpu::Base64Cpu,
};
use simd_cpu::SimdCpu;

/// Decodes a base64 body into `out` under `cpu`, returning whether the
/// input was well-formed.
///
/// Line breaks and blanks are skipped silently; any other byte outside the
/// alphabet is skipped and reported, and so are padding in the middle of
/// the data and a lone trailing symbol. Missing padding is accepted.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-6.8>
pub fn decode(cpu: SimdCpu, input: &[u8], out: &mut Vec<u8>) -> bool {
    codec(cpu, None).decode_vec(input, out)
}

/// Decodes the text of an RFC 2047 B encoded word into `out` under
/// [`SimdCpu::current`], returning whether it was well-formed.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2047#section-4.1>
pub fn decode_b(input: &[u8], out: &mut Vec<u8>) -> bool {
    Base64Codec::default().decode_vec(input, out)
}

/// Encodes `input` as base64 into `out` under `cpu`, in lines of 76
/// characters each ended by `eol` (CRLF unless it is a bare LF), the last
/// line ended too.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-6.8>
pub fn encode(cpu: SimdCpu, input: &[u8], eol: &[u8], out: &mut Vec<u8>) {
    let eol = match eol {
        b"\n" => Base64Eol::Lf,
        _ => Base64Eol::Crlf,
    };

    codec(cpu, Some(eol)).encode_vec(input, out);
}

/// Encodes `input` as the unwrapped text of an RFC 2047 B encoded word under
/// [`SimdCpu::current`].
pub fn encode_b(input: &[u8], out: &mut Vec<u8>) {
    Base64Codec::default().encode_vec(input, out);
}

fn codec(cpu: SimdCpu, wrap: Option<Base64Eol>) -> Base64Codec {
    Base64Codec {
        cpu: Base64Cpu::new(cpu),
        wrap,
        ..Base64Codec::BASELINE
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use simd_cpu::SimdCpu;

    use crate::codec::base64::*;

    fn decoded(input: &[u8]) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let clean = decode(SimdCpu::BASELINE, input, &mut out);
        (out, clean)
    }

    #[test]
    fn wrapped_lines_decode_clean() {
        assert_eq!(
            decoded(b"AAEC\r\nAwQF\r\n"),
            (b"\x00\x01\x02\x03\x04\x05".to_vec(), true)
        );
        assert_eq!(decoded(b"aGk=\n"), (b"hi".to_vec(), true));
    }

    #[test]
    fn a_stray_byte_is_skipped_and_reported() {
        assert_eq!(
            decoded(b"AAEC*AwQF"),
            (b"\x00\x01\x02\x03\x04\x05".to_vec(), false)
        );
    }

    #[test]
    fn missing_padding_and_odd_lines_still_decode() {
        assert_eq!(decoded(b"aGk"), (b"hi".to_vec(), true));
        assert_eq!(decoded(b"aG\nk=\n"), (b"hi".to_vec(), true));
    }

    #[test]
    fn data_after_padding_is_a_second_run() {
        assert_eq!(decoded(b"aGk=aGk="), (b"hihi".to_vec(), false));
    }

    #[test]
    fn decoding_appends_to_the_output() {
        let mut out = b"x".to_vec();
        assert!(decode(SimdCpu::BASELINE, b"aGk=", &mut out));
        assert!(decode_b(b"aGk=", &mut out));
        assert_eq!(out, b"xhihi");
    }

    #[test]
    fn encoding_wraps_at_76_and_reads_back() {
        let input: Vec<u8> = (0..=255u8).cycle().take(1000).collect();

        for eol in [&b"\r\n"[..], b"\n"] {
            let mut encoded = Vec::new();
            encode(SimdCpu::BASELINE, &input, eol, &mut encoded);

            assert!(encoded.ends_with(eol));
            assert!(encoded.split(|b| *b == b'\n').all(|line| line.len() <= 77));
            assert_eq!(decoded(&encoded), (input.clone(), true));
        }
    }

    #[test]
    fn b_encoding_is_one_padded_run() {
        let mut out = Vec::new();
        encode_b(b"hi", &mut out);
        assert_eq!(out, b"aGk=");
    }
}
