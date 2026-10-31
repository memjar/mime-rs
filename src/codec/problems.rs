//! # Decode problems
//!
//! What a checked decoding met on its way. Decoding never fails: it always
//! returns content, and these say how far that content can be trusted.

/// The problems one decoding met.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeDecodeProblems {
    /// The transfer encoding held bytes it does not allow, skipped or kept
    /// literal.
    pub malformed_encoding: bool,
    /// The transfer encoding is not one this build decodes, so the content
    /// came out as written.
    pub unknown_encoding: bool,
    /// The charset is not one this build knows, so the text came out as
    /// lossy UTF-8.
    pub unknown_charset: bool,
    /// The bytes did not all fit the charset, some replaced by U+FFFD.
    pub malformed_charset: bool,
}

impl MimeDecodeProblems {
    /// Whether nothing went wrong.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
