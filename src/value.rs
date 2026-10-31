//! # Header values
//!
//! The decoded values header fields carry: one [`MimeHeaderValue`] arm per
//! RFC 8621 4.1.2 parsed form plus the MIME forms, each over the bespoke
//! types of the submodules.

pub mod address;
pub mod content_type;
pub mod datetime;
pub mod message_id;
pub mod subject;
pub mod transfer_encoding;

use alloc::{string::String, vec::Vec};

use crate::value::{
    address::MimeAddressList,
    content_type::{MimeContentType, MimeDisposition},
    datetime::MimeDateTime,
    transfer_encoding::MimeTransferEncoding,
};

/// A decoded header field value.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc8621#section-4.1.2>
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeHeaderValue {
    /// The value as written, folds included: the raw form, and what a value
    /// that fits no other form reads as.
    Raw(Vec<u8>),
    /// Unstructured text, encoded words decoded.
    Text(String),
    /// Mailboxes and groups.
    Addresses(MimeAddressList),
    /// Message identifiers, without angle brackets.
    MessageIds(Vec<String>),
    /// A date-time.
    Date(MimeDateTime),
    /// URLs, without angle brackets.
    Urls(Vec<String>),
    /// A comma-separated list of phrases.
    CommaList(Vec<String>),
    /// A media type and its parameters.
    ContentType(MimeContentType),
    /// A disposition type and its parameters.
    Disposition(MimeDisposition),
    /// A transfer encoding mechanism.
    TransferEncoding(MimeTransferEncoding),
}

impl MimeHeaderValue {
    /// The text, for a [`Text`](Self::Text) value.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The address list, for an [`Addresses`](Self::Addresses) value.
    pub fn as_addresses(&self) -> Option<&MimeAddressList> {
        match self {
            Self::Addresses(list) => Some(list),
            _ => None,
        }
    }

    /// The date, for a [`Date`](Self::Date) value.
    pub fn as_date(&self) -> Option<&MimeDateTime> {
        match self {
            Self::Date(date) => Some(date),
            _ => None,
        }
    }

    /// The identifiers, for a [`MessageIds`](Self::MessageIds) value.
    pub fn as_message_ids(&self) -> Option<&[String]> {
        match self {
            Self::MessageIds(ids) => Some(ids),
            _ => None,
        }
    }

    /// The URLs, for a [`Urls`](Self::Urls) value.
    pub fn as_urls(&self) -> Option<&[String]> {
        match self {
            Self::Urls(urls) => Some(urls),
            _ => None,
        }
    }

    /// The items, for a [`CommaList`](Self::CommaList) value.
    pub fn as_comma_list(&self) -> Option<&[String]> {
        match self {
            Self::CommaList(items) => Some(items),
            _ => None,
        }
    }

    /// The content type, for a [`ContentType`](Self::ContentType) value.
    pub fn as_content_type(&self) -> Option<&MimeContentType> {
        match self {
            Self::ContentType(content_type) => Some(content_type),
            _ => None,
        }
    }

    /// The disposition, for a [`Disposition`](Self::Disposition) value.
    pub fn as_disposition(&self) -> Option<&MimeDisposition> {
        match self {
            Self::Disposition(disposition) => Some(disposition),
            _ => None,
        }
    }

    /// The mechanism, for a [`TransferEncoding`](Self::TransferEncoding)
    /// value.
    pub fn as_transfer_encoding(&self) -> Option<&MimeTransferEncoding> {
        match self {
            Self::TransferEncoding(encoding) => Some(encoding),
            _ => None,
        }
    }
}
