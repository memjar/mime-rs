//! # Body
//!
//! The byte-faithful body of an entity: raw leaf bytes, a multipart split
//! at its delimiter lines, or a nested message.
//!
//! A delimiter line is kept whole, the line ending before it included (RFC
//! 2046 5.1.1 attaches that line ending to the delimiter, not to the part it
//! ends), so the parts hold exactly their content.

use alloc::{borrow::Cow, boxed::Box, vec::Vec};

use crate::tree::entity::MimeEntity;

/// The body of an entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeBodyNode<'a> {
    /// Raw content, still transfer-encoded.
    Leaf(Cow<'a, [u8]>),
    /// A multipart split at its delimiter lines.
    Multipart(MimeMultipart<'a>),
    /// A nested `message/rfc822` or `message/global` entity.
    Message(Box<MimeEntity<'a>>),
}

/// A multipart body.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2046#section-5.1.1>
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeMultipart<'a> {
    /// What precedes the first delimiter line.
    pub preamble: Cow<'a, [u8]>,
    /// The parts, in source order.
    pub parts: Vec<MimePart<'a>>,
    /// The close delimiter line with the line ending before and after it,
    /// empty when the multipart was never closed.
    pub close: Cow<'a, [u8]>,
    /// What follows the close delimiter line.
    pub epilogue: Cow<'a, [u8]>,
}

/// One part of a multipart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimePart<'a> {
    /// The delimiter line opening the part, with the line ending before and
    /// after it, its transport padding kept.
    pub delimiter: Cow<'a, [u8]>,
    /// The part itself.
    pub entity: MimeEntity<'a>,
}

impl Default for MimeBodyNode<'_> {
    fn default() -> Self {
        Self::Leaf(Cow::Borrowed(&[]))
    }
}

impl<'a> MimeBodyNode<'a> {
    /// The raw bytes of a leaf, `None` for a multipart or a message.
    pub fn leaf(&self) -> Option<&[u8]> {
        match self {
            Self::Leaf(bytes) => Some(bytes),
            Self::Multipart(_) | Self::Message(_) => None,
        }
    }

    /// The multipart, `None` for a leaf or a message.
    pub fn multipart(&self) -> Option<&MimeMultipart<'a>> {
        match self {
            Self::Multipart(multipart) => Some(multipart),
            Self::Leaf(_) | Self::Message(_) => None,
        }
    }

    /// The nested message, `None` for a leaf or a multipart.
    pub fn message(&self) -> Option<&MimeEntity<'a>> {
        match self {
            Self::Message(entity) => Some(entity),
            Self::Leaf(_) | Self::Multipart(_) => None,
        }
    }

    pub(crate) fn push_chunks<'s>(&'s self, out: &mut Vec<&'s [u8]>) {
        match self {
            Self::Leaf(bytes) => out.push(bytes),
            Self::Multipart(multipart) => {
                out.push(&multipart.preamble);

                for part in &multipart.parts {
                    out.push(&part.delimiter);
                    part.entity.push_chunks(out);
                }

                out.push(&multipart.close);
                out.push(&multipart.epilogue);
            }
            Self::Message(entity) => entity.push_chunks(out),
        }
    }
}

impl MimeBodyNode<'_> {
    /// The body with every leaf owned, free of the input's lifetime.
    pub fn into_owned(self) -> MimeBodyNode<'static> {
        match self {
            Self::Leaf(bytes) => MimeBodyNode::Leaf(Cow::Owned(bytes.into_owned())),
            Self::Multipart(multipart) => MimeBodyNode::Multipart(MimeMultipart {
                preamble: Cow::Owned(multipart.preamble.into_owned()),
                parts: multipart
                    .parts
                    .into_iter()
                    .map(|part| MimePart {
                        delimiter: Cow::Owned(part.delimiter.into_owned()),
                        entity: part.entity.into_owned(),
                    })
                    .collect(),
                close: Cow::Owned(multipart.close.into_owned()),
                epilogue: Cow::Owned(multipart.epilogue.into_owned()),
            }),
            Self::Message(entity) => MimeBodyNode::Message(Box::new(entity.into_owned())),
        }
    }
}
