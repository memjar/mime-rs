//! # Concrete syntax tree
//!
//! [`MimeCst`], the byte-faithful tree of a message: its root entity and
//! everything nested under it, every leaf a [`Cow`] borrowed from the input
//! until an edit replaces it.
//!
//! [`Cow`]: alloc::borrow::Cow

use alloc::vec::Vec;

use crate::{
    mime::Mime,
    tree::{
        anomaly::MimeAnomaly,
        body::MimeBodyNode,
        coroutine::MimeCoroutineState,
        entity::MimeEntity,
        parse::{MimeParse, MimeParseOptions, MimeParseYield},
        scan::memchr,
    },
};

/// A message as a byte-faithful tree.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeCst<'a> {
    /// The root entity: the message itself.
    pub root: MimeEntity<'a>,
}

impl<'a> MimeCst<'a> {
    /// Parses any bytes into a tree, answering the parse coroutine's
    /// searches through memchr under the default bounds. Never fails.
    pub fn parse(input: &'a [u8]) -> Self {
        Self::parse_with(input, MimeParseOptions::default())
    }

    /// Parses like [`parse`](Self::parse), under the given bounds.
    pub fn parse_with(input: &'a [u8], opts: MimeParseOptions) -> Self {
        answer(input, MimeParse::new(input, opts))
    }

    /// Splits every body the parse left unparsed, as
    /// [`MimeEntity::expand`] does, each from its own entity.
    pub fn expand(&mut self, opts: MimeParseOptions) {
        let mut stack = Vec::from([&mut self.root]);

        while let Some(entity) = stack.pop() {
            if matches!(entity.body, MimeBodyNode::Unparsed(_)) {
                entity.expand(opts.clone());
                continue;
            }

            match &mut entity.body {
                MimeBodyNode::Multipart(multipart) => {
                    stack.extend(multipart.parts.iter_mut().map(|part| &mut part.entity));
                }
                MimeBodyNode::Message(nested) => stack.push(nested),
                MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => {}
            }
        }
    }

    /// Every byte of the message, in order, as borrowed slices; their
    /// concatenation is [`to_bytes`](Self::to_bytes).
    pub fn chunks(&self) -> Vec<&[u8]> {
        self.root.chunks()
    }

    /// The message serialized back to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.root.to_bytes()
    }

    /// Every entity, the root first, in document order.
    pub fn entities(&self) -> Vec<&MimeEntity<'a>> {
        let mut out = Vec::new();
        let mut stack = Vec::from([&self.root]);

        while let Some(entity) = stack.pop() {
            out.push(entity);

            match &entity.body {
                MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => {}
                MimeBodyNode::Multipart(multipart) => {
                    stack.extend(multipart.parts.iter().rev().map(|part| &part.entity));
                }
                MimeBodyNode::Message(nested) => stack.push(nested),
            }
        }

        out
    }

    /// Every anomaly the parse recorded, with the index of its entity in
    /// [`entities`](Self::entities).
    pub fn anomalies(&self) -> Vec<(usize, MimeAnomaly)> {
        self.entities()
            .into_iter()
            .enumerate()
            .flat_map(|(index, entity)| {
                entity
                    .anomalies
                    .iter()
                    .map(move |anomaly| (index, anomaly))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl MimeCst<'_> {
    /// Projects the message onto the decoded model.
    pub fn decode(&self) -> Mime {
        self.root.decode()
    }

    /// The tree with every leaf owned, free of the input's lifetime.
    pub fn into_owned(self) -> MimeCst<'static> {
        MimeCst {
            root: self.root.into_owned(),
        }
    }
}

/// Runs a parse coroutine to completion over `input`, answering its
/// searches through memchr.
pub(crate) fn answer<'a>(input: &'a [u8], mut parse: MimeParse<'a>) -> MimeCst<'a> {
    let mut found = Vec::new();
    let mut arg = None;

    loop {
        match parse.resume(arg.take()) {
            MimeCoroutineState::Yielded(MimeParseYield::WantsFieldEnds { range }) => {
                found.clear();
                memchr::field_ends(input, range, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Yielded(MimeParseYield::WantsDashLines { range }) => {
                found.clear();
                memchr::dash_lines(input, range, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Yielded(MimeParseYield::WantsFind { range, needle }) => {
                found.clear();
                memchr::find(input, range, &needle, &mut found);
                arg = Some(found.as_slice());
            }
            MimeCoroutineState::Complete(cst) => break cst,
        }
    }
}
