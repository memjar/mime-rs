//! # Header block
//!
//! The byte-faithful header block of an entity: its fields and opaque lines
//! in source order, then the empty line ending the block.
//!
//! Every piece is a [`Cow`] borrowed from the input until an edit replaces
//! it, so an edit owns the bytes it writes and nothing else. Values keep
//! their folds; [`MimeHeaderField::unfolded`] resolves them on read.

use alloc::{borrow::Cow, vec::Vec};

use crate::{
    header::{MimeHeaderKind, spec::MimeHeaderForm},
    tree::field::value,
    value::MimeHeaderValue,
};

/// The header block of an entity.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-2.2>
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeHeaderBlock<'a> {
    /// The fields and opaque lines, in source order.
    pub items: Vec<MimeHeaderItem<'a>>,
    /// The empty line ending the block (`\r\n` or `\n`), empty when the
    /// block ran into the end of its entity.
    pub end: Cow<'a, [u8]>,
}

/// One item of a header block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeHeaderItem<'a> {
    /// A header field.
    Field(MimeHeaderField<'a>),
    /// A line that is neither a field nor a continuation, its line ending
    /// included, kept so the block still round-trips.
    Opaque(Cow<'a, [u8]>),
}

/// A header field as written: one leaf holding the whole line (name, colon,
/// raw value with its folds, line ending), cut by three offsets, so a parsed
/// field costs one borrowed slice and an edit owns exactly its own line.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-2.2>
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimeHeaderField<'a> {
    line: Cow<'a, [u8]>,
    name_end: u32,
    colon_end: u32,
    value_end: u32,
}

impl<'a> MimeHeaderBlock<'a> {
    /// The fields of the block, in source order.
    pub fn fields(&self) -> impl Iterator<Item = &MimeHeaderField<'a>> {
        self.items.iter().filter_map(|item| match item {
            MimeHeaderItem::Field(field) => Some(field),
            MimeHeaderItem::Opaque(_) => None,
        })
    }

    /// The first field named `name`, compared ASCII case-insensitively.
    pub fn get(&self, name: &str) -> Option<&MimeHeaderField<'a>> {
        self.fields().find(|field| field.is(name))
    }

    /// Every field named `name`, compared ASCII case-insensitively.
    pub fn get_all<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s MimeHeaderField<'a>> {
        self.fields().filter(move |field| field.is(name))
    }

    /// The line ending the block uses: that of its first field, or `\r\n`.
    pub fn line_ending(&self) -> &'static [u8] {
        let eol = self.fields().map(|field| field.eol()).next();

        match eol {
            Some(b"\n") => b"\n",
            _ if self.end.as_ref() == b"\n" => b"\n",
            _ => b"\r\n",
        }
    }

    /// Whether the block holds no item and no end line.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.end.is_empty()
    }

    pub(crate) fn push_chunks<'s>(&'s self, out: &mut Vec<&'s [u8]>) {
        for item in &self.items {
            match item {
                MimeHeaderItem::Field(field) => out.push(field.line()),
                MimeHeaderItem::Opaque(line) => out.push(line),
            }
        }

        out.push(&self.end);
    }
}

impl MimeHeaderField<'_> {
    /// Whether the field is named `name`, compared ASCII case-insensitively.
    ///
    /// A name holding a space, a control byte or a NUL never matches, so a
    /// smuggled `Sub ject` is never read as `Subject`.
    pub fn is(&self, name: &str) -> bool {
        self.name().eq_ignore_ascii_case(name.as_bytes())
    }

    /// The name as text, lossily when it is not UTF-8.
    pub fn name_str(&self) -> Cow<'_, str> {
        alloc::string::String::from_utf8_lossy(self.name())
    }

    /// The raw value with its folds removed (RFC 5322 2.2.3): every line
    /// break followed by a blank is dropped, the blank kept.
    pub fn unfolded(&self) -> Cow<'_, [u8]> {
        unfold(self.value())
    }
}

