//! # Message builder
//!
//! [`MimeBuilder`], the strict way to compose a message: fields checked
//! against their specs as set, text and HTML bodies, inline resources and
//! attachments laid out as mail-builder does, raw fields and parts as escape
//! hatch. It yields a decoded [`Mime`], which [`Mime::to_bytes`] writes.
//!
//! The layout: one part alone; text and HTML in `multipart/alternative`;
//! inline resources with the HTML in `multipart/related` (RFC 2387);
//! attachments, and inline resources with no HTML to show them, around
//! everything in `multipart/mixed`.

use alloc::{string::String, vec, vec::Vec};

use crate::{
    build::{MimeBuildError, MimeBuildOptions, entity::is_field_name},
    header::{
        MimeHeader, MimeHeaderKind, MimeHeaderName, marker::MimeHeaderMarker, spec::MimeHeaderForm,
    },
    mime::{Mime, MimeBody},
    value::{
        MimeHeaderValue,
        content_type::{MimeContentType, MimeDisposition, MimeParam},
        message_id::MimeMessageId,
    },
};

/// A file to attach, or a resource for the HTML body to reference.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeAttachment {
    /// The media type.
    pub content_type: MimeContentType,
    /// The name it is offered to be saved under.
    pub filename: Option<String>,
    /// The content identifier the HTML references (`cid:`), without brackets.
    ///
    /// Required for an inline resource.
    pub content_id: Option<String>,
    /// The content, written unchanged.
    pub content: Vec<u8>,
}

/// A strict message builder.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeBuilder {
    headers: Vec<MimeHeader>,
    text: Option<String>,
    html: Option<String>,
    inline: Vec<Mime>,
    attachments: Vec<Mime>,
}

impl MimeBuilder {
    /// Sets a known field from its typed value via [`Self::header_value`].
    pub fn header<M: MimeHeaderMarker>(
        &mut self,
        value: M::Value,
    ) -> Result<&mut Self, MimeBuildError> {
        self.header_value(M::KIND, M::into_value(value))
    }

    /// Sets a known field from a decoded value, checked against its spec.
    ///
    /// Refuses a value of another form than the field's, a second instance of
    /// a field RFC 5322 3.6 allows once, a field the builder writes itself, and
    /// a message identifier that is not one.
    pub fn header_value(
        &mut self,
        kind: MimeHeaderKind,
        value: MimeHeaderValue,
    ) -> Result<&mut Self, MimeBuildError> {
        let spec = kind.spec();

        if matches!(
            kind,
            MimeHeaderKind::ContentType
                | MimeHeaderKind::ContentTransferEncoding
                | MimeHeaderKind::MimeVersion
        ) {
            return Err(MimeBuildError::Reserved(kind));
        }

        let form = form_of(&value);
        let fits = form == spec.form
            || (form == MimeHeaderForm::Addresses && spec.form == MimeHeaderForm::GroupedAddresses);

        if !fits {
            return Err(MimeBuildError::Form { kind, form });
        }

        let present = self
            .headers
            .iter()
            .any(|header| header.name.kind() == Some(kind));

        if present && spec.max == Some(1) {
            return Err(MimeBuildError::Repeated(kind));
        }

        if let MimeHeaderValue::MessageIds(ids) = &value {
            for id in ids {
                MimeMessageId::parse(id)?;
            }
        }

        self.headers.push(MimeHeader {
            name: MimeHeaderName::Known(kind),
            value,
        });

        Ok(self)
    }

    /// Adds any field as written: `value` is everything after the colon.
    ///
    /// The value includes its leading blank. Refuses a name holding a byte RFC
    /// 5322 2.2 does not allow, a field the builder writes itself, and a value
    /// with a line break no blank follows.
    pub fn raw_header(&mut self, name: &str, value: &[u8]) -> Result<&mut Self, MimeBuildError> {
        if !is_field_name(name) {
            return Err(MimeBuildError::FieldName(String::from(name)));
        }

        if let Some(
            kind @ (MimeHeaderKind::ContentType
            | MimeHeaderKind::ContentTransferEncoding
            | MimeHeaderKind::MimeVersion),
        ) = MimeHeaderName::new(name).kind()
        {
            return Err(MimeBuildError::Reserved(kind));
        }

        let breaks = value.iter().enumerate().any(|(at, byte)| {
            let line_break =
                *byte == b'\n' || (*byte == b'\r' && value.get(at + 1) != Some(&b'\n'));
            line_break && !matches!(value.get(at + 1), Some(b' ' | b'\t'))
        });

        if breaks {
            return Err(MimeBuildError::RawValue(String::from(name)));
        }

        self.headers.push(MimeHeader {
            name: MimeHeaderName::new(name),
            value: MimeHeaderValue::Raw(value.to_vec()),
        });

        Ok(self)
    }

    /// Sets the plain text body.
    pub fn text(&mut self, text: impl Into<String>) -> &mut Self {
        self.text = Some(text.into());
        self
    }

    /// Sets the HTML body.
    pub fn html(&mut self, html: impl Into<String>) -> &mut Self {
        self.html = Some(html.into());
        self
    }

    /// Adds an attachment.
    pub fn attachment(&mut self, attachment: MimeAttachment) -> Result<&mut Self, MimeBuildError> {
        let part = part(attachment, "attachment")?;
        self.attachments.push(part);
        Ok(self)
    }

