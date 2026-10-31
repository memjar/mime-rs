//! # Entity
//!
//! A MIME entity: a header block and a body, the unit every message, part
//! and nested message is made of, with the readers a caller needs from one:
//! typed header access through the field markers, the structural fields,
//! and the leaf accessors decoding content only when asked.

use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};

use crate::{
    codec::{charset, flowed, problems::MimeDecodeProblems, quoted_printable},
    header::{MimeHeader, MimeHeaderName, marker::MimeHeaderMarker},
    mime::{Mime, MimeBody},
    tree::{
        anomaly::MimeAnomaly,
        body::MimeBodyNode,
        cst::MimeCst,
        field::{content_type, value},
        header::MimeHeaderBlock,
        lists::MimeBodyLists,
    },
    value::{
        content_type::{MimeContentType, MimeDisposition},
        transfer_encoding::MimeTransferEncoding,
    },
};

/// A header block and a body.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-2.4>
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeEntity<'a> {
    /// The header block.
    pub header: MimeHeaderBlock<'a>,
    /// The body.
    pub body: MimeBodyNode<'a>,
    /// What the parser worked around in this entity, its descendants not
    /// included.
    pub anomalies: Vec<MimeAnomaly>,
}

impl<'a> MimeEntity<'a> {
    /// Every byte of the entity, in order, as borrowed slices.
    pub fn chunks(&self) -> Vec<&[u8]> {
        let mut out = Vec::new();
        self.push_chunks(&mut out);
        out
    }

    /// The entity serialized back to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let chunks = self.chunks();
        let mut out = Vec::with_capacity(chunks.iter().map(|chunk| chunk.len()).sum());

        for chunk in chunks {
            out.extend_from_slice(chunk);
        }

