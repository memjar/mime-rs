//! # Editing
//!
//! Edits on the syntax tree that rewrite only what they name: [`field`]
//! sets, inserts and removes header fields and the items of address
//! fields, [`content`] replaces a leaf's content, [`structure`] inserts,
//! removes, moves, wraps and unwraps parts, and [`transport`] fills the
//! required fields and downgrades a message to 7bit. Every byte an edit
//! writes follows the building rules, with the line ending of the block or
//! body it lands in; every leaf it does not write keeps borrowing the
//! input.
//!
//! An edit reached through [`MimeCst::header_mut`] or
//! [`MimeCst::entity_mut`] refuses to change content a signature or an
//! encryption protects (RFC 1847 2.1, RFC 3156 5): the first part of a
//! `multipart/signed` and the parts of a `multipart/encrypted`, and the
//! body of either. [`MimeCst::unwrap_protected`] removes such a layer
//! first.

pub mod content;
pub mod field;
pub mod structure;
pub mod transport;

use alloc::{borrow::Cow, string::String, vec::Vec};
use core::{error, fmt};

use crate::{
    build::{MimeBuildError, MimeBuildOptions, MimeLineEnding, entity::Boundaries},
    mime::Mime,
    tree::{
        anomaly::MimeAnomaly, body::MimeBodyNode, cst::MimeCst, entity::MimeEntity,
        header::MimeHeaderBlock,
    },
    value::transfer_encoding::MimeTransferEncoding,
};

/// Why an edit could not be made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeEditError {
    /// The edit would change content the entity at this index protects: a
    /// `multipart/signed` or a `multipart/encrypted`.
    Sealed(usize),
    /// No entity has this index in [`MimeCst::entities`].
    NoSuchEntity(usize),
    /// No field has this name.
    NoSuchField(String),
    /// No item, or no part, has this position.
    NoSuchItem(usize),
    /// The edit needs a multipart and the entity is not one.
    NotAMultipart,
    /// The edit needs a leaf and the entity is not one.
    NotALeaf,
    /// The edit would leave a multipart with no part, which RFC 2046 5.1.1
    /// does not allow.
    LastPart,
    /// The multipart was split at delimiters found mid-line, or holds none,
    /// a reading an edit writing a delimiter line would change.
    Damaged,
    /// What the edit writes could not be built.
    Build(MimeBuildError),
}

impl fmt::Display for MimeEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sealed(entity) => write!(f, "Entity {entity} protects the content to edit"),
            Self::NoSuchEntity(entity) => write!(f, "Cannot find entity {entity}"),
            Self::NoSuchField(name) => write!(f, "Cannot find field {name}"),
            Self::NoSuchItem(position) => write!(f, "Cannot find item {position}"),
            Self::NotAMultipart => f.write_str("Entity is not a multipart"),
            Self::NotALeaf => f.write_str("Entity is not a leaf"),
            Self::LastPart => f.write_str("Multipart keeps at least one part"),
            Self::Damaged => f.write_str("Multipart has damaged delimiters"),
            Self::Build(error) => write!(f, "Cannot write the edit: {error}"),
        }
    }
}

impl error::Error for MimeEditError {}

impl From<MimeBuildError> for MimeEditError {
    fn from(error: MimeBuildError) -> Self {
        Self::Build(error)
    }
}

impl<'a> MimeCst<'a> {
    /// The entity protecting the one at `index`, as indexed by
    /// [`entities`](Self::entities): the nearest `multipart/signed` whose
    /// first part holds it, or `multipart/encrypted` holding it.
    pub fn protector(&self, index: usize) -> Option<usize> {
        self.locate(index).and_then(|(_, protector)| protector)
    }

    /// The header block of the entity at `index`, for editing; refused
    /// for an entity inside protected content.
    pub fn header_mut(&mut self, index: usize) -> Result<&mut MimeHeaderBlock<'a>, MimeEditError> {
        let (path, protector) = self
            .locate(index)
            .ok_or(MimeEditError::NoSuchEntity(index))?;

        if let Some(protector) = protector {
            return Err(MimeEditError::Sealed(protector));
        }

