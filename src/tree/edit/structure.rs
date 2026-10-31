//! # Structural edits
//!
//! Inserting, removing and moving the parts of a multipart, wrapping an
//! entity into a new multipart and unwrapping a multipart of one part. Each
//! changes the part list of the one multipart it names: an inserted part
//! gets a delimiter line in the style of its siblings, a removed one leaves
//! with its own, and the boundary is regenerated only when inserted content
//! holds it, rewriting that multipart's boundary parameter and delimiter
//! lines alone.

use alloc::{borrow::Cow, boxed::Box, format, string::String, vec, vec::Vec};
use core::mem;

use crate::{
    build::{
        MimeBuildError, MimeBuildOptions,
        entity::{self as writer, Boundaries, contains, field_line},
    },
    header::{MimeHeaderKind, MimeHeaderName},
    mime::Mime,
    tree::{
        body::{MimeBodyNode, MimeMultipart, MimePart},
        cst::MimeCst,
        edit::{MimeEditError, options},
        entity::MimeEntity,
        field::content_type::param_span,
        header::{MimeHeaderBlock, MimeHeaderField, MimeHeaderItem},
    },
    value::{
        MimeHeaderValue,
        content_type::{MimeContentType, MimeParam},
    },
};

impl<'a> MimeEntity<'a> {
    /// Inserts `part` at `position` (at the end past it) in a multipart,
    /// avoiding `taken`, the boundaries of the multiparts enclosing it.
    pub(crate) fn insert_part_within(
        &mut self,
        position: usize,
        part: &Mime,
        taken: &[String],
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let eol = self.header.line_ending();
        let opts = options(opts, eol);
        let boundary = self
            .content_type()
            .param("boundary")
            .ok_or(MimeEditError::NotAMultipart)?;

        let MimeBodyNode::Multipart(multipart) = &self.body else {
            return Err(MimeEditError::NotAMultipart);
        };

        // NOTE: the part avoids this multipart's boundary and the enclosing
        // ones.
        let mut around = taken.to_vec();
        around.push(boundary.clone());

        let digest = self.content_type().is("multipart", "digest");
        let mut bytes = Vec::new();
        writer::write(
            &writer::explicit(part, digest),
            &opts,
            false,
            &mut Boundaries::new(&around),
            &mut bytes,
        )?;

        let delimiter = sibling_delimiter(multipart, &boundary, eol);
        let entity = MimeCst::parse(&bytes).into_owned().root;

        if contains(&bytes, boundary.as_bytes()) {
            let parts: Vec<Vec<u8>> = multipart
                .parts
                .iter()
                .map(|part| part.entity.to_bytes())
                .collect();

            let mut contents: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
            contents.push(&bytes);
            let replacement = Boundaries::new(taken).fresh(&contents);

            self.rewrite_boundary(&boundary, &replacement, &opts)?;
        }

        let current = self.content_type().param("boundary").unwrap_or_default();
        let delimiter = replace_once(
            &delimiter,
            &format!("--{boundary}"),
            &format!("--{current}"),
        );

        let MimeBodyNode::Multipart(multipart) = &mut self.body else {
            return Err(MimeEditError::NotAMultipart);
        };

        let position = position.min(multipart.parts.len());
        multipart.parts.insert(
            position,
            MimePart {
                delimiter: Cow::Owned(delimiter),
                entity,
            },
        );

        separate(multipart, eol);
        Ok(())
    }

    /// Removes the part at `position` of a multipart, with its delimiter
    /// line, and returns it.
    pub(crate) fn remove_part(&mut self, position: usize) -> Result<MimeEntity<'a>, MimeEditError> {
        let eol = self.header.line_ending();
        let MimeBodyNode::Multipart(multipart) = &mut self.body else {
            return Err(MimeEditError::NotAMultipart);
        };

        if position >= multipart.parts.len() {
            return Err(MimeEditError::NoSuchItem(position));
        }

        if multipart.parts.len() == 1 {
            return Err(MimeEditError::LastPart);
        }

        let part = multipart.parts.remove(position);

