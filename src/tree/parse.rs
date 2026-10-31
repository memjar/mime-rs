//! # Parse coroutine
//!
//! The I/O-free coroutine turning bytes into a [`MimeCst`]. It performs no
//! structural search of its own: it yields bulk requests (every field end
//! of one header block, every dash line of one body, one needle in one
//! range) and resumes with the positions the caller found, by any means.
//!
//! A message costs one request per header block plus one for its body, so
//! the hand-off is paid per entity rather than per field. Every answer is
//! checked before use; a position failing its check is dropped and
//! reported, and whatever the answers the tree still round-trips, its
//! leaves partitioning the input.
//!
//! The walk is iterative over an explicit stack, so no input recurses on
//! the call stack while it is read.
//!
//! # Example
//!
//! ```rust
//! use mime::tree::{
//!     coroutine::MimeCoroutineState,
//!     parse::{MimeParse, MimeParseOptions, MimeParseYield},
//!     scan::memchr,
//! };
//!
//! let input = b"Subject: hi\r\n\r\nbody";
//! let mut parse = MimeParse::new(input, MimeParseOptions::default());
//! let mut found = Vec::new();
//! let mut arg = None;
//!
//! let cst = loop {
//!     match parse.resume(arg.take()) {
//!         MimeCoroutineState::Yielded(MimeParseYield::WantsFieldEnds { range }) => {
//!             found.clear();
//!             memchr::field_ends(input, range, &mut found);
//!             arg = Some(found.as_slice());
//!         }
//!         MimeCoroutineState::Yielded(MimeParseYield::WantsDashLines { range }) => {
//!             found.clear();
//!             memchr::dash_lines(input, range, &mut found);
//!             arg = Some(found.as_slice());
//!         }
//!         MimeCoroutineState::Yielded(MimeParseYield::WantsFind { range, needle }) => {
//!             found.clear();
//!             memchr::find(input, range, &needle, &mut found);
//!             arg = Some(found.as_slice());
//!         }
//!         MimeCoroutineState::Complete(cst) => break cst,
//!     }
//! };
//!
//! assert_eq!(cst.to_bytes(), input);
//! ```

use alloc::{borrow::Cow, boxed::Box, vec, vec::Vec};
use core::{mem, ops::Range};

use crate::tree::{
    anomaly::{MimeAnomalies, MimeAnomaly},
    body::{MimeBodyNode, MimeMultipart, MimePart},
    coroutine::MimeCoroutineState,
    cst::MimeCst,
    entity::MimeEntity,
    field::content_type::{MimeMediaType, raw_param},
    header::{MimeHeaderBlock, MimeHeaderField, MimeHeaderItem},
    scan::scalar::is_empty_line,
};
use crate::value::transfer_encoding::MimeTransferEncoding;

/// The default nesting bound, the root counting as one.
pub const DEPTH_MAX: usize = 64;

/// The default entity bound.
pub const ENTITIES_MAX: usize = 10_000;

/// Bounds a parse keeps to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeParseOptions {
    /// The most containers (multiparts and messages) nesting on one path,
    /// the root included. Defaults to [`DEPTH_MAX`].
    pub max_depth: Option<usize>,
    /// The most entities a parse holds. Defaults to [`ENTITIES_MAX`].
    pub max_entities: Option<usize>,
}

/// What the parse coroutine needs found before it can progress. Each
/// request is answered by resuming with the positions found, in
/// increasing order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeParseYield {
    /// Every field end of the header block starting the range: each `\n`
    /// whose next byte is neither SP nor HTAB, or that ends the range,
    /// stopping after the first one followed by an empty line.
    WantsFieldEnds {
        /// The entity the header block starts.
        range: Range<usize>,
    },
    /// Every line start in the range beginning with `--`, the range start
    /// counting as a line start.
    WantsDashLines {
        /// The body of the outermost multipart.
        range: Range<usize>,
    },
    /// Every non-overlapping occurrence of `needle` in the range, leftmost
    /// first, for a multipart with no delimiter at the start of a line.
    WantsFind {
        /// The multipart body.
        range: Range<usize>,
        /// The boundary.
        needle: Vec<u8>,
    },
}

