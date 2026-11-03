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
    /// Parses any bytes into a tree under the default bounds; never fails.
    ///
    /// The parse coroutine's searches are answered through memchr.
    pub fn parse(input: &'a [u8]) -> Self {
        Self::parse_with(input, MimeParseOptions::default())
    }

    /// Parses like [`parse`](Self::parse), under the given bounds.
    pub fn parse_with(input: &'a [u8], opts: MimeParseOptions) -> Self {
        answer(input, MimeParse::new(input, opts))
    }

    /// Splits every unparsed body through its entity's [`MimeEntity::expand`].
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

    /// Every byte of the message, in order, as borrowed slices.
    ///
    /// Their concatenation is [`to_bytes`](Self::to_bytes).
    pub fn chunks(&self) -> Vec<&[u8]> {
        self.root.chunks()
    }

    /// The message serialized back to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.root.to_bytes()
    }

    /// Every entity, the root first, in document order.
    pub fn entities(&self) -> Vec<&MimeEntity<'a>> {
        let mut out = Vec::with_capacity(8);
        push_entities(&self.root, &mut out);
        out
    }

    /// Every anomaly the parse recorded, with the index of its entity.
    ///
    /// The index is into [`entities`](Self::entities).
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

/// Pushes `entity`, then every entity under it in document order.
// NOTE: the walk recurses as deep as the tree, as the serializer and the
// tree's own drop do.
fn push_entities<'s, 'a>(entity: &'s MimeEntity<'a>, out: &mut Vec<&'s MimeEntity<'a>>) {
    out.push(entity);

    match &entity.body {
        MimeBodyNode::Multipart(multipart) => {
            for part in &multipart.parts {
                push_entities(&part.entity, out);
            }
        }
        MimeBodyNode::Message(nested) => push_entities(nested, out),
        MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => {}
    }
}

/// Runs a parse coroutine over `input`, answering its searches through memchr.
pub(crate) fn answer<'a>(input: &'a [u8], mut parse: MimeParse<'a>) -> MimeCst<'a> {
    let mut found = Vec::with_capacity(64);
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
