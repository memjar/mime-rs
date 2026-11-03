//! # Decode problems
//!
//! What a checked decoding met on its way. Decoding never fails: it always
//! returns content, and these say how far that content can be trusted.

/// The problems one decoding met.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeDecodeProblems {
    /// The transfer encoding held bytes it forbids, skipped or kept literal.
    pub malformed_encoding: bool,
    /// A transfer encoding this build does not decode, content left as written.
    pub unknown_encoding: bool,
    /// A charset this build does not know, the text read as lossy UTF-8.
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
