//! # Content transfer encoding
//!
//! The decoded value of a Content-Transfer-Encoding field (RFC 2045 6.1).

use alloc::string::String;
use core::fmt;

/// A content transfer encoding mechanism.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-6.1>
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub enum MimeTransferEncoding {
    /// `7bit`, the default.
    #[default]
    SevenBit,
    /// `8bit`.
    EightBit,
    /// `binary`.
    Binary,
    /// `quoted-printable`.
    QuotedPrintable,
    /// `base64`.
    Base64,
    /// A mechanism no RFC this crate covers defines, as written, decoded as
    /// identity.
    Unknown(String),
}

impl MimeTransferEncoding {
    /// Reads a mechanism token, ASCII case-insensitively, blanks trimmed.
    pub fn parse(token: &str) -> Self {
        let token = token.trim();

        [
            ("7bit", Self::SevenBit),
            ("8bit", Self::EightBit),
            ("binary", Self::Binary),
            ("quoted-printable", Self::QuotedPrintable),
            ("base64", Self::Base64),
        ]
        .into_iter()
        .find(|(name, _)| token.eq_ignore_ascii_case(name))
        .map_or_else(
            || Self::Unknown(String::from(token)),
            |(_, encoding)| encoding,
        )
    }

    /// The mechanism's canonical spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::SevenBit => "7bit",
            Self::EightBit => "8bit",
            Self::Binary => "binary",
            Self::QuotedPrintable => "quoted-printable",
            Self::Base64 => "base64",
            Self::Unknown(token) => token,
        }
    }

    /// Whether the body is written as is: `7bit`, `8bit` or `binary` (RFC
    /// 2045 6.2), the only encodings a composite type may take.
    pub fn is_identity(&self) -> bool {
        matches!(self, Self::SevenBit | Self::EightBit | Self::Binary)
    }
}

impl fmt::Display for MimeTransferEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
