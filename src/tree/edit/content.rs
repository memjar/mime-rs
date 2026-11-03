//! # Content edits
//!
//! Replacing a leaf's content, touching no other leaf and no field but those
//! stating how the content reads.
//!
//! The content is encoded under the transfer encoding the caller names when it
//! can carry it, else by the building rules, with the line ending of the
//! entity's block; Content-Transfer-Encoding is rewritten or added.

use alloc::{borrow::Cow, string::String};

use crate::{
    build::{MimeBuildOptions, entity::encode_leaf},
    codec::charset,
    header::{MimeHeaderKind, MimeHeaderName},
    tree::{
        body::MimeBodyNode,
        edit::{MimeEditError, options},
        entity::MimeEntity,
    },
    value::{MimeHeaderValue, content_type::MimeParam, transfer_encoding::MimeTransferEncoding},
};

impl MimeEntity<'_> {
    /// Replaces a leaf's content, stating its encoding in the header.
    ///
    /// `content` is in the leaf's own charset for a text leaf, written under
    /// `encoding` when it can carry it, else the one the building rules choose,
    /// which the Content-Transfer-Encoding field then states.
    pub(crate) fn replace_content(
        &mut self,
        content: &[u8],
        encoding: Option<&MimeTransferEncoding>,
        taken: &[String],
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        if !matches!(self.body, MimeBodyNode::Leaf(_)) {
            return Err(MimeEditError::NotALeaf);
        }

        // NOTE: a leaf under a multipart type is a multipart holding no
        // delimiter, which new content could make one.
        if self.content_type().is_top("multipart") {
            return Err(MimeEditError::Damaged);
        }

        let opts = options(opts, self.header.line_ending());
        let text = self.content_type().is_top("text");
        let (bytes, chosen) = encode_leaf(content, text, encoding, &opts, taken);
        let name = MimeHeaderName::Known(MimeHeaderKind::ContentTransferEncoding);

        let stated = match chosen {
            Some(encoding) => Some(encoding),
            None if self.header.get("content-transfer-encoding").is_some() => {
                Some(MimeTransferEncoding::SevenBit)
            }
            None => None,
        };

        if let Some(encoding) = stated {
            let value = MimeHeaderValue::TransferEncoding(encoding);
            self.header.set_field(&name, &value, &opts)?;
        }

        if !bytes.is_empty() {
            self.header.terminate();
        }

        self.body = MimeBodyNode::Leaf(Cow::Owned(bytes));
        Ok(())
    }

    /// Replaces a text leaf's content with `text`.
    ///
    /// Written in the leaf's declared charset when it can hold the text, else
    /// in UTF-8 with the Content-Type field rewritten to `charset=utf-8`.
    pub(crate) fn replace_text(
        &mut self,
        text: &str,
        taken: &[String],
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let label = self.content_type().param("charset");
        let encoded = label
            .as_deref()
            .and_then(|label| charset::encode(label, text))
            .map(Cow::into_owned);

        if encoded.is_none() && (label.is_some() || !text.is_ascii()) {
            let mut content_type = self.content_type().to_content_type();
            content_type
                .params
                .retain(|param| !param.name.eq_ignore_ascii_case("charset"));
            content_type.params.push(MimeParam {
                name: String::from("charset"),
                value: String::from("utf-8"),
            });

            let opts = options(opts, self.header.line_ending());
            let name = MimeHeaderName::Known(MimeHeaderKind::ContentType);
            let value = MimeHeaderValue::ContentType(content_type);
            self.header.set_field(&name, &value, &opts)?;
        }

        match encoded {
            Some(bytes) => self.replace_content(&bytes, None, taken, opts),
            None => self.replace_content(text.as_bytes(), None, taken, opts),
        }
    }
}
