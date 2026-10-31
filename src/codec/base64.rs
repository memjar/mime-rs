//! # Base64
//!
//! The RFC 2045 6.8 base64 transfer encoding, over the base64 crate.
//!
//! The crate is strict: it rejects a line break and every byte outside its
//! alphabet. A MIME body is wrapped at 76 characters, so the decoder runs
//! the crate line by line (a well-formed line is a whole number of
//! quanta), and falls back to one filtering pass when a line is not, which
//! is also where skipped bytes are reported.

use alloc::vec::Vec;

use base64::{
    Engine,
    alphabet::STANDARD,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

const ENGINE: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// Decodes a base64 body into `out`, returning whether the input was
/// well-formed.
///
/// Line breaks and blanks are skipped silently; any other byte outside the
/// alphabet is skipped and reported, and so are padding in the middle of
/// the data and a lone trailing symbol. Missing padding is accepted.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-6.8>
pub fn decode(input: &[u8], out: &mut Vec<u8>) -> bool {
    let start = out.len();

    if decode_lines(input, out) {
        return true;
    }

    out.truncate(start);
    decode_filtered(input, out)
}

/// Decodes the text of an RFC 2047 B encoded word into `out`, returning
/// whether it was well-formed.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2047#section-4.1>
pub fn decode_b(input: &[u8], out: &mut Vec<u8>) -> bool {
    decode(input, out)
}

/// Encodes `input` as base64 into `out`, in lines of 76 characters each
/// ended by `eol`, the last line ended too.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-6.8>
pub fn encode(input: &[u8], eol: &[u8], out: &mut Vec<u8>) {
    out.reserve(input.len().div_ceil(57) * (76 + eol.len()));

    for chunk in input.chunks(57) {
        let start = out.len();
        out.resize(start + chunk.len().div_ceil(3) * 4, 0);

        let written = ENGINE
            .encode_slice(chunk, &mut out[start..])
            .unwrap_or_default();

        out.truncate(start + written);
        out.extend_from_slice(eol);
    }
}

/// Encodes `input` as the unwrapped text of an RFC 2047 B encoded word.
pub fn encode_b(input: &[u8], out: &mut Vec<u8>) {
    let start = out.len();
    out.resize(start + input.len().div_ceil(3) * 4, 0);

    let written = ENGINE
        .encode_slice(input, &mut out[start..])
        .unwrap_or_default();

    out.truncate(start + written);
}

/// The fast path: every line a whole number of quanta, padding only at the
/// very end.
fn decode_lines(input: &[u8], out: &mut Vec<u8>) -> bool {
    let mut padded = false;

    for line in input.split(|byte| *byte == b'\n') {
        let line = trim_blanks(line);

        if line.is_empty() {
            continue;
        }

        if padded || !line.len().is_multiple_of(4) {
            return false;
        }

        padded = line.ends_with(b"=");

        let start = out.len();
        out.resize(start + line.len() / 4 * 3, 0);

        match ENGINE.decode_slice(line, &mut out[start..]) {
            Ok(written) => out.truncate(start + written),
            Err(_) => return false,
        }
    }

    true
}

/// The slow path: keep the alphabet, drop the rest, decode once.
fn decode_filtered(input: &[u8], out: &mut Vec<u8>) -> bool {
    let mut clean = true;
    let mut symbols = Vec::with_capacity(input.len());
    let mut padding = false;

    for &byte in input {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' => {
                if padding {
                    // NOTE: data after padding, as concatenated encoders
                    // write it; the quantum before the padding is complete.
                    clean = false;
                    padding = false;
                    flush(&mut symbols, out);
                }

                symbols.push(byte);
            }
            b'=' => padding = true,
            b' ' | b'\t' | b'\r' | b'\n' => {}
            _ => clean = false,
        }
    }

    if symbols.len() % 4 == 1 {
        clean = false;
        symbols.pop();
    }

    flush(&mut symbols, out);
    clean
}

fn flush(symbols: &mut Vec<u8>, out: &mut Vec<u8>) {
    if symbols.len() % 4 == 1 {
        symbols.pop();
    }

    let start = out.len();
    out.resize(start + symbols.len().div_ceil(4) * 3, 0);

    let written = ENGINE
        .decode_slice(symbols.as_slice(), &mut out[start..])
        .unwrap_or_default();

    out.truncate(start + written);
    symbols.clear();
}

fn trim_blanks(line: &[u8]) -> &[u8] {
    let blank = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\r');
    let start = line
        .iter()
        .position(|byte| !blank(byte))
        .unwrap_or(line.len());
    let end = line
        .iter()
        .rposition(|byte| !blank(byte))
        .map_or(start, |end| end + 1);

    &line[start..end]
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use crate::codec::base64::*;

    fn decoded(input: &[u8]) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let clean = decode(input, &mut out);
        (out, clean)
    }

    #[test]
    fn wrapped_lines_decode_on_the_fast_path() {
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
    fn encoding_wraps_at_76_and_reads_back() {
        let input: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        let mut encoded = Vec::new();
        encode(&input, b"\r\n", &mut encoded);

        assert!(encoded.split(|b| *b == b'\n').all(|line| line.len() <= 77));
        assert_eq!(decoded(&encoded), (input, true));
    }
}
