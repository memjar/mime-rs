//! # Character sets
//!
//! Transcoding text from the charset a label names to UTF-8, the way vcard
//! and ical do it: the core transcodes nothing, and the `encoding` feature
//! goes through `encoding_rs`, falling back to UTF-8 for a label it does not
//! know.

use alloc::{borrow::Cow, string::String};

/// The outcome of transcoding one run of bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimeCharsetText<'b> {
    /// The text, lossily decoded where a byte did not fit the charset.
    pub text: Cow<'b, str>,
    /// Whether the label named a charset the build knows.
    ///
    /// An unknown one decodes as UTF-8.
    pub known: bool,
    /// Whether some bytes did not fit the charset and were replaced.
    pub malformed: bool,
}

/// Whether `label` names UTF-8 or US-ASCII, read even without `encoding`.
pub fn is_utf8_label(label: &str) -> bool {
    let label = clean_label(label);

    [
        "utf-8",
        "utf8",
        "us-ascii",
        "ascii",
        "us_ascii",
        "ansi_x3.4-1968",
    ]
    .iter()
    .any(|known| label.eq_ignore_ascii_case(known))
}

/// Strips a label's quotes, blanks and RFC 2231 language suffix (`utf-8*en`).
pub fn clean_label(label: &str) -> &str {
    let label = label.trim().trim_matches('"').trim();
    label.split('*').next().unwrap_or(label)
}

/// Transcodes `bytes` from the charset `label` names.
///
/// Without `encoding`, only UTF-8 and US-ASCII decode; others give `None` so
/// the caller keeps the bytes rather than guess. With it, WHATWG labels decode
/// (BOM ignored) and an unknown label decodes as UTF-8 with `known` false.
pub fn transcode<'b>(label: &str, bytes: &'b [u8]) -> Option<MimeCharsetText<'b>> {
    if is_utf8_label(label) {
        return Some(utf8(bytes, true));
    }

    transcode_other(label, bytes)
}

/// Transcodes from a charset other than UTF-8 and US-ASCII.
#[cfg(feature = "encoding")]
fn transcode_other<'b>(label: &str, bytes: &'b [u8]) -> Option<MimeCharsetText<'b>> {
    let text = match encoding_rs::Encoding::for_label(clean_label(label).as_bytes()) {
        Some(encoding) => {
            let (text, malformed) = encoding.decode_without_bom_handling(bytes);
            MimeCharsetText {
                text,
                known: true,
                malformed,
            }
        }
        None => utf8(bytes, false),
    };

    Some(text)
}

/// Transcodes from a charset other than UTF-8 and US-ASCII: never.
#[cfg(not(feature = "encoding"))]
fn transcode_other<'b>(_label: &str, _bytes: &'b [u8]) -> Option<MimeCharsetText<'b>> {
    None
}

/// Transcodes a body from the charset `label` names, or UTF-8 with no label.
///
/// A byte order mark at its start takes precedence. Without the `encoding`
/// feature this is [`transcode`] with the UTF-8 fallback for an absent label.
pub fn transcode_body<'b>(label: Option<&str>, bytes: &'b [u8]) -> Option<MimeCharsetText<'b>> {
    if let Some(text) = transcode_bom(bytes) {
        return Some(text);
    }

    transcode(label.unwrap_or("utf-8"), bytes)
}

/// Transcodes a body from the charset its leading byte order mark names.
#[cfg(feature = "encoding")]
fn transcode_bom(bytes: &[u8]) -> Option<MimeCharsetText<'_>> {
    let (encoding, bom) = encoding_rs::Encoding::for_bom(bytes)?;
    let (text, malformed) = encoding.decode_without_bom_handling(&bytes[bom..]);

    Some(MimeCharsetText {
        text,
        known: true,
        malformed,
    })
}

/// Transcodes a body from its byte order mark: never, the mark is not read.
#[cfg(not(feature = "encoding"))]
fn transcode_bom(_bytes: &[u8]) -> Option<MimeCharsetText<'_>> {
    None
}

/// Encodes `text` in the charset `label` names, `None` when it cannot.
///
/// The build may not know the charset, or the charset not hold every
/// character. Without `encoding` only UTF-8 and US-ASCII (ASCII text only)
/// encode; with it, every charset the WHATWG standard can write.
pub fn encode<'t>(label: &str, text: &'t str) -> Option<Cow<'t, [u8]>> {
    if is_utf8_label(label) {
        let ascii = !clean_label(label).to_ascii_lowercase().contains("utf");
        return (!ascii || text.is_ascii()).then_some(Cow::Borrowed(text.as_bytes()));
    }

    encode_other(label, text)
}

/// Encodes in a charset other than UTF-8 and US-ASCII.
#[cfg(feature = "encoding")]
fn encode_other<'t>(label: &str, text: &'t str) -> Option<Cow<'t, [u8]>> {
    let encoding = encoding_rs::Encoding::for_label(clean_label(label).as_bytes())?;
    let (bytes, used, unmappable) = encoding.encode(text);

    (used == encoding && !unmappable).then_some(bytes)
}

/// Encodes in a charset other than UTF-8 and US-ASCII: never.
#[cfg(not(feature = "encoding"))]
fn encode_other<'t>(_label: &str, _text: &'t str) -> Option<Cow<'t, [u8]>> {
    None
}

fn utf8(bytes: &[u8], known: bool) -> MimeCharsetText<'_> {
    // NOTE: core's validation takes the ASCII fast path the lossy decoder's
    // byte-by-byte chunking lacks, so it runs first.
    let text = match core::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => String::from_utf8_lossy(bytes),
    };
    let malformed = matches!(text, Cow::Owned(_));

    MimeCharsetText {
        text,
        known,
        malformed,
    }
}