/// The I/O-free parse coroutine.
#[derive(Debug)]
pub struct MimeParse<'a> {
    input: &'a [u8],
    max_depth: usize,
    max_entities: usize,
    tasks: Vec<Task>,
    flat: Vec<Flat<'a>>,
    dashes: Option<Vec<usize>>,
    state: State,
}

impl<'a> MimeParse<'a> {
    /// Builds a coroutine parsing `input`.
    pub fn new(input: &'a [u8], opts: MimeParseOptions) -> Self {
        let root = Task {
            range: 0..input.len(),
            depth: 1,
            digest: false,
        };

        Self {
            input,
            max_depth: opts.max_depth.unwrap_or(DEPTH_MAX).max(1),
            max_entities: opts.max_entities.unwrap_or(ENTITIES_MAX).max(1),
            tasks: vec![root],
            flat: Vec::new(),
            dashes: None,
            state: State::Next,
        }
    }

    /// Progresses the parse. Pass `None` first, then the positions found
    /// for the request last yielded.
    pub fn resume(
        &mut self,
        arg: Option<&[usize]>,
    ) -> MimeCoroutineState<MimeParseYield, MimeCst<'a>> {
        let mut arg = arg;

        loop {
            match mem::replace(&mut self.state, State::Done) {
                State::Next => {
                    let Some(task) = self.tasks.pop() else {
                        return MimeCoroutineState::Complete(self.assemble());
                    };

                    if let Some(request) = self.start(task) {
                        return MimeCoroutineState::Yielded(request);
                    }
                }
                State::FindFieldEnds(task) => {
                    let range = task.range.clone();
                    let input = self.input;
                    let (ends, dropped) = checked(arg.take(), &range, |i| {
                        input[i] == b'\n'
                            && !matches!(
                                input.get(i + 1).filter(|_| i + 1 < range.end),
                                Some(b' ' | b'\t')
                            )
                    });

                    if let Some(request) = self.entity(task, &ends, dropped) {
                        return MimeCoroutineState::Yielded(request);
                    }
                }
                State::FindDashLines(split) => {
                    let range = split.body.clone();
                    let input = self.input;
                    let (dashes, dropped) = checked(arg.take(), &range, |i| {
                        input[i..].starts_with(b"--")
                            && i + 2 <= range.end
                            && (i == range.start || input[i - 1] == b'\n')
                    });

                    if dropped {
                        self.flat[split.entity]
                            .anomalies
                            .insert(MimeAnomaly::DroppedAnswer);
                    }

                    self.dashes = Some(dashes.into_owned());

                    if let Some(request) = self.split(split, None) {
                        return MimeCoroutineState::Yielded(request);
                    }
                }
                State::FindNeedle(split) => {
                    let range = split.body.clone();
                    let input = self.input;
                    let needle = split.boundary.clone();
                    let (found, dropped) = checked(arg.take(), &range, |i| {
                        i + needle.len() <= range.end && input[i..].starts_with(&needle)
                    });

                    if dropped {
                        self.flat[split.entity]
                            .anomalies
                            .insert(MimeAnomaly::DroppedAnswer);
                    }

                    self.split(split, Some(&found));
                }
                State::Done => {
                    self.state = State::Done;
                    return MimeCoroutineState::Complete(MimeCst::default());
                }
            }

            if matches!(self.state, State::Done) {
                self.state = State::Next;
            }
        }
    }

    /// Starts an entity: settles a header block that needs no search, or
    /// asks for its field ends.
    fn start(&mut self, task: Task) -> Option<MimeParseYield> {
        let input = self.input;
        let range = task.range.clone();
        let first = &input[range.clone()];

        // NOTE: only a line starting with a blank or a line ending can be
        // empty, so only such a line is scanned.
        let maybe_blank = matches!(first.first(), Some(b' ' | b'\t' | b'\r' | b'\n'));
        let line_end = if maybe_blank {
            first.iter().position(|byte| *byte == b'\n')
        } else {
            None
        };
        let first_line = &first[..line_end.map_or(first.len(), |end| end + 1)];
        let blank_line = maybe_blank
            && first_line
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'));

        if first.is_empty() || (blank_line && line_end.is_some()) {
            let header = MimeHeaderBlock {
                items: Vec::new(),
                end: Cow::Borrowed(first_line),
            };

            return self.body(
                task,
                header,
                range.start + first_line.len(),
                MimeAnomalies::default(),
            );
        }

        self.state = State::FindFieldEnds(task);
        Some(MimeParseYield::WantsFieldEnds { range })
    }

    /// Builds the header block of an entity from its field ends, then its
    /// body.
    fn entity(&mut self, task: Task, ends: &[usize], dropped: bool) -> Option<MimeParseYield> {
        let input = self.input;
        let end = task.range.end;
        let mut anomalies = MimeAnomalies::default();
        let mut items = Vec::with_capacity(ends.len() + 1);
        let mut block_end: &[u8] = &[];
        let mut pos = task.range.start;
        let mut terminated = false;

        if dropped {
            anomalies.insert(MimeAnomaly::DroppedAnswer);
        }

        for &at in ends {
            push_items(&input[pos..=at], &mut items, &mut anomalies);
            pos = at + 1;

            if is_empty_line(input, pos, end) {
                let len = if input[pos] == b'\n' { 1 } else { 2 };
                block_end = &input[pos..pos + len];
                pos += len;
                terminated = true;
                break;
            }
        }

        if !terminated && pos < end {
            push_items(&input[pos..end], &mut items, &mut anomalies);
            pos = end;
        }

        // NOTE: a block holding no field is no header block at all: the
        // entity is all body, as the RFC 5322 3.5 body of a message with no
        // field would be.
        if !items
            .iter()
            .any(|item| matches!(item, MimeHeaderItem::Field(_)))
        {
            let dropped = anomalies.contains(MimeAnomaly::DroppedAnswer);
            anomalies = MimeAnomalies::default();

            if dropped {
                anomalies.insert(MimeAnomaly::DroppedAnswer);
            }
            return self.body(
                task.clone(),
                MimeHeaderBlock::default(),
                task.range.start,
                anomalies,
            );
        }

        if !terminated {
            anomalies.insert(MimeAnomaly::NoBlankLine);
        }

        let header = MimeHeaderBlock {
            items,
            end: Cow::Borrowed(block_end),
        };

        self.body(task, header, pos, anomalies)
    }

    /// Records an entity and decides its body: a multipart to split, a
    /// nested message to parse, or a leaf.
    fn body(
        &mut self,
        task: Task,
        header: MimeHeaderBlock<'a>,
        start: usize,
        mut anomalies: MimeAnomalies,
    ) -> Option<MimeParseYield> {
        let input = self.input;
        let body = start..task.range.end;
        let kind = Kind::of(&header, task.digest);
        let index = self.flat.len();

        let container = matches!(kind, Kind::Multipart { .. } | Kind::Message) && !body.is_empty();
        let too_deep = container && task.depth > self.max_depth;

        if too_deep {
            anomalies.insert(MimeAnomaly::NestingLimit);
        }

        self.flat.push(Flat {
            header,
            body: FlatBody::Leaf(&input[body.clone()]),
            anomalies,
        });

        if too_deep {
            return None;
        }

        match kind {
            Kind::Multipart { boundary, digest } if !body.is_empty() => {
                let split = Split {
                    entity: index,
                    body: body.clone(),
                    boundary,
                    depth: task.depth,
                    digest,
                };

                // NOTE: a boundary of dashes alone would match every dashed
                // line of the body, so it never matches.
                if split.boundary.iter().all(|byte| *byte == b'-') {
                    let entity = &mut self.flat[index];
                    entity.anomalies.insert(MimeAnomaly::MissingDelimiter);
                    entity.body = FlatBody::Multipart {
                        preamble: &input[body],
                        delimiters: Vec::new(),
                        close: &[],
                        epilogue: &[],
                    };
                    return None;
                }

                if self.dashes.is_some() {
                    return self.split(split, None);
                }

                self.state = State::FindDashLines(split);
                Some(MimeParseYield::WantsDashLines { range: body })
            }
            Kind::Message if starts_with_field(&input[body.clone()]) => {
                self.flat[index].body = FlatBody::Message;
                self.tasks.push(Task {
                    range: body,
                    depth: task.depth + 1,
                    digest: false,
                });
                None
            }
            Kind::Multipart { .. } | Kind::Message | Kind::Leaf => None,
        }
    }

    /// Splits a multipart at its delimiter lines, from the dash lines
    /// already found or, for the fallback, from the boundary's mid-line
    /// occurrences.
    fn split(&mut self, split: Split, fallback: Option<&[usize]>) -> Option<MimeParseYield> {
        let input = self.input;
        let body = split.body.clone();
        let boundary = split.boundary.as_slice();

        let dashes = self.dashes.as_deref().unwrap_or_default();
        let from = dashes.partition_point(|&at| at < body.start);
        let to = dashes.partition_point(|&at| at < body.end);
        let mut delimiters = line_delimiters(input, &body, boundary, &dashes[from..to]);

        let needs_open = delimiters.first().is_none_or(|delimiter| delimiter.close);
        let needs_close = !delimiters.last().is_some_and(|delimiter| delimiter.close);

        let Some(found) = fallback else {
            if needs_open || needs_close {
                let needle = boundary.to_vec();
                self.state = State::FindNeedle(split);
                return Some(MimeParseYield::WantsFind {
                    range: body,
                    needle,
                });
            }

            return self.build(split, delimiters, false);
        };

        // NOTE: a `--` and boundary mid-line or followed by more text, as
        // broken mailers write them, which the strict line-start rule skips.
        let mid: Vec<usize> = found
            .iter()
            .filter(|&&at| at >= body.start + 2 && &input[at - 2..at] == b"--")
            .map(|&at| at - 2)
            .collect();

        let mut fell_back = false;

        if needs_open {
            let limit = delimiters
                .first()
                .map_or(body.end, |delimiter| delimiter.start);
            let opener = mid
                .iter()
                .filter(|&&line| line < limit)
                .find_map(|&line| Delimiter::at(input, &body, body.start, line, boundary, false));

            if let Some(opener) = opener {
                fell_back = true;
                delimiters.retain(|delimiter| delimiter.start >= opener.end);

                if opener.close {
                    delimiters.clear();
                }

                delimiters.insert(0, opener);
            }
        }

        let needs_close = !delimiters.last().is_some_and(|delimiter| delimiter.close);

        if needs_close && let Some(last) = delimiters.last() {
            let after = last.end;
            let closer = mid
                .iter()
                .filter(|&&line| line >= after)
                .filter_map(|&line| Delimiter::at(input, &body, after, line, boundary, false))
                .find(|delimiter| delimiter.close);

            if let Some(closer) = closer {
                fell_back = true;
                delimiters.push(closer);
            }
        }

        self.build(split, delimiters, fell_back)
    }

    /// Records the multipart its delimiters split, and queues its parts.
    fn build(
        &mut self,
        split: Split,
        delimiters: Vec<Delimiter>,
        fell_back: bool,
    ) -> Option<MimeParseYield> {
        let input = self.input;
        let body = split.body.clone();

        if delimiters.is_empty() {
            self.flat[split.entity]
                .anomalies
                .insert(MimeAnomaly::MissingDelimiter);
            return None;
        }

        let opens = delimiters
            .iter()
            .filter(|delimiter| !delimiter.close)
            .count();

        if self.flat.len() + self.tasks.len() + opens > self.max_entities {
            self.flat[split.entity]
                .anomalies
                .insert(MimeAnomaly::PartLimit);
            return None;
        }

        let entity = &mut self.flat[split.entity];

        if fell_back {
            entity.anomalies.insert(MimeAnomaly::FallbackDelimiter);
        }

        let closed = delimiters.last().is_some_and(|delimiter| delimiter.close);

        if !closed {
            entity.anomalies.insert(MimeAnomaly::Unterminated);
        }

        let first = delimiters[0].start;
        let mut parts = Vec::with_capacity(opens);
        let mut close: &[u8] = &[];
        let mut epilogue: &[u8] = &[];

        for (i, delimiter) in delimiters.iter().enumerate() {
            if delimiter.close {
                close = &input[delimiter.start..delimiter.end];
                epilogue = &input[delimiter.end..body.end];
                break;
            }

            let content_end = delimiters.get(i + 1).map_or(body.end, |next| next.start);
            parts.push((delimiter.start..delimiter.end, delimiter.end..content_end));
        }

        entity.body = FlatBody::Multipart {
            preamble: &input[body.start..first],
            delimiters: parts.iter().map(|(line, _)| &input[line.clone()]).collect(),
            close,
            epilogue,
        };

        for (_, content) in parts.into_iter().rev() {
            self.tasks.push(Task {
                range: content,
                depth: split.depth + 1,
                digest: split.digest,
            });
        }

        None
    }

    /// Folds the flat entities, last first, into the nested tree.
    fn assemble(&mut self) -> MimeCst<'a> {
        // NOTE: walked last first, every entity finds its children on top of
        // the stack, the first child topmost, its descendants already folded.
        let flat = mem::take(&mut self.flat);
        let mut stack: Vec<MimeEntity<'a>> = Vec::new();

        for entity in flat.into_iter().rev() {
            let count = match &entity.body {
                FlatBody::Leaf(_) => 0,
                FlatBody::Multipart { delimiters, .. } => delimiters.len(),
                FlatBody::Message => 1,
            };

            let mut kids: Vec<MimeEntity<'a>> = Vec::with_capacity(count);

            for _ in 0..count {
                kids.push(stack.pop().unwrap_or_default());
            }

            let body = match entity.body {
                FlatBody::Leaf(bytes) => MimeBodyNode::Leaf(Cow::Borrowed(bytes)),
                FlatBody::Multipart {
                    preamble,
                    delimiters,
                    close,
                    epilogue,
                } => MimeBodyNode::Multipart(Box::new(MimeMultipart {
                    preamble: Cow::Borrowed(preamble),
                    parts: delimiters
                        .into_iter()
                        .zip(kids)
                        .map(|(delimiter, entity)| MimePart {
                            delimiter: Cow::Borrowed(delimiter),
                            entity,
                        })
                        .collect(),
                    close: Cow::Borrowed(close),
                    epilogue: Cow::Borrowed(epilogue),
                })),
                FlatBody::Message => {
                    MimeBodyNode::Message(Box::new(kids.pop().unwrap_or_default()))
                }
            };

            let built = MimeEntity {
                header: entity.header,
                body,
                anomalies: entity.anomalies,
            };

            stack.push(built);
        }

        MimeCst {
            root: stack.pop().unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
enum State {
    Next,
    FindFieldEnds(Task),
    FindDashLines(Split),
    FindNeedle(Split),
    Done,
}

#[derive(Clone, Debug)]
struct Task {
    range: Range<usize>,
    depth: usize,
    digest: bool,
}

#[derive(Debug)]
struct Split {
    entity: usize,
    body: Range<usize>,
    boundary: Vec<u8>,
    depth: usize,
    digest: bool,
}

#[derive(Debug)]
struct Flat<'a> {
    header: MimeHeaderBlock<'a>,
    body: FlatBody<'a>,
    anomalies: MimeAnomalies,
}

