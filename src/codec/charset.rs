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
    /// Whether the label named a charset the build knows. An unknown one
    /// decodes as UTF-8.
    pub known: bool,
    /// Whether some bytes did not fit the charset and were replaced.
    pub malformed: bool,
}

/// Whether `label` names UTF-8 or US-ASCII, the two charsets the core reads
/// without the `encoding` feature.
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

/// Strips the quotes, blanks and RFC 2231 language suffix (`utf-8*en`) a
/// label may carry.
pub fn clean_label(label: &str) -> &str {
    let label = label.trim().trim_matches('"').trim();
    label.split('*').next().unwrap_or(label)
}

/// Transcodes `bytes` from the charset `label` names.
///
/// Without the `encoding` feature, only a UTF-8 or US-ASCII label decodes,
/// and any other returns `None`, so the caller keeps the bytes as written
/// rather than guess. With it, every WHATWG label decodes, a byte order mark
/// is not looked at, and an unknown label decodes as UTF-8 with `known`
/// false.
pub fn transcode<'b>(label: &str, bytes: &'b [u8]) -> Option<MimeCharsetText<'b>> {
    if is_utf8_label(label) {
        return Some(utf8(bytes, true));
    }

    #[cfg(feature = "encoding")]
    let text = Some(
        match encoding_rs::Encoding::for_label(clean_label(label).as_bytes()) {
            Some(encoding) => {
                let (text, malformed) = encoding.decode_without_bom_handling(bytes);
                MimeCharsetText {
                    text,
                    known: true,
                    malformed,
                }
            }
            None => utf8(bytes, false),
        },
    );
    #[cfg(not(feature = "encoding"))]
    let text = None;

    text
}

/// Transcodes a body from the charset `label` names, a byte order mark at
/// its start taking precedence; an absent label reads as UTF-8.
///
/// Without the `encoding` feature this is [`transcode`] with the UTF-8
/// fallback for an absent label.
pub fn transcode_body<'b>(label: Option<&str>, bytes: &'b [u8]) -> Option<MimeCharsetText<'b>> {
    #[cfg(feature = "encoding")]
    if let Some((encoding, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, malformed) = encoding.decode_without_bom_handling(&bytes[bom..]);
        return Some(MimeCharsetText {
            text,
            known: true,
            malformed,
        });
    }

    transcode(label.unwrap_or("utf-8"), bytes)
}

fn utf8(bytes: &[u8], known: bool) -> MimeCharsetText<'_> {
    let text = String::from_utf8_lossy(bytes);
    let malformed = matches!(text, Cow::Owned(_));

    MimeCharsetText {
        text,
        known,
        malformed,
    }
}