        separate(multipart, eol);
        Ok(part.entity)
    }

    /// Moves the part at `from` of a multipart to `to`, with its delimiter
    /// line.
    pub(crate) fn move_part(&mut self, from: usize, to: usize) -> Result<(), MimeEditError> {
        let eol = self.header.line_ending();
        let MimeBodyNode::Multipart(multipart) = &mut self.body else {
            return Err(MimeEditError::NotAMultipart);
        };

        if from >= multipart.parts.len() || to >= multipart.parts.len() {
            return Err(MimeEditError::NoSuchItem(from.max(to)));
        }

        let part = multipart.parts.remove(from);
        multipart.parts.insert(to, part);
        separate(multipart, eol);
        Ok(())
    }

    /// Wraps the entity into a new `multipart/<sub>`: its Content- fields
    /// move, in order, to the new first part, whose body is the entity's
    /// body unchanged, and its other fields stay on the entity.
    pub(crate) fn wrap_within(
        &mut self,
        sub: &str,
        taken: &[String],
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let eol = self.header.line_ending();
        let opts = options(opts, eol);

        let mut kept = Vec::with_capacity(self.header.items.len());
        let mut moved = Vec::new();
        let mut first = None;

        for item in mem::take(&mut self.header.items) {
            match &item {
                MimeHeaderItem::Field(field) if is_content(field) => {
                    first.get_or_insert(kept.len());
                    moved.push(item);
                }
                _ => kept.push(item),
            }
        }

        let mut child = MimeEntity {
            header: MimeHeaderBlock {
                items: moved,
                end: Cow::Borrowed(eol),
            },
            body: mem::take(&mut self.body),
            anomalies: Default::default(),
        };
        child.header.terminate();

        // NOTE: a digest part with no Content-Type is a message, which it
        // would no longer be under the new multipart.
        let implicit = child.header.get("content-type").is_none()
            && match &child.body {
                MimeBodyNode::Message(_) => true,
                MimeBodyNode::Unparsed(unparsed) => unparsed.digest,
                MimeBodyNode::Leaf(_) | MimeBodyNode::Multipart(_) => false,
            };

        if implicit {
            let field = type_field("message", "rfc822", &opts)?;
            child
                .header
                .place(child.header.items.len(), MimeHeaderItem::Field(field));
        }

        let bytes = child.to_bytes();
        let boundary = Boundaries::new(taken).fresh(&[&bytes]);

        let value = MimeHeaderValue::ContentType(MimeContentType {
            top: String::from("multipart"),
            sub: String::from(sub),
            params: vec![MimeParam {
                name: String::from("boundary"),
                value: boundary.clone(),
            }],
        });
        let name = MimeHeaderName::Known(MimeHeaderKind::ContentType);
        let line = field_line(&name, &value, &opts)?;
        let field = owned_field(line, "Content-Type", eol)?;

        let at = first.unwrap_or(kept.len());
        self.header.items = kept;
        self.header.place(at, MimeHeaderItem::Field(field));
        self.header.terminate();

        let mut delimiter = Vec::from(format!("--{boundary}").as_bytes());
        delimiter.extend_from_slice(eol);

        let mut close = eol.to_vec();
        close.extend_from_slice(format!("--{boundary}--").as_bytes());
        close.extend_from_slice(eol);

        self.body = MimeBodyNode::Multipart(Box::new(MimeMultipart {
            preamble: Cow::Borrowed(&[]),
            parts: vec![MimePart {
                delimiter: Cow::Owned(delimiter),
                entity: child,
            }],
            close: Cow::Owned(close),
            epilogue: Cow::Borrowed(&[]),
        }));

        if let MimeBodyNode::Multipart(multipart) = &mut self.body {
            separate(multipart, eol);
        }

        Ok(())
    }

    /// Replaces a multipart of one part by that part: the multipart's
    /// Content- fields give way to the part's, its other fields stay.
    pub(crate) fn unwrap(&mut self) -> Result<(), MimeEditError> {
        match &self.body {
            MimeBodyNode::Multipart(multipart) if multipart.parts.len() == 1 => self.unwrap_part(0),
            MimeBodyNode::Multipart(multipart) => {
                Err(MimeEditError::NoSuchItem(multipart.parts.len()))
            }
            _ => Err(MimeEditError::NotAMultipart),
        }
    }

    /// Replaces a multipart by its part at `position`, as
    /// [`unwrap`](Self::unwrap) does.
    pub(crate) fn unwrap_part(&mut self, position: usize) -> Result<(), MimeEditError> {
        let digest = self.content_type().is("multipart", "digest");
        let MimeBodyNode::Multipart(multipart) = &mut self.body else {
            return Err(MimeEditError::NotAMultipart);
        };

        if position >= multipart.parts.len() {
            return Err(MimeEditError::NoSuchItem(position));
        }

        let mut part = multipart.parts.remove(position).entity;

        // NOTE: the part read as the multipart it leaves defaulted it, which
        // is stated, since where it lands may default otherwise.
        if part.header.get("content-type").is_none() {
            let opts = options(&MimeBuildOptions::default(), part.header.line_ending());
            let (top, sub) = if digest {
                ("message", "rfc822")
            } else {
                ("text", "plain")
            };
            let field = type_field(top, sub, &opts)?;
            part.header
                .place(part.header.items.len(), MimeHeaderItem::Field(field));
        }

        let mut items = Vec::with_capacity(self.header.items.len() + part.header.items.len());
        let mut first = None;

        for item in mem::take(&mut self.header.items) {
            match &item {
                MimeHeaderItem::Field(field) if is_content(field) => {
                    first.get_or_insert(items.len());
                }
                _ => items.push(item),
            }
        }

        let at = first.unwrap_or(items.len());
        let seam = at.checked_sub(1);
        items.splice(at..at, part.header.items);

        self.header.items = items;

        if let Some(seam) = seam {
            self.header.end_line(seam);
        }

        self.header.terminate();
        self.body = part.body;
        Ok(())
    }

    /// Replaces the boundary `old` by `new` in this multipart's delimiter
    /// lines and in its Content-Type boundary parameter.
    pub(crate) fn rewrite_boundary(
        &mut self,
        old: &str,
        new: &str,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let eol = self.header.line_ending();
        let MimeBodyNode::Multipart(multipart) = &mut self.body else {
            return Err(MimeEditError::NotAMultipart);
        };

        let old_line = format!("--{old}");
        let new_line = format!("--{new}");

        for part in &mut multipart.parts {
            part.delimiter = Cow::Owned(replace_once(&part.delimiter, &old_line, &new_line));
        }

        multipart.close = Cow::Owned(replace_once(&multipart.close, &old_line, &new_line));

        let index = self
            .header
            .items
            .iter()
            .rposition(
                |item| matches!(item, MimeHeaderItem::Field(field) if field.is("content-type")),
            )
            .ok_or(MimeEditError::NotAMultipart)?;

        let MimeHeaderItem::Field(field) = &self.header.items[index] else {
            return Err(MimeEditError::NotAMultipart);
        };

        let raw = field.value();
        let field = match param_span(raw, "boundary") {
            Some(span) => {
                let mut value = raw[..span.start].to_vec();
                value.extend_from_slice(format!("\"{new}\"").as_bytes());
                value.extend_from_slice(&raw[span.end..]);
                MimeHeaderField::from_parts(field.name(), field.colon(), &value, field.eol())
            }
            None => {
                let mut content_type = self.content_type().to_content_type();
                content_type
                    .params
                    .retain(|param| !param.name.eq_ignore_ascii_case("boundary"));
                content_type.params.push(MimeParam {
                    name: String::from("boundary"),
                    value: String::from(new),
                });

                let name = MimeHeaderName::Known(MimeHeaderKind::ContentType);
                let value = MimeHeaderValue::ContentType(content_type);
                owned_field(field_line(&name, &value, opts)?, "Content-Type", eol)?
            }
        };

        self.header.items[index] = MimeHeaderItem::Field(field);
        Ok(())
    }
}