#[derive(Debug)]
enum FlatBody<'a> {
    Leaf(&'a [u8]),
    Multipart {
        preamble: &'a [u8],
        delimiters: Vec<&'a [u8]>,
        close: &'a [u8],
        epilogue: &'a [u8],
    },
    Message,
}

/// What the structural fields make of a body.
enum Kind {
    Multipart { boundary: Vec<u8>, digest: bool },
    Message,
    Leaf,
}

impl Kind {
    fn of(header: &MimeHeaderBlock<'_>, digest: bool) -> Self {
        let field = header.get_all("content-type").last();
        let (top, sub) = field.map_or((&[][..], &[][..]), |field| {
            let media = MimeMediaType::new(field.value());
            (media.top, media.sub)
        });

        if top.is_empty() {
            return if digest { Self::Message } else { Self::Leaf };
        }

        let message = top.eq_ignore_ascii_case(b"message")
            && (sub.eq_ignore_ascii_case(b"rfc822") || sub.eq_ignore_ascii_case(b"global"));

        if !top.eq_ignore_ascii_case(b"multipart") {
            return if message && identity_encoding(header) {
                Self::Message
            } else {
                Self::Leaf
            };
        }

        let value = field.map(|field| field.value()).unwrap_or_default();

        {
            match raw_param(value, "boundary").filter(|b| !b.is_empty()) {
                Some(boundary) => Self::Multipart {
                    boundary,
                    digest: sub.eq_ignore_ascii_case(b"digest"),
                },
                None => Self::Leaf,
            }
        }
    }
}

