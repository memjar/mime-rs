//! # Field edits
//!
//! Setting, inserting and removing the fields of a header block, and
//! adding, changing and removing one item of an address field. A written
//! field is one owned line, encoded and folded by the building rules with
//! the block's own line ending; every other field keeps its bytes.
//!
//! A new field goes after the last field of its name, else at the end of
//! the block, except a trace field (Received, Return-Path), which goes at
//! the top (RFC 5321 4.4).

use alloc::{borrow::Cow, string::String, vec, vec::Vec};

use crate::{
    build::{MimeBuildError, MimeBuildOptions, entity::field_line, fold::LINE_TARGET},
    header::{MimeHeaderKind, MimeHeaderName, marker::MimeHeaderMarker},
    tree::{
        edit::{MimeEditError, options},
        field::address::{is_item, item_spans},
        header::{MimeHeaderBlock, MimeHeaderField, MimeHeaderItem},
    },
    value::{
        MimeHeaderValue,
        address::{MimeAddress, MimeAddressList},
    },
};

impl<'a> MimeHeaderBlock<'a> {
    /// Sets the first field the marker names to `value`, rewriting that
    /// field alone, or inserts it when the block has none.
    pub fn set<M: MimeHeaderMarker>(
        &mut self,
        value: M::Value,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeBuildError> {
        self.set_value(&MimeHeaderName::Known(M::KIND), &M::into_value(value), opts)
    }

    /// Inserts a field the marker names, where it belongs.
    pub fn insert<M: MimeHeaderMarker>(
        &mut self,
        value: M::Value,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeBuildError> {
        self.insert_value(&MimeHeaderName::Known(M::KIND), &M::into_value(value), opts)
    }

    /// Sets the first field named `name` to `value`, rewriting that field
    /// alone, or inserts it when the block has none. Content-Type and
    /// Content-Transfer-Encoding are refused: they state how the body reads,
    /// and change through the content and structural edits.
    pub fn set_value(
        &mut self,
        name: &MimeHeaderName,
        value: &MimeHeaderValue,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeBuildError> {
        structural(name.as_str())?;
        self.set_field(name, value, opts)
    }

    /// Inserts a field named `name`, after the last field of that name,
    /// else at the end of the block, or at the top for a trace field. The
    /// structural fields are refused, as [`set_value`](Self::set_value)
    /// refuses them.
    pub fn insert_value(
        &mut self,
        name: &MimeHeaderName,
        value: &MimeHeaderValue,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeBuildError> {
        structural(name.as_str())?;
        self.insert_field(name, value, opts)
    }

    /// [`set_value`](Self::set_value) for any field, the structural ones
    /// included.
    pub(crate) fn set_field(
        &mut self,
        name: &MimeHeaderName,
        value: &MimeHeaderValue,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeBuildError> {
        let Some(index) = self.position(name.as_str()) else {
            return self.insert_field(name, value, opts);
        };

        let field = self.written(name, value, opts)?;
        self.items[index] = MimeHeaderItem::Field(field);
        Ok(())
    }

    /// [`insert_value`](Self::insert_value) for any field, the structural
    /// ones included.
    pub(crate) fn insert_field(
        &mut self,
        name: &MimeHeaderName,
        value: &MimeHeaderValue,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeBuildError> {
        let field = self.written(name, value, opts)?;
        let trace = matches!(
            name.kind(),
            Some(MimeHeaderKind::Received | MimeHeaderKind::ReturnPath)
        );

        let last = self.items.iter().rposition(
            |item| matches!(item, MimeHeaderItem::Field(field) if field.is(name.as_str())),
        );

        // NOTE: the end of the block is after its last field, before any
        // opaque line trailing it, which a line ending could turn blank.
        let fields_end = self
            .items
            .iter()
            .rposition(|item| matches!(item, MimeHeaderItem::Field(_)))
            .map_or(self.items.len(), |last| last + 1);

        let position = match last {
            Some(last) => last + 1,
            None if trace => 0,
            None => fields_end,
        };

        self.place(position, MimeHeaderItem::Field(field));
        Ok(())
    }

    /// Removes every field named `name`, its continuation lines included,
    /// and returns how many there were. When no field is left, the opaque
    /// lines go too, since a block of them alone would read as body. The
    /// structural fields are refused.
    pub fn remove_all(&mut self, name: &str) -> Result<usize, MimeBuildError> {
        structural(name)?;

        let before = self.fields().count();
        let mut kept: Vec<MimeHeaderItem<'a>> = Vec::with_capacity(self.items.len());

        for item in self.items.drain(..) {
            if !matches!(&item, MimeHeaderItem::Field(field) if field.is(name)) {
                kept.push(item);
                continue;
            }

            // NOTE: a field folded after a line holding no field leaves that
            // opaque line its folding blanks; they go with the field, else
            // they would fold the line after it in.
            if let Some(MimeHeaderItem::Opaque(line)) = kept.last_mut() {
                let tail = line
                    .iter()
                    .rposition(|byte| *byte == b'\n')
                    .map_or(0, |lf| lf + 1);

                if line[tail..].iter().all(|byte| matches!(byte, b' ' | b'\t')) {
                    match line {
                        Cow::Borrowed(bytes) => *bytes = &bytes[..tail],
                        Cow::Owned(bytes) => bytes.truncate(tail),
                    }
                }
            }

            if matches!(kept.last(), Some(MimeHeaderItem::Opaque(line)) if line.is_empty()) {
                kept.pop();
            }
        }

        self.items = kept;
        let after = self.fields().count();

        if after == 0 {
            self.items.clear();
        }

        Ok(before - after)
    }

    /// Appends an item (a mailbox or a group) to the first address field
    /// named `name`, keeping every byte of its other items, or inserts the
    /// field when the block has none.
    pub fn push_address(
        &mut self,
        name: &str,
        item: &MimeAddress,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let Some(index) = self.position(name) else {
            let list = MimeAddressList {
                items: vec![item.clone()],
            };
            let value = MimeHeaderValue::Addresses(list);
            return Ok(self.insert_value(&MimeHeaderName::new(name), &value, opts)?);
        };

        let field = self.field(index);
        let written = self.item_bytes(item, opts)?;
        let raw = field.value();
        let mut value = Vec::with_capacity(raw.len() + written.len() + 4);

        if item_spans(raw)
            .iter()
            .any(|span| is_item(&raw[span.clone()]))
        {
            let last_line = field.line()[..field.colon().len() + field.name().len() + raw.len()]
                .rsplit(|byte| *byte == b'\n')
                .next()
                .map_or(0, <[u8]>::len);

            value.extend_from_slice(raw);
            value.push(b',');

            if last_line + 2 + written.len() > LINE_TARGET {
                value.extend_from_slice(self.eol(field));
            }
        }

        value.push(b' ');
        value.extend_from_slice(&written);
        self.rewrite(index, &value);
        Ok(())
    }

    /// Replaces the item at `position` of the first address field named
    /// `name`, keeping the blanks around it and every byte of the others.
    pub fn set_address(
        &mut self,
        name: &str,
        position: usize,
        item: &MimeAddress,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let index = self
            .position(name)
            .ok_or_else(|| MimeEditError::NoSuchField(String::from(name)))?;

        let written = self.item_bytes(item, opts)?;
        let raw = self.field(index).value();
        let spans = item_spans(raw);
        let span = nth_item(raw, &spans, position).ok_or(MimeEditError::NoSuchItem(position))?;
        let span = spans[span].clone();

        let item = &raw[span.clone()];
        let lead = item.len() - trim_start(item).len();
        let trail = item.len() - trim_end(item).len();

        let mut value = Vec::with_capacity(raw.len() + written.len());
        value.extend_from_slice(&raw[..span.start + lead]);
        value.extend_from_slice(&written);
        value.extend_from_slice(&raw[span.end - trail..]);
        self.rewrite(index, &value);
        Ok(())
    }

    /// Removes the item at `position` of the first address field named
    /// `name` with the comma separating it, keeping every byte of the
    /// others; removing the last item removes the field.
    pub fn remove_address(&mut self, name: &str, position: usize) -> Result<(), MimeEditError> {
        let index = self
            .position(name)
            .ok_or_else(|| MimeEditError::NoSuchField(String::from(name)))?;

        let raw = self.field(index).value();
        let spans = item_spans(raw);
        let found = nth_item(raw, &spans, position).ok_or(MimeEditError::NoSuchItem(position))?;
        let span = spans[found].clone();

        if spans
            .iter()
            .filter(|span| is_item(&raw[(*span).clone()]))
            .count()
            == 1
        {
            self.items.remove(index);

            if self.fields().next().is_none() {
                self.items.clear();
            }

            return Ok(());
        }

        let mut value = Vec::with_capacity(raw.len());

        if found + 1 < spans.len() {
            // NOTE: the comma after the item goes with it; the first item
            // keeps its own leading blanks, the next one loses its own.
            let item = &raw[span.clone()];
            let lead = item.len() - trim_start(item).len();
            let next = &raw[span.end + 1..];

            value.extend_from_slice(&raw[..span.start]);

            if found == 0 {
                value.extend_from_slice(&item[..lead]);
                value.extend_from_slice(trim_start(next));
            } else {
                value.extend_from_slice(next);
            }
        } else {
            let comma = spans[found - 1].end;
            value.extend_from_slice(&raw[..comma]);
        }

        self.rewrite(index, &value);
        Ok(())
    }

    /// The index in `items` of the first field named `name`.
    fn position(&self, name: &str) -> Option<usize> {
        self.items
            .iter()
            .position(|item| matches!(item, MimeHeaderItem::Field(field) if field.is(name)))
    }

    fn field(&self, index: usize) -> &MimeHeaderField<'a> {
        match &self.items[index] {
            MimeHeaderItem::Field(field) => field,
            MimeHeaderItem::Opaque(_) => unreachable!("a field position"),
        }
    }

    /// The line ending a rewritten field ends with: its own, else the
    /// block's.
    fn eol(&self, field: &MimeHeaderField<'_>) -> &'static [u8] {
        match field.eol() {
            b"\n" => b"\n",
            b"\r\n" => b"\r\n",
            _ => self.line_ending(),
        }
    }

    /// Writes a field with the block's line ending.
    fn written(
        &self,
        name: &MimeHeaderName,
        value: &MimeHeaderValue,
        opts: &MimeBuildOptions,
    ) -> Result<MimeHeaderField<'static>, MimeBuildError> {
        let eol = self.line_ending();
        let line = field_line(name, value, &options(opts, eol))?;
        let name_end = name.as_str().len();
        let value_end = line.len() - eol.len();

        MimeHeaderField::new(Cow::Owned(line), name_end, name_end + 1, value_end)
            .ok_or(MimeBuildError::LineTooLong)
    }

    /// Rewrites the value of the field at `index` as `value`, its name and
    /// colon kept as written.
    fn rewrite(&mut self, index: usize, value: &[u8]) {
        let field = self.field(index);
        let eol = self.eol(field);
        let field = MimeHeaderField::from_parts(field.name(), field.colon(), value, eol);
        self.items[index] = MimeHeaderItem::Field(field);
    }

    /// The bytes of one address item as written, without leading blank.
    fn item_bytes(
        &self,
        item: &MimeAddress,
        opts: &MimeBuildOptions,
    ) -> Result<Vec<u8>, MimeBuildError> {
        let eol = self.line_ending();
        let list = MimeAddressList {
            items: vec![item.clone()],
        };
        let line = field_line(
            &MimeHeaderName::new("X"),
            &MimeHeaderValue::Addresses(list),
            &options(opts, eol),
        )?;

        let value = &line[2..line.len() - eol.len()];
        Ok(trim_start(value).to_vec())
    }

    /// Inserts an item at `position`, giving the line before it a line
    /// ending when it has none, and the block an empty line when it had no
    /// field and none, so that what follows still reads as the body.
    pub(crate) fn place(&mut self, position: usize, item: MimeHeaderItem<'a>) {
        let eol = self.line_ending();
        let empty = !self
            .items
            .iter()
            .any(|item| matches!(item, MimeHeaderItem::Field(_)));

        if position > 0 {
            self.end_line(position - 1);
        }

        self.items.insert(position, item);

        // NOTE: a blank line holding spaces ends a block of no field, but
        // reads as a continuation once a field precedes it.
        if empty && !matches!(self.end.as_ref(), b"\n" | b"\r\n") {
            self.end = Cow::Borrowed(eol);
        }
    }
}

impl MimeHeaderBlock<'_> {
    /// Ends the block so that a body can follow it: its last line given a
    /// line ending when it has none, and the empty line added when it is
    /// missing.
    pub(crate) fn terminate(&mut self) {
        // NOTE: a trailing opaque line of blanks would end the block once
        // given a line ending, so it becomes the empty line itself.
        if self.end.is_empty()
            && let Some(MimeHeaderItem::Opaque(line)) = self.items.last()
            && line
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\x0c' | b'\n'))
        {
            let mut end = line.to_vec();

            if !end.ends_with(b"\n") {
                end.extend_from_slice(b"\n");
            }

            self.items.pop();
            self.end = Cow::Owned(end);
        }

        if let Some(last) = self.items.len().checked_sub(1) {
            self.end_line(last);
        }

        if self.end.is_empty() {
            self.end = Cow::Borrowed(self.line_ending());
        }
    }

    /// Gives the item at `index` a line ending when it has none.
    pub(crate) fn end_line(&mut self, index: usize) {
        let eol = self.line_ending();

        let Some(item) = self.items.get_mut(index) else {
            return;
        };

        let ends = match item {
            MimeHeaderItem::Field(field) => !field.eol().is_empty(),
            MimeHeaderItem::Opaque(line) => line.ends_with(b"\n"),
        };

        if ends {
            return;
        }

        // NOTE: a line ending in a CR would lose it to an LF after it, which
        // reads as CRLF; a CRLF keeps it.
        let carriage = match item {
            MimeHeaderItem::Field(field) => field.line().ends_with(b"\r"),
            MimeHeaderItem::Opaque(line) => line.ends_with(b"\r"),
        };
        let eol: &[u8] = if carriage { b"\r\n" } else { eol };

        *item = match item {
            MimeHeaderItem::Field(field) => MimeHeaderItem::Field(MimeHeaderField::from_parts(
                field.name(),
                field.colon(),
                field.value(),
                eol,
            )),
            MimeHeaderItem::Opaque(line) => {
                let mut line = line.to_vec();
                line.extend_from_slice(eol);
                MimeHeaderItem::Opaque(Cow::Owned(line))
            }
        };
    }
}

/// Refuses Content-Type and Content-Transfer-Encoding, which a field edit
/// may not change.
fn structural(name: &str) -> Result<(), MimeBuildError> {
    match MimeHeaderName::new(name).kind() {
        Some(kind @ (MimeHeaderKind::ContentType | MimeHeaderKind::ContentTransferEncoding)) => {
            Err(MimeBuildError::Reserved(kind))
        }
        _ => Ok(()),
    }
}

/// The index in `spans` of the item at `position`, empty items skipped.
fn nth_item(raw: &[u8], spans: &[core::ops::Range<usize>], position: usize) -> Option<usize> {
    spans
        .iter()
        .enumerate()
        .filter(|(_, span)| is_item(&raw[(*span).clone()]))
        .nth(position)
        .map(|(index, _)| index)
}

fn trim_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn trim_end(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        .map_or(0, |end| end + 1);
    &bytes[..end]
}