/// A delimiter line for a new part, in the style of its siblings: a later
/// sibling's as written, else the line ending, the boundary and the line
/// ending.
fn sibling_delimiter(multipart: &MimeMultipart<'_>, boundary: &str, eol: &[u8]) -> Vec<u8> {
    multipart
        .parts
        .get(1)
        .map(|part| part.delimiter.to_vec())
        .unwrap_or_else(|| {
            let mut line = eol.to_vec();
            line.extend_from_slice(format!("--{boundary}").as_bytes());
            line.extend_from_slice(eol);
            line
        })
}

/// Gives every delimiter line past the first, and the close delimiter
/// line, the line ending before it they need to start a line.
pub(crate) fn separate(multipart: &mut MimeMultipart<'_>, eol: &[u8]) {
    let starts = |line: &[u8]| line.starts_with(b"\n") || line.starts_with(b"\r\n");

    for part in multipart.parts.iter_mut().skip(1) {
        if !starts(&part.delimiter) {
            let mut line = eol.to_vec();
            line.extend_from_slice(&part.delimiter);
            part.delimiter = Cow::Owned(line);
        }
    }

    let preamble = !multipart.preamble.is_empty();

    if let Some(first) = multipart.parts.first_mut()
        && preamble
        && !starts(&first.delimiter)
    {
        let mut line = eol.to_vec();
        line.extend_from_slice(&first.delimiter);
        first.delimiter = Cow::Owned(line);
    }

    let opened = !multipart.parts.is_empty() || preamble;

    if opened && !multipart.close.is_empty() && !starts(&multipart.close) {
        let mut line = eol.to_vec();
        line.extend_from_slice(&multipart.close);
        multipart.close = Cow::Owned(line);
    }

    // NOTE: a part ending in a CR would lose it to an LF line ending
    // before the delimiter after it, which would read as CRLF; a CRLF one
    // keeps it.
    for index in 0..multipart.parts.len() {
        let carriage = multipart.parts[index]
            .entity
            .chunks()
            .into_iter()
            .rev()
            .find(|chunk| !chunk.is_empty())
            .is_some_and(|chunk| chunk.ends_with(b"\r"));

        let next = match multipart.parts.get_mut(index + 1) {
            Some(part) => &mut part.delimiter,
            None => &mut multipart.close,
        };

        if carriage && next.starts_with(b"\n") {
            let mut line = b"\r".to_vec();
            line.extend_from_slice(next);
            *next = Cow::Owned(line);
        }
    }
}