/// Whether the entity's transfer encoding leaves its body as written:
/// none, `7bit`, `8bit`, `binary`, or one this crate does not know.
pub(crate) fn identity_encoding(header: &MimeHeaderBlock<'_>) -> bool {
    let Some(field) = header.get_all("content-transfer-encoding").last() else {
        return true;
    };

    !matches!(
        crate::tree::field::value::transfer_encoding(field.value()),
        MimeTransferEncoding::Base64 | MimeTransferEncoding::QuotedPrintable
    )
}

/// A delimiter line, from the line ending before it to the end of its own
/// line.
struct Delimiter {
    start: usize,
    end: usize,
    close: bool,
}

impl Delimiter {
    fn at(
        input: &[u8],
        body: &Range<usize>,
        floor: usize,
        line: usize,
        boundary: &[u8],
        line_start: bool,
    ) -> Option<Self> {
        let after = line + 2 + boundary.len();

        if after > body.end || input.get(line + 2..after) != Some(boundary) {
            return None;
        }

        let rest = &input[after..body.end];
        let close = rest.starts_with(b"--");

        if line_start
            && !close
            && !matches!(rest.first(), None | Some(b'\r' | b'\n' | b' ' | b'\t'))
        {
            return None;
        }

        // NOTE: the line ending before the delimiter is its own, unless the
        // previous delimiter line already ended on it.
        let start = if line == floor || input[line - 1] != b'\n' {
            line
        } else if line >= floor + 2 && &input[line - 2..line] == b"\r\n" {
            line - 2
        } else {
            line - 1
        };

        let end = rest
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(body.end, |at| after + at + 1);

        Some(Self { start, end, close })
    }
}

