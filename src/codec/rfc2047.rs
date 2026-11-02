//! # Encoded words
//!
//! RFC 2047 encoded words (`=?charset?B?...?=` and `=?charset?Q?...?=`) in
//! unstructured text and phrases.
//!
//! Decoding is liberal, as real mailers write: a word glued to the text
//! around it still decodes, a fold inside a word is dropped, adjacent words
//! of one charset are joined before transcoding so a character split across
//! two words survives, and blanks between two words are dropped (RFC 2047
//! 6.2). A word that does not parse, whose encoding is not built in, or
//! whose charset cannot be transcoded stays as written.

use alloc::{borrow::Cow, string::String, vec::Vec};

use crate::codec::{charset, quoted_printable};

/// Decodes an unstructured value: encoded words decoded, every run of
/// blanks holding a fold collapsed to one space, the blanks at its ends
/// trimmed; the blanks an encoded word decodes to are text and stay.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2047#section-6.2>
pub fn decode_text(raw: &[u8]) -> Cow<'_, str> {
    let raw = trim(raw);

    if !contains_word_start(raw) && !raw.iter().any(|byte| matches!(byte, b'\r' | b'\n')) {
        return match core::str::from_utf8(raw) {
            Ok(text) => Cow::Borrowed(text),
            Err(_) => Cow::Owned(String::from_utf8_lossy(raw).into_owned()),
        };
    }

    let mut out = String::with_capacity(raw.len());
    let mut pending: Option<(&[u8], Vec<u8>)> = None;
    let mut literal_start = 0;
    let mut finder = WordEnd::default();
    let mut i = 0;

    while i + 1 < raw.len() {
        if raw[i] != b'=' || raw[i + 1] != b'?' {
            i += 1;
            continue;
        }

        let Some(word) = parse_word(raw, i, &mut finder) else {
            i += 1;
            continue;
        };

        let between = &raw[literal_start..i];
        let joins = pending.is_some() && between.iter().all(|byte| is_blank(*byte));

        if !joins {
            flush(&mut pending, &mut out);
            push_literal(between, &mut out);
        }

        match &mut pending {
            Some((label, bytes)) if label.eq_ignore_ascii_case(word.charset) => {
                bytes.extend_from_slice(&word.bytes);
            }
            _ => {
                flush(&mut pending, &mut out);
                pending = Some((word.charset, word.bytes));
            }
        }

        i = word.end;
        literal_start = word.end;
    }

    flush(&mut pending, &mut out);
    push_literal(&raw[literal_start..], &mut out);
    Cow::Owned(out)
}

/// Decodes one encoded word standing alone (a parameter value written as an
/// encoded word, or a phrase word), or `None` when it is not one.
pub fn decode_word(raw: &[u8]) -> Option<String> {
    let mut finder = WordEnd::default();
    let word = parse_word(raw, 0, &mut finder)?;
    let mut pending = Some((word.charset, word.bytes));
    let mut out = String::new();

    flush(&mut pending, &mut out);
    Some(out)
}

/// The end of the encoded word starting at `start` in `raw`, when one does
/// and decodes in this build.
pub fn word_end(raw: &[u8], start: usize) -> Option<usize> {
    let mut finder = WordEnd::default();
    parse_word(raw, start, &mut finder).map(|word| word.end)
}

/// Whether `raw` holds a `=?`, the only way an encoded word can start.
pub fn contains_word_start(raw: &[u8]) -> bool {
    let mut rest = raw;

    while let Some(at) = rest.iter().position(|byte| *byte == b'=') {
        if rest.get(at + 1) == Some(&b'?') {
            return true;
        }

        rest = &rest[at + 1..];
    }

    false
}

struct Word<'r> {
    charset: &'r [u8],
    bytes: Vec<u8>,
    end: usize,
}

/// The position of the next `?=`, cached so that a value full of word
/// starts with no end is read once, not once per start.
#[derive(Default)]
struct WordEnd {
    from: usize,
    at: Option<Option<usize>>,
}

impl WordEnd {
    fn find(&mut self, raw: &[u8], from: usize) -> Option<usize> {
        if let Some(found) = self.at
            && from >= self.from
            && found.is_none_or(|at| at >= from)
        {
            return found;
        }

        let found = raw
            .get(from..)
            .and_then(|tail| tail.windows(2).position(|pair| pair == b"?="))
            .map(|at| from + at);

        self.from = from;
        self.at = Some(found);
        found
    }
}

