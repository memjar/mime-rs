//! # Field specs
//!
//! What the RFCs allow of each known field: the form it is read in, how
//! often it may appear at the top of a message, whether a MIME part may
//! carry it, and the section defining it. The decoder, the validator and
//! the builder all read this one table.

use core::{fmt, ops::Deref, str::FromStr};

use crate::header::MimeHeaderKind;
#[cfg(feature = "parser")]
use crate::value::MimeHeaderValue;

/// The forms a field value is read in: RFC 8621 4.1.2's and the MIME ones.
///
/// Spelled as RFC 8621 names them (`asText`, `asAddresses`), the MIME ones in
/// the same style.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc8621#section-4.1.2>
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MimeHeaderForm {
    /// The value as written.
    Raw,
    /// Unstructured text, encoded words decoded.
    Text,
    /// Every mailbox, group members included, groups flattened.
    Addresses,
    /// Mailboxes and groups, as written.
    GroupedAddresses,
    /// Message identifiers.
    MessageIds,
    /// A date-time.
    Date,
    /// URLs in angle brackets.
    Urls,
    /// A comma-separated list of phrases.
    CommaList,
    /// A media type and its parameters.
    ContentType,
    /// A disposition type and its parameters.
    Disposition,
    /// A transfer encoding mechanism.
    TransferEncoding,
}

impl MimeHeaderForm {
    /// Every form.
    pub const ALL: &[Self] = &[
        Self::Raw,
        Self::Text,
        Self::Addresses,
        Self::GroupedAddresses,
        Self::MessageIds,
        Self::Date,
        Self::Urls,
        Self::CommaList,
        Self::ContentType,
        Self::Disposition,
        Self::TransferEncoding,
    ];

    /// Reads a value standing alone, outside any message, in this form.
    ///
    /// The bytes after a colon, up to the end of the field (a line break not
    /// followed by a blank), anything after it ignored.
    #[cfg(feature = "parser")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parser")))]
    pub fn parse(self, bytes: &[u8]) -> MimeHeaderValue {
        crate::tree::field::value::parse(self, bytes)
    }

    fn spelling(&self) -> &'static str {
        match self {
            Self::Raw => "asRaw",
            Self::Text => "asText",
            Self::Addresses => "asAddresses",
            Self::GroupedAddresses => "asGroupedAddresses",
            Self::MessageIds => "asMessageIds",
            Self::Date => "asDate",
            Self::Urls => "asURLs",
            Self::CommaList => "asCommaList",
            Self::ContentType => "asContentType",
            Self::Disposition => "asDisposition",
            Self::TransferEncoding => "asTransferEncoding",
        }
    }
}

impl Deref for MimeHeaderForm {
    type Target = str;

    fn deref(&self) -> &str {
        self.spelling()
    }
}

impl FromStr for MimeHeaderForm {
    type Err = ();

    /// Reads a form name ASCII case-insensitively.
    fn from_str(name: &str) -> Result<Self, ()> {
        Self::ALL
            .iter()
            .find(|form| form.spelling().eq_ignore_ascii_case(name))
            .copied()
            .ok_or(())
    }
}

impl fmt::Display for MimeHeaderForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

/// What the RFCs allow of one known field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MimeHeaderSpec {
    /// The field this spec describes.
    pub kind: MimeHeaderKind,
    /// The form its value is read in by default.
    pub form: MimeHeaderForm,
    /// How many times it must appear at the top of a message.
    pub min: u8,
    /// How many times it may appear at the top of a message, `None` for any.
    pub max: Option<u8>,
    /// Whether a MIME part may carry it (a Content- field).
    pub part: bool,
    /// The RFC section defining it.
    pub rfc: &'static str,
}