/// The delimiter lines among the dash lines of a body, in order, the scan
/// stopping after the first close delimiter.
fn line_delimiters(
    input: &[u8],
    body: &Range<usize>,
    boundary: &[u8],
    candidates: &[usize],
) -> Vec<Delimiter> {
    let mut delimiters: Vec<Delimiter> = Vec::new();
    let mut floor = body.start;

    for &line in candidates {
        if line < floor {
            continue;
        }

        let Some(delimiter) = Delimiter::at(input, body, floor, line, boundary, true) else {
            continue;
        };

        floor = delimiter.end;
        let close = delimiter.close;
        delimiters.push(delimiter);

        if close {
            break;
        }
    }

    delimiters
}

/// Whether the first line of `bytes` can open a header block: it holds a
/// colon. A nested message whose content opens with no field is not one.
fn starts_with_field(bytes: &[u8]) -> bool {
    let line = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    !matches!(line.first(), None | Some(b' ' | b'\t')) && line.contains(&b':')
}

/// Pushes the header block items one logical line makes, its line ending
/// included: a field, or an opaque line, or an opaque line then the field
/// folded after it, as in `garbage\n Subject: x`.
fn push_items<'a>(
    bytes: &'a [u8],
    items: &mut Vec<MimeHeaderItem<'a>>,
    anomalies: &mut MimeAnomalies,
) {
    let (content, _) = if let Some(content) = bytes.strip_suffix(b"\r\n") {
        (content, &bytes[content.len()..])
    } else if let Some(content) = bytes.strip_suffix(b"\n") {
        (content, &bytes[content.len()..])
    } else {
        (bytes, &bytes[bytes.len()..])
    };

    // NOTE: the common field, a name then a colon on its first line, is
    // read with one search.
    if !matches!(content.first(), None | Some(b' ' | b'\t' | b':'))
        && let Some(colon) =
            ::memchr::memchr2(b':', b'\n', content).filter(|&at| content[at] == b':')
    {
        let name_end = content[..colon]
            .iter()
            .rposition(|byte| !matches!(byte, b' ' | b'\t'))
            .map_or(0, |at| at + 1);

        push_field(bytes, name_end, colon + 1, content.len(), items);
        return;
    }

    // NOTE: the field starts on the first physical line holding a colon,
    // after its folding blanks; a first line starting with a blank has no
    // field to continue.
    let mut start = None;
    let mut line_start = 0;

    for line in content.split_inclusive(|byte| *byte == b'\n') {
        let lead = line
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let can_start = line_start > 0 || lead == 0;

        if can_start && has_colon(&line[lead..]) {
            start = Some(line_start + lead);
            break;
        }

        line_start += line.len();
    }

    let Some(start) = start else {
        anomalies.insert(MimeAnomaly::OpaqueLine);
        items.push(MimeHeaderItem::Opaque(Cow::Borrowed(bytes)));
        return;
    };

    if start > 0 {
        anomalies.insert(MimeAnomaly::OpaqueLine);
        items.push(MimeHeaderItem::Opaque(Cow::Borrowed(&content[..start])));
    }

    let field = &content[start..];
    let first_line = &field[..field
        .iter()
        .position(|byte| *byte == b'\n')
        .unwrap_or(field.len())];
    let lead = first_line.iter().take_while(|byte| **byte == b':').count();
    let colon = lead
        + first_line[lead..]
            .iter()
            .position(|byte| *byte == b':')
            .unwrap_or_default();

    let name_end = field[..colon]
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(0, |at| at + 1);

    push_field(&bytes[start..], name_end, colon + 1, field.len(), items);
}