fn parse_word<'r>(raw: &'r [u8], start: usize, finder: &mut WordEnd) -> Option<Word<'r>> {
    let rest = raw.get(start + 2..)?;
    let charset_len = rest.iter().position(|byte| *byte == b'?')?;
    let charset = &rest[..charset_len];

    // NOTE: a charset is an ASCII token (RFC 2047 2), dots let through as
    // `ANSI_X3.4-1968` is written.
    if charset.is_empty() || !charset.iter().all(u8::is_ascii_graphic) {
        return None;
    }

    let encoding = *rest.get(charset_len + 1)?;

    if rest.get(charset_len + 2) != Some(&b'?') {
        return None;
    }

    let text_start = start + 2 + charset_len + 3;
    let text_end = finder.find(raw, text_start)?;
    let text = unfold_word(&raw[text_start..text_end]);
    let mut bytes = Vec::with_capacity(text.len());

    let decoded = match encoding {
        b'Q' | b'q' => quoted_printable::decode_q(&text, &mut bytes),
        #[cfg(feature = "base64")]
        b'B' | b'b' => crate::codec::base64::decode_b(&text, &mut bytes),
        _ => false,
    };

    let label = core::str::from_utf8(charset).ok()?;

    if !decoded || charset::transcode(label, &[]).is_none() {
        return None;
    }

    Some(Word {
        charset,
        bytes,
        end: text_end + 2,
    })
}

fn flush(pending: &mut Option<(&[u8], Vec<u8>)>, out: &mut String) {
    let Some((label, bytes)) = pending.take() else {
        return;
    };

    let label = core::str::from_utf8(label).unwrap_or("utf-8");

    if let Some(decoded) = charset::transcode(label, &bytes) {
        out.push_str(&decoded.text);
    }
}

fn push_literal(bytes: &[u8], out: &mut String) {
    let mut i = 0;

    while i < bytes.len() {
        if is_blank(bytes[i]) {
            let start = i;

            while i < bytes.len() && is_blank(bytes[i]) {
                i += 1;
            }

            let run = &bytes[start..i];

            if run.iter().any(|byte| matches!(byte, b'\r' | b'\n')) {
                out.push(' ');
            } else {
                out.push_str(core::str::from_utf8(run).unwrap_or(" "));
            }

            continue;
        }

        let start = i;

        while i < bytes.len() && !is_blank(bytes[i]) {
            i += 1;
        }

        out.push_str(&String::from_utf8_lossy(&bytes[start..i]));
    }
}

/// Drops every fold of an encoded word's text: the line break and every
/// blank after it.
fn unfold_word(text: &[u8]) -> Cow<'_, [u8]> {
    if !text.contains(&b'\n') {
        return Cow::Borrowed(text);
    }

    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;

    while i < text.len() {
        match text[i] {
            b'\r' if text.get(i + 1) == Some(&b'\n') => i += 1,
            b'\n' => {
                i += 1;

                while matches!(text.get(i), Some(b' ' | b'\t')) {
                    i += 1;
                }
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }

    Cow::Owned(out)
}

fn is_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !is_blank(*byte))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !is_blank(*byte))
        .map_or(start, |end| end + 1);

    &bytes[start..end]
}

#[cfg(test)]
mod tests {
    use crate::codec::rfc2047::*;

    #[test]
    fn adjacent_words_join_and_folds_collapse() {
        assert_eq!(decode_text(b"=?utf-8?q?a?=  \r\n  =?utf-8?q?b?="), "ab");
        assert_eq!(decode_text(b"=?utf-8?q?a?=\r\n b"), "a b");
        assert_eq!(decode_text(b"x   =?utf-8?q?a?=   y"), "x   a   y");
        assert_eq!(
            decode_text(b" Fwd: \n\tSaying \n Hello"),
            "Fwd: Saying Hello"
        );
    }

    #[test]
    fn a_charset_outside_ascii_makes_no_word() {
        assert_eq!(decode_text(b"=?\xc7\x93?b??="), "=?\u{1d3}?b??=");
    }

    #[test]
    fn the_blanks_a_word_decodes_to_are_kept() {
        assert_eq!(decode_text(b" =?utf-8?q?_lead?=\r\n "), " lead");
        assert_eq!(decode_text(b"=?utf-8?q?a?= =?utf-8?q?trail_?="), "atrail ");
    }

    #[test]
    fn a_character_split_across_two_words_survives() {
        assert_eq!(decode_text(b"=?utf-8?q?caf=C3?= =?UTF-8?q?=A9?="), "café");
    }

    #[test]
    fn broken_words_stay_literal() {
        assert_eq!(
            decode_text(b" let's = try =?iso-8859-1? to break\n =? the \n parser"),
            "let's = try =?iso-8859-1? to break =? the parser"
        );
        assert_eq!(decode_text(b"Some text =?utf-8?Q??=here"), "Some text here");
    }

    #[test]
    fn a_fold_inside_a_word_is_dropped() {
        assert_eq!(
            decode_text(b"=?utf-8?Q?Hello\r\n   \t  _there!?="),
            "Hello there!"
        );
    }
}
