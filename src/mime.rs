//! # Decoded message
//!
//! [`Mime`], a message (or a MIME part) as pure data: its decoded header
//! fields in order and its body, transfer encodings resolved. It carries no
//! wire detail (folds, quoting, preambles), which the syntax tree keeps.

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::{
    header::{MimeHeader, MimeHeaderKind},
    value::{MimeHeaderValue, content_type::MimeContentType},
};

/// A decoded message or MIME part.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mime {
    /// The header fields, in order.
    pub headers: Vec<MimeHeader>,
    /// The body.
    pub body: MimeBody,
}

/// A decoded body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeBody {
    /// The content of a `text` leaf, as UTF-8.
    ///
    /// Transfer-decoded, transcoded to UTF-8 and flowed text unwrapped.
    Text(String),
    /// The content of any other leaf, transfer-decoded where the build can.
    ///
    /// Also a text leaf whose charset the build cannot transcode.
    Binary(Vec<u8>),
    /// The parts of a multipart, in order.
    Multipart(Vec<Mime>),
    /// A nested message.
    Message(Box<Mime>),
}

impl Default for MimeBody {
    fn default() -> Self {
        Self::Binary(Vec::new())
    }
}

impl Mime {
    /// The value of the first field of `kind`.
    pub fn header(&self, kind: MimeHeaderKind) -> Option<&MimeHeaderValue> {
        self.headers
            .iter()
            .find(|header| header.name.kind() == Some(kind))
            .map(|header| &header.value)
    }

    /// The values of every field named `name`, ASCII case-insensitively.
    pub fn headers_named<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s MimeHeaderValue> {
        self.headers
            .iter()
            .filter(move |header| header.name.is(name))
            .map(|header| &header.value)
    }

    /// The last Content-Type field, or `text/plain` with none (RFC 2045 5.2).
    pub fn content_type(&self) -> MimeContentType {
        self.headers
            .iter()
            .rev()
            .find(|header| header.name.kind() == Some(MimeHeaderKind::ContentType))
            .and_then(|header| header.value.as_content_type().cloned())
            .unwrap_or_else(|| MimeContentType {
                top: "text".into(),
                sub: "plain".into(),
                params: Vec::new(),
            })
    }
}