/// Removes the line breaks of a folded value, keeping the blanks after them.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-2.2.3>
pub fn unfold(value: &[u8]) -> Cow<'_, [u8]> {
    if !value.contains(&b'\n') {
        return Cow::Borrowed(value);
    }

    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;

    while let Some(&byte) = value.get(i) {
        match byte {
            b'\n' => i += 1,
            b'\r' if value.get(i + 1) == Some(&b'\n') => i += 2,
            _ => {
                out.push(byte);
                i += 1;
            }
        }
    }

    Cow::Owned(out)
}

impl<'a> MimeHeaderBlock<'a> {
    /// The block with every leaf owned, free of the input's lifetime.
    pub fn into_owned(self) -> MimeHeaderBlock<'static> {
        MimeHeaderBlock {
            items: self
                .items
                .into_iter()
                .map(|item| match item {
                    MimeHeaderItem::Field(field) => MimeHeaderItem::Field(field.into_owned()),
                    MimeHeaderItem::Opaque(line) => {
                        MimeHeaderItem::Opaque(Cow::Owned(line.into_owned()))
                    }
                })
                .collect(),
            end: Cow::Owned(self.end.into_owned()),
        }
    }
}

impl MimeHeaderField<'_> {
    /// Reads the value in the default form of the field's name: its spec's
    /// for a known field, raw for any other.
    pub fn read(&self) -> MimeHeaderValue {
        let form = core::str::from_utf8(self.name())
            .ok()
            .and_then(|name| name.parse::<MimeHeaderKind>().ok())
            .map_or(MimeHeaderForm::Raw, |kind| kind.spec().form);

        value::read(form, self.value())
    }

    /// Reads the value in any form, as RFC 8621 4.1.2 lets a client ask.
    pub fn read_as(&self, form: MimeHeaderForm) -> MimeHeaderValue {
        value::read(form, self.value())
    }

    /// The field with every leaf owned, free of the input's lifetime.
    pub fn into_owned(self) -> MimeHeaderField<'static> {
        MimeHeaderField {
            line: Cow::Owned(self.line.into_owned()),
            name_end: self.name_end,
            colon_end: self.colon_end,
            value_end: self.value_end,
        }
    }
}

impl<'a> MimeHeaderField<'a> {
    /// Builds a field from its whole line and where its name, its colon
    /// and its value end in it; `None` when the offsets do not cut the line
    /// in order.
    pub fn new(
        line: Cow<'a, [u8]>,
        name_end: usize,
        colon_end: usize,
        value_end: usize,
    ) -> Option<Self> {
        let in_order = name_end <= colon_end && colon_end <= value_end && value_end <= line.len();

        if !in_order || line.len() > u32::MAX as usize {
            return None;
        }

        Some(Self {
            line,
            name_end: name_end as u32,
            colon_end: colon_end as u32,
            value_end: value_end as u32,
        })
    }

    /// Builds a field from its four pieces, joined into one owned line.
    pub fn from_parts(
        name: &[u8],
        colon: &[u8],
        value: &[u8],
        eol: &[u8],
    ) -> MimeHeaderField<'static> {
        let mut line = Vec::with_capacity(name.len() + colon.len() + value.len() + eol.len());
        line.extend_from_slice(name);
        line.extend_from_slice(colon);
        line.extend_from_slice(value);
        line.extend_from_slice(eol);

        MimeHeaderField {
            line: Cow::Owned(line),
            name_end: name.len() as u32,
            colon_end: (name.len() + colon.len()) as u32,
            value_end: (name.len() + colon.len() + value.len()) as u32,
        }
    }

    /// The whole field as written, its line ending included.
    pub fn line(&self) -> &[u8] {
        &self.line
    }

    /// The field name, without the blanks before the colon.
    pub fn name(&self) -> &[u8] {
        &self.line[..self.name_end as usize]
    }

    /// The blanks before the colon (the RFC 5322 4.5 obsolete syntax), then
    /// the colon.
    pub fn colon(&self) -> &[u8] {
        &self.line[self.name_end as usize..self.colon_end as usize]
    }

    /// The raw value after the colon, its folds and leading blanks kept,
    /// without the final line ending.
    pub fn value(&self) -> &[u8] {
        &self.line[self.colon_end as usize..self.value_end as usize]
    }

    /// The line ending (`\r\n` or `\n`), empty at the end of the input.
    pub fn eol(&self) -> &[u8] {
        &self.line[self.value_end as usize..]
    }
}