        out
    }

    /// The first field the marker names, read in its form.
    pub fn header<M: MimeHeaderMarker>(&self) -> Option<M::Value> {
        let field = self.header.get(&M::KIND)?;
        M::from_value(field.read())
    }

    /// The content type: the last Content-Type field, or `text/plain` with
    /// none (RFC 2045 5.2).
    pub fn content_type(&self) -> MimeContentType {
        self.header
            .get_all("content-type")
            .last()
            .map(|field| content_type::parse_content_type(&field.value))
            .filter(|content_type| !content_type.top.is_empty())
            .unwrap_or_else(|| MimeContentType {
                top: "text".into(),
                sub: "plain".into(),
                params: Vec::new(),
            })
    }

    /// The transfer encoding: the last Content-Transfer-Encoding field, or
    /// `7bit` with none (RFC 2045 6.1).
    pub fn transfer_encoding(&self) -> MimeTransferEncoding {
        self.header
            .get_all("content-transfer-encoding")
            .last()
            .map(|field| value::transfer_encoding(&field.value))
            .unwrap_or_default()
    }

    /// The disposition, from the last Content-Disposition field.
    pub fn disposition(&self) -> Option<MimeDisposition> {
        self.header
            .get_all("content-disposition")
            .last()
            .map(|field| content_type::parse_disposition(&field.value))
            .filter(|disposition| !disposition.kind.is_empty())
    }

    /// The name a part offers to be saved under: the disposition's
    /// `filename`, else the content type's `name` (RFC 2183 2.3).
    pub fn attachment_name(&self) -> Option<String> {
        let from_disposition = self
            .disposition()
            .and_then(|disposition| disposition.param("filename").map(String::from));

        from_disposition.or_else(|| self.content_type().param("name").map(String::from))
    }

    /// Projects the entity onto the decoded model: every field read in its
    /// default form, every leaf transfer-decoded where the build can.
    pub fn decode(&self) -> Mime {
        let headers = self
            .header
            .fields()
            .map(|field| MimeHeader {
                name: MimeHeaderName::new(&field.name_str()),
                value: field.read(),
            })
            .collect();

        let body = match &self.body {
            MimeBodyNode::Leaf(_) => {
                MimeBody::Leaf(self.decoded().unwrap_or_default().into_owned())
            }
            MimeBodyNode::Multipart(multipart) => MimeBody::Multipart(
                multipart
                    .parts
                    .iter()
                    .map(|part| part.entity.decode())
                    .collect(),
            ),
            MimeBodyNode::Message(nested) => MimeBody::Message(Box::new(nested.decode())),
        };

        Mime { headers, body }
    }

    /// The raw bytes of a leaf, still transfer-encoded; `None` for a
    /// multipart or a message.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.body.leaf()
    }

    /// A leaf's quoted-printable decoded, or its raw bytes when it is not
    /// quoted-printable. Still in its own charset.
    pub fn quoted_printable(&self) -> Option<Vec<u8>> {
        let raw = self.bytes()?;

        if self.transfer_encoding() != MimeTransferEncoding::QuotedPrintable {
            return Some(raw.to_vec());
        }

        let mut out = Vec::new();
        quoted_printable::decode(raw, &mut out);
        Some(out)
    }

    /// A leaf's base64 decoded, or its raw bytes when it is not base64.
    #[cfg(feature = "base64")]
    #[cfg_attr(docsrs, doc(cfg(feature = "base64")))]
    pub fn base64(&self) -> Option<Vec<u8>> {
        let raw = self.bytes()?;

        if self.transfer_encoding() != MimeTransferEncoding::Base64 {
            return Some(raw.to_vec());
        }

        let mut out = Vec::new();
        crate::codec::base64::decode(raw, &mut out);
        Some(out)
    }

    /// A leaf transfer-decoded by whichever decoders the build carries,
    /// borrowed when its encoding is the identity.
    pub fn decoded(&self) -> Option<Cow<'_, [u8]>> {
        self.decoded_checked().map(|(bytes, _)| bytes)
    }

    /// [`decoded`](Self::decoded), with the problems met.
    pub fn decoded_checked(&self) -> Option<(Cow<'_, [u8]>, MimeDecodeProblems)> {
        let raw = self.bytes()?;
        let mut problems = MimeDecodeProblems::default();

        let bytes = match self.transfer_encoding() {
            MimeTransferEncoding::QuotedPrintable => {
                let mut out = Vec::with_capacity(raw.len());
                problems.malformed_encoding = !quoted_printable::decode(raw, &mut out);
                Cow::Owned(out)
            }
            #[cfg(feature = "base64")]
            MimeTransferEncoding::Base64 => {
                let mut out = Vec::with_capacity(raw.len() / 4 * 3);
                problems.malformed_encoding = !crate::codec::base64::decode(raw, &mut out);
                Cow::Owned(out)
            }
            MimeTransferEncoding::SevenBit
            | MimeTransferEncoding::EightBit
            | MimeTransferEncoding::Binary => Cow::Borrowed(raw),
            _ => {
                problems.unknown_encoding = true;
                Cow::Borrowed(raw)
            }
        };

        Some((bytes, problems))
    }

    /// The length of a leaf once transfer-decoded.
    pub fn decoded_len(&self) -> Option<usize> {
        self.decoded().map(|bytes| bytes.len())
    }

    /// A leaf's text: transfer-decoded, transcoded from its `charset` (a
    /// byte order mark taking precedence, UTF-8 without one), and unwrapped
    /// when it is `format=flowed` (RFC 3676). `None` for a multipart or a
    /// message, and for a charset this build cannot transcode.
    pub fn text(&self) -> Option<Cow<'_, str>> {
        self.text_checked().map(|(text, _)| text)
    }

    /// [`text`](Self::text), with the problems met.
    pub fn text_checked(&self) -> Option<(Cow<'_, str>, MimeDecodeProblems)> {
        let (bytes, mut problems) = self.decoded_checked()?;
        let content_type = self.content_type();
        let label = content_type.param("charset");

        let text: Cow<'_, str> = match bytes {
            Cow::Borrowed(bytes) => {
                let decoded = charset::transcode_body(label, bytes)?;
                problems.unknown_charset = !decoded.known;
                problems.malformed_charset = decoded.malformed;
                decoded.text
            }
            Cow::Owned(bytes) => {
                let decoded = charset::transcode_body(label, &bytes)?;
                problems.unknown_charset = !decoded.known;
                problems.malformed_charset = decoded.malformed;
                Cow::Owned(decoded.text.into_owned())
            }
        };

        let flowed = content_type.is("text", "plain")
            && content_type
                .param("format")
                .is_some_and(|format| format.eq_ignore_ascii_case("flowed"));

        if !flowed {
            return Some((text, problems));
        }

        let delsp = content_type
            .param("delsp")
            .is_some_and(|delsp| delsp.eq_ignore_ascii_case("yes"));

        Some((Cow::Owned(flowed::decode(&text, delsp)), problems))
    }

    /// At most `chars` characters of a leaf's text, decoding only as much of
    /// the leaf as they need.
    pub fn text_prefix(&self, chars: usize) -> Option<String> {
        let raw = self.bytes()?;
        let mut take = (chars * 16).max(64);

        loop {
            let whole = take >= raw.len();
            let cut = if whole {
                raw.len()
            } else {
                // NOTE: cut after a line break so no encoded quantum or
                // escape is split.
                raw[..take]
                    .iter()
                    .rposition(|byte| *byte == b'\n')
                    .map_or(take, |at| at + 1)
            };

            let prefix = MimeEntity {
                header: self.header.clone(),
                body: MimeBodyNode::Leaf(Cow::Borrowed(&raw[..cut])),
                anomalies: Vec::new(),
            };

            let text = prefix.text()?;

            if whole || text.chars().count() > chars {
                return Some(text.chars().take(chars).collect());
            }

            take *= 2;
        }
    }

    /// An encoded `message/rfc822` or `message/global` leaf, decoded and
    /// parsed as an owned tree; `None` for any other entity, and for one
    /// whose content opens with no header field.
    ///
    /// The structural parse keeps such a leaf as it is, since RFC 2046 5.2.1
    /// forbids encoding a message; it is parsed here, one level per call.
    pub fn nested(&self) -> Option<MimeCst<'static>> {
        let content_type = self.content_type();
        let message = content_type.is("message", "rfc822") || content_type.is("message", "global");

        if !message || self.transfer_encoding().is_identity() {
            return None;
        }

        let bytes = self.decoded()?.into_owned();
        let line = bytes
            .split(|byte| *byte == b'\n')
            .next()
            .unwrap_or_default();

        if !line.contains(&b':') || matches!(line.first(), Some(b' ' | b'\t')) {
            return None;
        }

        Some(MimeCst::parse(&bytes).into_owned())
    }

    /// The RFC 8621 4.1.4 text body, HTML body and attachment lists of the
    /// message rooted here.
    pub fn body_lists(&self) -> MimeBodyLists<'_, 'a> {
        MimeBodyLists::new(self)
    }

    /// The entity with every leaf owned, free of the input's lifetime.
    pub fn into_owned(self) -> MimeEntity<'static> {
        MimeEntity {
            header: self.header.into_owned(),
            body: self.body.into_owned(),
            anomalies: self.anomalies,
        }
    }

    pub(crate) fn push_chunks<'s>(&'s self, out: &mut Vec<&'s [u8]>) {
        self.header.push_chunks(out);
        self.body.push_chunks(out);
    }
}