    /// Adds an inline resource the HTML body references by its `content_id`.
    ///
    /// Refuses one without a `content_id`.
    pub fn inline(&mut self, resource: MimeAttachment) -> Result<&mut Self, MimeBuildError> {
        if resource.content_id.is_none() {
            return Err(MimeBuildError::InlineWithoutId);
        }

        let part = part(resource, "inline")?;
        self.inline.push(part);
        Ok(self)
    }

    /// Adds a part as given, unchecked, beside the attachments.
    pub fn part(&mut self, part: Mime) -> &mut Self {
        self.attachments.push(part);
        self
    }

    /// Lays the bodies and attachments out under the message fields.
    pub fn build(self) -> Mime {
        let text = self.text.map(|text| leaf("plain", text));
        let mut html = self.html.map(|html| leaf("html", html));
        let mut attachments = self.attachments;

        if !self.inline.is_empty() {
            match html.take() {
                Some(page) => {
                    let mut parts = vec![page];
                    parts.extend(self.inline);

                    let mut related = multipart("related", parts);
                    if let MimeHeaderValue::ContentType(content_type) =
                        &mut related.headers[0].value
                    {
                        content_type.params.push(MimeParam {
                            name: String::from("type"),
                            value: String::from("text/html"),
                        });
                    }

                    html = Some(related);
                }
                None => {
                    let mut parts = self.inline;
                    parts.append(&mut attachments);
                    attachments = parts;
                }
            }
        }

        let content = match (text, html) {
            (Some(text), Some(html)) => Some(multipart("alternative", vec![text, html])),
            (Some(text), None) => Some(text),
            (None, Some(html)) => Some(html),
            (None, None) => None,
        };

        let content = match (content, attachments.is_empty()) {
            (Some(content), true) => content,
            (Some(content), false) => {
                attachments.insert(0, content);
                multipart("mixed", attachments)
            }
            (None, false) => multipart("mixed", attachments),
            (None, true) => leaf("plain", String::new()),
        };

        let mut headers = self.headers;
        headers.extend(content.headers);

        Mime {
            headers,
            body: content.body,
        }
    }

    /// Lays the message out and writes it as bytes.
    pub fn write(self, opts: &MimeBuildOptions) -> Result<Vec<u8>, MimeBuildError> {
        self.build().to_bytes(opts)
    }
}

/// The form a decoded value is of.
fn form_of(value: &MimeHeaderValue) -> MimeHeaderForm {
    match value {
        MimeHeaderValue::Raw(_) => MimeHeaderForm::Raw,
        MimeHeaderValue::Text(_) => MimeHeaderForm::Text,
        MimeHeaderValue::Addresses(_) => MimeHeaderForm::Addresses,
        MimeHeaderValue::MessageIds(_) => MimeHeaderForm::MessageIds,
        MimeHeaderValue::Date(_) => MimeHeaderForm::Date,
        MimeHeaderValue::Urls(_) => MimeHeaderForm::Urls,
        MimeHeaderValue::CommaList(_) => MimeHeaderForm::CommaList,
        MimeHeaderValue::ContentType(_) => MimeHeaderForm::ContentType,
        MimeHeaderValue::Disposition(_) => MimeHeaderForm::Disposition,
        MimeHeaderValue::TransferEncoding(_) => MimeHeaderForm::TransferEncoding,
    }
}

fn content_type_header(top: &str, sub: &str, params: Vec<MimeParam>) -> MimeHeader {
    MimeHeader {
        name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
        value: MimeHeaderValue::ContentType(MimeContentType {
            top: String::from(top),
            sub: String::from(sub),
            params,
        }),
    }
}

fn leaf(sub: &str, text: String) -> Mime {
    let charset = MimeParam {
        name: String::from("charset"),
        value: String::from("utf-8"),
    };

    Mime {
        headers: vec![content_type_header("text", sub, vec![charset])],
        body: MimeBody::Text(text),
    }
}

fn multipart(sub: &str, parts: Vec<Mime>) -> Mime {
    Mime {
        headers: vec![content_type_header("multipart", sub, Vec::new())],
        body: MimeBody::Multipart(parts),
    }
}

fn part(attachment: MimeAttachment, disposition: &str) -> Result<Mime, MimeBuildError> {
    let mut headers = vec![MimeHeader {
        name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
        value: MimeHeaderValue::ContentType(attachment.content_type),
    }];

    let params = attachment
        .filename
        .map(|filename| {
            vec![MimeParam {
                name: String::from("filename"),
                value: filename,
            }]
        })
        .unwrap_or_default();

    headers.push(MimeHeader {
        name: MimeHeaderName::Known(MimeHeaderKind::ContentDisposition),
        value: MimeHeaderValue::Disposition(MimeDisposition {
            kind: String::from(disposition),
            params,
        }),
    });

    if let Some(id) = attachment.content_id {
        let id = MimeMessageId::parse(&id)?;

        headers.push(MimeHeader {
            name: MimeHeaderName::Known(MimeHeaderKind::ContentId),
            value: MimeHeaderValue::MessageIds(vec![String::from(id)]),
        });
    }

    Ok(Mime {
        headers,
        body: MimeBody::Binary(attachment.content),
    })
}
