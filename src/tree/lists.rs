//! # Body lists
//!
//! The RFC 8621 4.1.4 text body, HTML body and attachment lists of a
//! message, computed on the tree by the parseStructure algorithm, without
//! decoding any leaf.

use alloc::{string::String, vec::Vec};

use crate::tree::{body::MimeBodyNode, entity::MimeEntity};

/// The body lists of one message, each in document order.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc8621#section-4.1.4>
#[derive(Clone, Debug, Default)]
pub struct MimeBodyLists<'t, 'a> {
    /// The parts to show as plain text.
    pub text: Vec<&'t MimeEntity<'a>>,
    /// The parts to show as HTML.
    pub html: Vec<&'t MimeEntity<'a>>,
    /// The parts to offer as attachments.
    pub attachments: Vec<&'t MimeEntity<'a>>,
}

impl<'t, 'a> MimeBodyLists<'t, 'a> {
    /// Computes the lists of the message rooted at `root`.
    ///
    /// A nested message is one attachment; its own lists are its own.
    pub fn new(root: &'t MimeEntity<'a>) -> Self {
        let mut lists = Self::default();
        let mut text = Vec::new();
        let mut html = Vec::new();

        parse_structure(
            &[root],
            "mixed",
            false,
            Some(&mut html),
            Some(&mut text),
            &mut lists.attachments,
        );

        lists.text = text;
        lists.html = html;
        lists
    }

    /// Whether the message has an attachment a reader should offer.
    ///
    /// That is one not marked `inline` (RFC 8621 4.1.4).
    pub fn has_attachment(&self) -> bool {
        self.attachments.iter().any(|part| {
            !part
                .disposition()
                .is_some_and(|disposition| disposition.is("inline"))
        })
    }
}

/// The RFC 8621 4.1.4 parseStructure, transcribed.
fn parse_structure<'t, 'a>(
    parts: &[&'t MimeEntity<'a>],
    multipart_type: &str,
    in_alternative: bool,
    html: Option<&mut Vec<&'t MimeEntity<'a>>>,
    text: Option<&mut Vec<&'t MimeEntity<'a>>>,
    attachments: &mut Vec<&'t MimeEntity<'a>>,
) {
    let mut html = html;
    let mut text = text;
    let text_len = text.as_ref().map(|text| text.len());
    let html_len = html.as_ref().map(|html| html.len());

    for (index, part) in parts.iter().enumerate() {
        let content_type = part.content_type();

        // NOTE: a part of a multipart/digest with no Content-Type is a
        // message/rfc822 (RFC 2046 5.1.5).
        let kind = if multipart_type == "digest" && part.header.get("content-type").is_none() {
            String::from("message/rfc822")
        } else {
            lower(content_type.top) + "/" + &lower(content_type.sub)
        };
        let is_inline_media = is_inline_media(&kind);
        let is_attachment = part
            .disposition()
            .is_some_and(|disposition| disposition.is("attachment"));

        let is_inline = !is_attachment
            && (kind == "text/plain" || kind == "text/html" || is_inline_media)
            && (index == 0
                || (multipart_type != "related"
                    && (is_inline_media || part.attachment_name().is_none())));

        if let MimeBodyNode::Multipart(multipart) = &part.body {
            let sub = lower(content_type.sub);
            let children: Vec<&MimeEntity<'a>> =
                multipart.parts.iter().map(|part| &part.entity).collect();

            parse_structure(
                &children,
                &sub,
                in_alternative || sub == "alternative",
                html.as_deref_mut(),
                text.as_deref_mut(),
                attachments,
            );
        } else if is_inline {
            if multipart_type == "alternative" {
                match kind.as_str() {
                    "text/plain" => {
                        if let Some(text) = text.as_deref_mut() {
                            text.push(part);
                        }
                    }
                    "text/html" => {
                        if let Some(html) = html.as_deref_mut() {
                            html.push(part);
                        }
                    }
                    _ => attachments.push(part),
                }

                continue;
            } else if in_alternative {
                if kind == "text/plain" {
                    html = None;
                }

                if kind == "text/html" {
                    text = None;
                }
            }

            if let Some(text) = text.as_deref_mut() {
                text.push(part);
            }

            if let Some(html) = html.as_deref_mut() {
                html.push(part);
            }

            if (text.is_none() || html.is_none()) && is_inline_media {
                attachments.push(part);
            }
        } else {
            attachments.push(part);
        }
    }

    if multipart_type == "alternative"
        && let (Some(text), Some(html)) = (text, html)
    {
        // NOTE: an alternative holding only HTML shows it as the text
        // body too, and one holding only text as the HTML body.
        if text_len == Some(text.len()) && html_len != Some(html.len()) {
            let found: Vec<_> = html[html_len.unwrap_or(0)..].to_vec();
            text.extend(found);
        }

        if html_len == Some(html.len()) && text_len != Some(text.len()) {
            let found: Vec<_> = text[text_len.unwrap_or(0)..].to_vec();
            html.extend(found);
        }
    }
}

fn is_inline_media(kind: &str) -> bool {
    kind.starts_with("image/") || kind.starts_with("audio/") || kind.starts_with("video/")
}

fn lower(text: &[u8]) -> String {
    String::from_utf8_lossy(text).to_ascii_lowercase()
}