        self.open(&path);
        Ok(&mut self.at_mut(&path).header)
    }

    /// The entity at `index`, for editing; refused for an entity inside
    /// protected content, and for a `multipart/signed` or a
    /// `multipart/encrypted` itself, whose body is the protected one.
    pub fn entity_mut(&mut self, index: usize) -> Result<&mut MimeEntity<'a>, MimeEditError> {
        let (path, protector) = self
            .locate(index)
            .ok_or(MimeEditError::NoSuchEntity(index))?;

        if let Some(protector) = protector {
            return Err(MimeEditError::Sealed(protector));
        }

        if Protection::of(self.at_mut(&path)).is_some() {
            return Err(MimeEditError::Sealed(index));
        }

        self.open(&path);
        Ok(self.at_mut(&path))
    }

    /// Replaces the `multipart/signed` or `multipart/encrypted` at `index`
    /// by its content entity (the signed part, or the encrypted data), its
    /// fields but the Content- ones kept, after which edits proceed.
    pub fn unwrap_protected(&mut self, index: usize) -> Result<(), MimeEditError> {
        let (path, protector) = self
            .locate(index)
            .ok_or(MimeEditError::NoSuchEntity(index))?;

        if let Some(protector) = protector {
            return Err(MimeEditError::Sealed(protector));
        }

        let entity = self.at_mut(&path);

        let position = match Protection::of(entity) {
            Some(Protection::Signed) => 0,
            Some(Protection::Encrypted) => 1,
            None => return Err(MimeEditError::NotAMultipart),
        };

        entity.unwrap_part(position)
    }

    /// Replaces the content of the leaf at `index`, `content` being in the
    /// leaf's own charset for a text leaf, under `encoding` when it can
    /// carry it or the one the building rules choose, and states the
    /// encoding in the entity's Content-Transfer-Encoding field.
    pub fn replace_content(
        &mut self,
        index: usize,
        content: &[u8],
        encoding: Option<&MimeTransferEncoding>,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let taken = self.enclosing(index);
        self.entity_mut(index)?
            .replace_content(content, encoding, &taken, opts)?;
        self.separate_around(index);
        Ok(())
    }

    /// Replaces the content of the text leaf at `index` with `text`,
    /// written in the leaf's declared charset when it can hold the text;
    /// otherwise in UTF-8, the Content-Type field then rewritten to say
    /// `charset=utf-8`.
    pub fn replace_text(
        &mut self,
        index: usize,
        text: &str,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let taken = self.enclosing(index);
        self.entity_mut(index)?.replace_text(text, &taken, opts)?;
        self.separate_around(index);
        Ok(())
    }

    /// Gives the delimiter line after the entity at `index` the line
    /// ending before it, which an empty part did not need.
    fn separate_around(&mut self, index: usize) {
        let Some((path, _)) = self.locate(index) else {
            return;
        };

        let Some((_, parent)) = path.split_last() else {
            return;
        };

        let parent = self.at_mut(parent);
        let eol = parent.header.line_ending();

        if let MimeBodyNode::Multipart(multipart) = &mut parent.body {
            structure::separate(multipart, eol);
        }
    }

    /// Inserts `part` at `position` (at the end past it) in the multipart
    /// at `index`, written by the building rules with the multipart's line
    /// ending; its boundary is regenerated only when the part holds it.
    pub fn insert_part(
        &mut self,
        index: usize,
        position: usize,
        part: &Mime,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let taken = self.prepare(index, opts)?;
        self.entity_mut(index)?
            .insert_part_within(position, part, &taken, opts)
    }

    /// Removes the part at `position` of the multipart at `index`, with its
    /// delimiter line, and returns it.
    pub fn remove_part(
        &mut self,
        index: usize,
        position: usize,
        opts: &MimeBuildOptions,
    ) -> Result<MimeEntity<'a>, MimeEditError> {
        self.prepare(index, opts)?;
        self.entity_mut(index)?.remove_part(position)
    }

    /// Moves the part at `from` of the multipart at `index` to `to`, with
    /// its delimiter line.
    pub fn move_part(
        &mut self,
        index: usize,
        from: usize,
        to: usize,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        self.prepare(index, opts)?;
        self.entity_mut(index)?.move_part(from, to)
    }

    /// Replaces the multipart at `index`, which must hold one part, by
    /// that part: the multipart's Content- fields give way to the part's
    /// (its default content type stated), its other fields stay.
    pub fn unwrap(&mut self, index: usize, opts: &MimeBuildOptions) -> Result<(), MimeEditError> {
        self.prepare(index, opts)?;
        self.entity_mut(index)?.unwrap()
    }

    /// Readies the entity at `index` for a structural edit, returning the
    /// boundaries enclosing it: a multipart read through fallback
    /// delimiters, or with none, is refused, since a delimiter line an edit
    /// writes would change how it splits, and one whose boundary conflicts with an
    /// enclosing one is given a fresh boundary first.
    fn prepare(
        &mut self,
        index: usize,
        opts: &MimeBuildOptions,
    ) -> Result<Vec<String>, MimeEditError> {
        let taken = self.enclosing(index);
        let entity = self.entity_mut(index)?;

        if !matches!(entity.body, MimeBodyNode::Multipart(_)) {
            return Ok(taken);
        }

        let damaged = entity.anomalies.contains(MimeAnomaly::FallbackDelimiter)
            || entity.anomalies.contains(MimeAnomaly::MissingDelimiter);

        if damaged {
            return Err(MimeEditError::Damaged);
        }

        let own = entity.content_type().param("boundary").unwrap_or_default();
        let mut boundaries = Boundaries::new(&taken);

        if !boundaries.allows(&own) {
            let bytes = entity.to_bytes();
            let fresh = boundaries.fresh(&[&bytes]);

            let opts = options(opts, entity.header.line_ending());
            entity.rewrite_boundary(&own, &fresh, &opts)?;
        }

        Ok(taken)
    }

    /// Wraps the entity at `index` into a new `multipart/<sub>`: its
    /// Content- fields move, in order, to the new first part, whose body is
    /// the entity's body unchanged, and its other fields stay.
    pub fn wrap(
        &mut self,
        index: usize,
        sub: &str,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let taken = self.prepare(index, opts)?;
        self.entity_mut(index)?.wrap_within(sub, &taken, opts)
    }

    /// Adds `part` as an attachment of the entity at `index`: at the end of
    /// a `multipart/mixed`, or after the entity's content once it is
    /// wrapped into one.
    pub fn attach(
        &mut self,
        index: usize,
        part: &Mime,
        opts: &MimeBuildOptions,
    ) -> Result<(), MimeEditError> {
        let taken = self.prepare(index, opts)?;
        let entity = self.entity_mut(index)?;

        if !entity.content_type().is("multipart", "mixed") {
            entity.wrap_within("mixed", &taken, opts)?;
        }

        entity.insert_part_within(usize::MAX, part, &taken, opts)
    }

    /// The boundaries of the multiparts enclosing the entity at `index`,
    /// which a boundary written inside it must avoid.
    fn enclosing(&self, index: usize) -> Vec<String> {
        let Some((path, _)) = self.locate(index) else {
            return Vec::new();
        };

        let mut boundaries = Vec::new();
        let mut entity = &self.root;

        for &position in &path {
            entity = match &entity.body {
                MimeBodyNode::Multipart(multipart) => {
                    boundaries.extend(entity.content_type().param("boundary"));
                    &multipart.parts[position].entity
                }
                MimeBodyNode::Message(nested) => nested,
                MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => break,
            };
        }

        boundaries
    }

    /// The child positions leading to the entity at `index`, and its
    /// nearest protector.
    fn locate(&self, target: usize) -> Option<(Vec<usize>, Option<usize>)> {
        let mut stack = Vec::from([(&self.root, Vec::new(), None)]);
        let mut index = 0;

        while let Some((entity, path, protector)) = stack.pop() {
            if index == target {
                return Some((path, protector));
            }

            let here = index;
            index += 1;

            match &entity.body {
                MimeBodyNode::Multipart(multipart) => {
                    let protection = Protection::of(entity);

                    for (position, part) in multipart.parts.iter().enumerate().rev() {
                        let protector = match protection {
                            Some(Protection::Signed) if position == 0 => Some(here),
                            Some(Protection::Encrypted) => Some(here),
                            _ => protector,
                        };

                        let mut path = path.clone();
                        path.push(position);
                        stack.push((&part.entity, path, protector));
                    }
                }
                MimeBodyNode::Message(nested) => {
                    let mut path = path.clone();
                    path.push(0);
                    stack.push((nested, path, protector));
                }
                MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => {}
            }
        }

        None
    }

    /// Gives the delimiter line opening the part at `path` a line ending
    /// when it has none, as at the end of the input, so that what an edit
    /// writes into the part starts a line of its own.
    fn open(&mut self, path: &[usize]) {
        let Some((&position, parent)) = path.split_last() else {
            return;
        };

        let parent = self.at_mut(parent);
        let eol = parent.header.line_ending();

        if let MimeBodyNode::Multipart(multipart) = &mut parent.body
            && let Some(part) = multipart.parts.get_mut(position)
            && !part.delimiter.ends_with(b"\n")
        {
            let mut line = part.delimiter.to_vec();
            line.extend_from_slice(eol);
            part.delimiter = Cow::Owned(line);
        }
    }

    fn at_mut(&mut self, path: &[usize]) -> &mut MimeEntity<'a> {
        let mut entity = &mut self.root;

        for &position in path {
            entity = match &mut entity.body {
                MimeBodyNode::Multipart(multipart) => &mut multipart.parts[position].entity,
                MimeBodyNode::Message(nested) => nested,
                MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => unreachable!("a located path"),
            };
        }

        entity
    }
}

/// What a protecting multipart protects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Protection {
    /// Its first part (RFC 1847 2.1).
    Signed,
    /// All its parts (RFC 1847 2.2).
    Encrypted,
}

impl Protection {
    pub(crate) fn of(entity: &MimeEntity<'_>) -> Option<Self> {
        let content_type = entity.content_type();

        if content_type.is("multipart", "signed") {
            Some(Self::Signed)
        } else if content_type.is("multipart", "encrypted") {
            Some(Self::Encrypted)
        } else {
            None
        }
    }
}

/// The options of an edit, with the line ending `eol` of what it lands in.
fn options(opts: &MimeBuildOptions, eol: &[u8]) -> MimeBuildOptions {
    MimeBuildOptions {
        line_ending: if eol == b"\n" {
            MimeLineEnding::Lf
        } else {
            MimeLineEnding::Crlf
        },
        ..opts.clone()
    }
}
