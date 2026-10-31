//! # Field specs
//!
//! What the RFCs allow of each known field: the form it is read in, how
//! often it may appear at the top of a message, whether a MIME part may
//! carry it, and the section defining it. The decoder, the validator and
//! the builder all read this one table.

use crate::header::MimeHeaderKind;

/// The forms a field value can be read in: the RFC 8621 4.1.2 forms plus
/// the MIME ones.
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