fn replace_once(line: &[u8], from: &str, to: &str) -> Vec<u8> {
    let from = from.as_bytes();

    match line.windows(from.len()).position(|window| window == from) {
        Some(at) => {
            let mut out = line[..at].to_vec();
            out.extend_from_slice(to.as_bytes());
            out.extend_from_slice(&line[at + from.len()..]);
            out
        }
        None => line.to_vec(),
    }
}

fn is_content(field: &MimeHeaderField<'_>) -> bool {
    field
        .name()
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"content-"))
}

/// A field from a line the writer wrote for `name`.
fn owned_field(
    line: Vec<u8>,
    name: &str,
    eol: &[u8],
) -> Result<MimeHeaderField<'static>, MimeEditError> {
    let value_end = line.len() - eol.len();
    MimeHeaderField::new(Cow::Owned(line), name.len(), name.len() + 1, value_end)
        .ok_or(MimeEditError::Build(MimeBuildError::LineTooLong))
}

/// A Content-Type field stating `top/sub`, as the writer writes it.
fn type_field(
    top: &str,
    sub: &str,
    opts: &MimeBuildOptions,
) -> Result<MimeHeaderField<'static>, MimeEditError> {
    let value = MimeHeaderValue::ContentType(MimeContentType {
        top: String::from(top),
        sub: String::from(sub),
        params: Vec::new(),
    });
    let name = MimeHeaderName::Known(MimeHeaderKind::ContentType);
    let eol = opts.line_ending.as_bytes();
    owned_field(field_line(&name, &value, opts)?, "Content-Type", eol)
}