/// Whether a physical line holds a colon past its leading colons.
fn has_colon(line: &[u8]) -> bool {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let lead = line.iter().take_while(|byte| **byte == b':').count();
    line[lead..].contains(&b':')
}

/// Keeps the positions of an answer that lie in the range, increase and
/// pass `valid`, reporting whether any was dropped.
fn checked<'r>(
    arg: Option<&'r [usize]>,
    range: &Range<usize>,
    valid: impl Fn(usize) -> bool,
) -> (Cow<'r, [usize]>, bool) {
    let answer = arg.unwrap_or_default();

    // NOTE: an answer that passes whole is used as it came.
    let mut last = None;
    let whole = answer.iter().all(|&at| {
        let in_order = last.is_none_or(|last| at > last);
        last = Some(at);
        range.contains(&at) && in_order && valid(at)
    });

    if whole {
        return (Cow::Borrowed(answer), false);
    }

    let mut kept = Vec::with_capacity(answer.len());
    let mut dropped = false;

    for &at in answer {
        let in_order = kept.last().is_none_or(|last| at > *last);

        if range.contains(&at) && in_order && valid(at) {
            kept.push(at);
        } else {
            dropped = true;
        }
    }

    (Cow::Owned(kept), dropped)
}

/// Pushes a field cut from its whole line, its line ending included; a
/// line too long for the offsets stays opaque.
fn push_field<'a>(
    line: &'a [u8],
    name_end: usize,
    colon_end: usize,
    value_end: usize,
    items: &mut Vec<MimeHeaderItem<'a>>,
) {
    match MimeHeaderField::new(Cow::Borrowed(line), name_end, colon_end, value_end) {
        Some(field) => items.push(MimeHeaderItem::Field(field)),
        None => items.push(MimeHeaderItem::Opaque(Cow::Borrowed(line))),
    }
}
