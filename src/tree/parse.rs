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

use alloc::{borrow::Cow, boxed::Box, vec::Vec};
use core::{mem, ops::Range};

use crate::{
    tree::{
        anomaly::{MimeAnomalies, MimeAnomaly},
        body::{MimeBodyNode, MimeMultipart, MimePart, MimeUnparsed},
        coroutine::MimeCoroutineState,
        cst::MimeCst,
        entity::MimeEntity,
        field::{
            content_type::{media, raw_params},
            lex::{Lexer, eq_ignore_case},
            value,
        },
        header::{MimeHeaderBlock, MimeHeaderField, MimeHeaderItem},
        scan::scalar::empty_line,
    },
    value::transfer_encoding::MimeTransferEncoding,
};

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
    /// How many levels of containers (multiparts and messages) to split,
    /// `None` for all: at `Some(0)` the parse stops after the root header
    /// block and keeps its body unparsed, so reading headers costs only the
    /// header block. A body left unparsed is split later by
    /// [`MimeEntity::expand`](crate::tree::entity::MimeEntity::expand).
    pub structure_depth: Option<usize>,
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
///
/// The tree is built in place: every entity is written straight into its
/// slot rather than moved through the parse. Entities run depth first and
/// a part joins its multipart when it starts, so the entity being parsed is
/// reached from the root through the last part of each multipart on the
/// way.
// NOTE: the entity and the multipart in progress are fields read one by
// one, and the state carries no data: a value moved whole through the state
// was copied with wide loads over the narrow stores that had just written
// it, padding included, and each such load waited on its stores (store
// forwarding failing).
#[derive(Debug)]
pub struct MimeParse<'a> {
    input: &'a [u8],
    max_depth: usize,
    max_entities: usize,
    structure_depth: usize,
    root: MimeEntity<'a>,
    tasks: Vec<Task>,
    entities: usize,
    dashes: Option<Vec<usize>>,
    delimiters: Vec<Delimiter>,
    task: Task,
    split: Split<'a>,
    state: State,
}

impl<'a> MimeParse<'a> {
    /// Builds a coroutine parsing `input`.
    pub fn new(input: &'a [u8], opts: MimeParseOptions) -> Self {
        Self {
            input,
            max_depth: opts.max_depth.unwrap_or(DEPTH_MAX).max(1),
            max_entities: opts.max_entities.unwrap_or(ENTITIES_MAX).max(1),
            structure_depth: opts.structure_depth.unwrap_or(usize::MAX),
            root: MimeEntity::default(),
            tasks: Vec::new(),
            entities: 0,
            dashes: None,
            delimiters: Vec::new(),
            task: Task {
                range: 0..input.len(),
                delimiter: 0,
                depth: 1,
                digest: false,
            },
            split: Split::default(),
            state: State::Start,
        }
    }

    /// Builds a coroutine splitting the body `input` of an entity whose
    /// header block is `header`, as the parse would have split it: the body
    /// a parse left [`Unparsed`](MimeBodyNode::Unparsed). The structure
    /// depth counts from that entity, so `Some(0)` splits it alone.
    ///
    /// The tree it completes with is that entity, under a copy of `header`.
    pub fn unparsed(
        header: &MimeHeaderBlock<'a>,
        input: &'a [u8],
        digest: bool,
        opts: MimeParseOptions,
    ) -> Self {
        let mut parse = Self::new(input, opts);
        parse.structure_depth = parse.structure_depth.saturating_add(1);
        parse.root.header = header.clone();
        parse.task.digest = digest;
        parse.state = State::Body;
        parse
    }

    /// Progresses the parse. Pass `None` first, then the positions found
    /// for the request last yielded.
    pub fn resume(
        &mut self,
        arg: Option<&[usize]>,
    ) -> MimeCoroutineState<MimeParseYield, MimeCst<'a>> {
        let mut arg = arg;

        // NOTE: a step that neither yields nor enters a task leaves the state
        // on the next task.
        loop {
            let request = match mem::replace(&mut self.state, State::Next) {
                State::Start => self.start(),
                State::Body => self.body(0, MimeAnomalies::default()),
                State::FieldEnds => self.entity(arg.take().unwrap_or_default()),
                State::DashLines => self.dash_lines(arg.take().unwrap_or_default()),
                State::Find => self.find(arg.take().unwrap_or_default()),
                State::Next => {
                    let Some(task) = self.tasks.pop() else {
                        self.state = State::Done;
                        let root = mem::take(&mut self.root);
                        return MimeCoroutineState::Complete(MimeCst { root });
                    };

                    self.enter(task);
                    None
                }
                State::Done => {
                    self.state = State::Done;
                    return MimeCoroutineState::Complete(MimeCst::default());
                }
            };

            if let Some(request) = request {
                return MimeCoroutineState::Yielded(request);
            }
        }
    }

    /// Makes `task` the next to run, a part joining its multipart.
    // NOTE: the first child is entered at once rather than pushed then
    // popped, whose pop read it back whole while its stores were pending.
    #[inline(always)]
    fn enter(&mut self, task: Task) {
        let parent = current(&mut self.root, task.depth - 1);

        if let MimeBodyNode::Multipart(multipart) = &mut parent.body {
            let delimiter = &self.input[task.delimiter..task.range.start];

            // NOTE: written in place rather than staged and copied, for the
            // reason the parse steps are inlined.
            multipart
                .parts
                .resize_with(multipart.parts.len() + 1, || MimePart {
                    delimiter: Cow::Borrowed(delimiter),
                    entity: MimeEntity::default(),
                });
        }

        self.task = task;
        self.state = State::Start;
    }

    /// Starts the entity: settles a header block that needs no search, or
    /// asks for its field ends.
    // NOTE: this step and the ones it leads to are inlined into `resume`:
    // a request returned through memory was written by narrow stores then
    // read back whole, which stalled the load (store forwarding failing)
    // and cost more than the calls.
    #[inline(always)]
    fn start(&mut self) -> Option<MimeParseYield> {
        let input = self.input;
        let range = self.task.range.clone();
        let first = &input[range.clone()];

        // NOTE: only a line starting with a blank or a line ending can be
        // empty, so only such a line is scanned.
        let maybe_blank = matches!(first.first(), Some(b' ' | b'\t' | b'\r' | b'\x0c' | b'\n'));
        let line_end = if maybe_blank {
            first.iter().position(|byte| *byte == b'\n')
        } else {
            None
        };
        let first_line = &first[..line_end.map_or(first.len(), |end| end + 1)];
        let blank_line = maybe_blank
            && first_line
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\x0c' | b'\n'));

        if first.is_empty() || (blank_line && line_end.is_some()) {
            current(&mut self.root, self.task.depth).header.end = Cow::Borrowed(first_line);
            return self.body(range.start + first_line.len(), MimeAnomalies::default());
        }

        self.state = State::FieldEnds;
        Some(MimeParseYield::WantsFieldEnds { range })
    }

    /// Builds the header block of the entity from its field ends, then its
    /// body.
    #[inline(always)]
    fn entity(&mut self, answer: &[usize]) -> Option<MimeParseYield> {
        let input = self.input;
        let Range { start, end } = self.task.range;
        let mut anomalies = MimeAnomalies::default();
        let header = &mut current(&mut self.root, self.task.depth).header;
        let mut pos = start;
        let mut next = start;
        let mut terminated = false;

        header.items.reserve_exact(answer.len() + 1);

        // NOTE: every position is checked, those past the empty line too:
        // in the block, increasing, on a line feed starting no continuation.
        for &at in answer {
            let valid = at >= next
                && at < end
                && input[at] == b'\n'
                && !(at + 1 < end && matches!(input[at + 1], b' ' | b'\t'));

            if !valid {
                anomalies.insert(MimeAnomaly::DroppedAnswer);
                continue;
            }

            next = at + 1;

            if terminated {
                continue;
            }

            push_items(&input[pos..=at], &mut header.items, &mut anomalies);
            pos = at + 1;

            if let Some(line_end) = empty_line(input, pos, end) {
                header.end = Cow::Borrowed(&input[pos..line_end]);
                pos = line_end;
                terminated = true;
            }
        }

        if !terminated && pos < end {
            push_items(&input[pos..end], &mut header.items, &mut anomalies);
            pos = end;
        }

        // NOTE: a block holding no field is no header block at all: the
        // entity is all body, as the RFC 5322 3.5 body of a message with no
        // field would be.
        if !header
            .items
            .iter()
            .any(|item| matches!(item, MimeHeaderItem::Field(_)))
        {
            *header = MimeHeaderBlock::default();
            let dropped = anomalies.contains(MimeAnomaly::DroppedAnswer);
            anomalies = MimeAnomalies::default();

            if dropped {
                anomalies.insert(MimeAnomaly::DroppedAnswer);
            }

            return self.body(start, anomalies);
        }

        if !terminated {
            anomalies.insert(MimeAnomaly::NoBlankLine);
        }

        self.body(pos, anomalies)
    }

    /// Records the entity and decides its body: a multipart to split, a
    /// nested message to parse, or a leaf.
    #[inline(always)]
    fn body(&mut self, start: usize, mut anomalies: MimeAnomalies) -> Option<MimeParseYield> {
        let input = self.input;
        let body = start..self.task.range.end;
        let depth = self.task.depth;
        self.entities += 1;

        let entity = current(&mut self.root, self.task.depth);
        let kind = Kind::of(&entity.header, self.task.digest);

        let container = matches!(kind, Kind::Multipart { .. } | Kind::Message) && !body.is_empty();
        let too_deep = container && depth > self.max_depth;

        if too_deep {
            anomalies.insert(MimeAnomaly::NestingLimit);
        }

        entity.anomalies = anomalies;

        // NOTE: past the structure depth, a container keeps its body whole
        // until it is expanded.
        if container && !too_deep && depth > self.structure_depth {
            entity.body = MimeBodyNode::Unparsed(Box::new(MimeUnparsed {
                bytes: Cow::Borrowed(&input[body]),
                digest: self.task.digest,
            }));
            return None;
        }

        entity.body = MimeBodyNode::Leaf(Cow::Borrowed(&input[body.clone()]));

        if too_deep {
            return None;
        }

        match kind {
            Kind::Multipart { boundary, digest } if !body.is_empty() => {
                // NOTE: a boundary of dashes alone would match every dashed
                // line of the body, so it never matches.
                if boundary.iter().all(|byte| *byte == b'-') {
                    entity.anomalies.insert(MimeAnomaly::MissingDelimiter);
                    entity.body = MimeBodyNode::Multipart(Box::new(MimeMultipart {
                        preamble: Cow::Borrowed(&input[body]),
                        ..Default::default()
                    }));
                    return None;
                }

                self.split = Split {
                    body: body.clone(),
                    boundary,
                    depth,
                    digest,
                };

                if self.dashes.is_some() {
                    return self.split(None);
                }

                self.state = State::DashLines;
                Some(MimeParseYield::WantsDashLines { range: body })
            }
            Kind::Message if starts_with_field(&input[body.clone()]) => {
                entity.body = MimeBodyNode::Message(Box::default());
                self.enter(Task {
                    delimiter: body.start,
                    range: body,
                    depth: depth + 1,
                    digest: false,
                });
                None
            }
            Kind::Multipart { .. } | Kind::Message | Kind::Leaf => None,
        }
    }

    /// Keeps the dash lines of the outermost multipart body, each checked
    /// (in the body, increasing, `--` at the start of a line), then splits
    /// it.
    #[inline(always)]
    fn dash_lines(&mut self, answer: &[usize]) -> Option<MimeParseYield> {
        let input = self.input;
        let body = self.split.body.clone();
        let dashes = self.dashes.insert(Vec::with_capacity(answer.len()));
        let mut next = body.start;

        for &at in answer {
            if at >= next
                && at < body.end
                && input[at..body.end].starts_with(b"--")
                && (at == body.start || input[at - 1] == b'\n')
            {
                dashes.push(at);
                next = at + 1;
            }
        }

        if dashes.len() < answer.len() {
            let entity = current(&mut self.root, self.task.depth);
            entity.anomalies.insert(MimeAnomaly::DroppedAnswer);
        }

        self.split(None)
    }

    /// Splits the multipart again with the boundary's occurrences, each
    /// checked (in the body, increasing, on the boundary), for its
    /// delimiters found mid-line.
    #[inline(always)]
    fn find(&mut self, answer: &[usize]) -> Option<MimeParseYield> {
        let input = self.input;
        let body = self.split.body.clone();
        let needle = &*self.split.boundary;
        let mut mid = Vec::new();
        let mut next = body.start;
        let mut dropped = false;

        for &at in answer {
            if !(at >= next && at < body.end && input[at..body.end].starts_with(needle)) {
                dropped = true;
                continue;
            }

            next = at + 1;

            // NOTE: a `--` and boundary mid-line or followed by more text,
            // as broken mailers write them, which the strict line-start rule
            // skips.
            if at >= body.start + 2 && &input[at - 2..at] == b"--" {
                mid.push(at - 2);
            }
        }

        if dropped {
            let entity = current(&mut self.root, self.task.depth);
            entity.anomalies.insert(MimeAnomaly::DroppedAnswer);
        }

        self.split(Some(&mid))
    }

    /// Splits the multipart at its delimiter lines, from the dash lines
    /// already found and, for the fallback, from the boundary's mid-line
    /// occurrences.
    #[inline(always)]
    fn split(&mut self, mid: Option<&[usize]>) -> Option<MimeParseYield> {
        let input = self.input;
        let body = self.split.body.clone();
        let boundary = &*self.split.boundary;
        let delimiters = &mut self.delimiters;

        let dashes = self.dashes.as_deref().unwrap_or_default();
        let from = dashes.partition_point(|&at| at < body.start);
        let to = dashes.partition_point(|&at| at < body.end);
        line_delimiters(input, &body, boundary, &dashes[from..to], delimiters);

        let needs_open = delimiters.first().is_none_or(|delimiter| delimiter.close);
        let needs_close = !delimiters.last().is_some_and(|delimiter| delimiter.close);

        let Some(mid) = mid else {
            if needs_open || needs_close {
                self.state = State::Find;
                return Some(MimeParseYield::WantsFind {
                    range: body,
                    needle: boundary.to_vec(),
                });
            }

            return self.build(false);
        };

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

        self.build(fell_back)
    }

    /// Records the multipart its delimiters split, and queues its parts.
    #[inline(always)]
    fn build(&mut self, fell_back: bool) -> Option<MimeParseYield> {
        let input = self.input;
        let split = &self.split;
        let delimiters = &self.delimiters;
        let entity = current(&mut self.root, self.task.depth);

        if delimiters.is_empty() {
            entity.anomalies.insert(MimeAnomaly::MissingDelimiter);
            return None;
        }

        let opens = delimiters
            .iter()
            .position(|delimiter| delimiter.close)
            .unwrap_or(delimiters.len());

        if self.entities + self.tasks.len() + opens > self.max_entities {
            entity.anomalies.insert(MimeAnomaly::PartLimit);
            return None;
        }

        if fell_back {
            entity.anomalies.insert(MimeAnomaly::FallbackDelimiter);
        }

        let closed = delimiters.last().is_some_and(|delimiter| delimiter.close);

        if !closed {
            entity.anomalies.insert(MimeAnomaly::Unterminated);
        }

        let body = split.body.clone();
        let (close, epilogue) = delimiters.get(opens).map_or((&[][..], &[][..]), |close| {
            (&input[close.start..close.end], &input[close.end..body.end])
        });

        // NOTE: each part joins the multipart when its task starts.
        entity.body = MimeBodyNode::Multipart(Box::new(MimeMultipart {
            preamble: Cow::Borrowed(&input[body.start..delimiters[0].start]),
            parts: Vec::with_capacity(opens),
            close: Cow::Borrowed(close),
            epilogue: Cow::Borrowed(epilogue),
        }));

        let task = |i: usize| Task {
            range: delimiters[i].end..delimiters.get(i + 1).map_or(body.end, |next| next.start),
            delimiter: delimiters[i].start,
            depth: split.depth + 1,
            digest: split.digest,
        };
        let first = (opens > 0).then(|| task(0));
        self.tasks.extend((1..opens).rev().map(task));

        if let Some(first) = first {
            self.enter(first);
        }

        None
    }
}

/// What the next resume does.
#[derive(Clone, Copy, Debug)]
enum State {
    /// Starts the task.
    Start,
    /// Reads the body of the task, its header block already set.
    Body,
    /// Reads the header block of the task from its field ends.
    FieldEnds,
    /// Splits the multipart from the dash lines of its body.
    DashLines,
    /// Splits the multipart again from the boundary's occurrences.
    Find,
    /// Takes the next task, or completes.
    Next,
    /// Completed.
    Done,
}

/// An entity to parse: its bytes, where the delimiter line opening it
/// starts when it is a part, its depth, and whether its multipart is a
/// digest.
#[derive(Debug)]
struct Task {
    range: Range<usize>,
    delimiter: usize,
    depth: usize,
    digest: bool,
}

/// A multipart to split: its body, boundary and depth, and whether it is a
/// digest.
#[derive(Debug, Default)]
struct Split<'a> {
    body: Range<usize>,
    boundary: Cow<'a, [u8]>,
    depth: usize,
    digest: bool,
}

/// The entity at `depth` on the way to the one being parsed, the root at
/// depth 1: through the last part of each multipart.
fn current<'e, 'a>(root: &'e mut MimeEntity<'a>, depth: usize) -> &'e mut MimeEntity<'a> {
    let mut entity = root;

    for _ in 1..depth {
        entity = match &mut entity.body {
            MimeBodyNode::Multipart(multipart) => match multipart.parts.last_mut() {
                Some(part) => &mut part.entity,
                None => unreachable!("a started part"),
            },
            MimeBodyNode::Message(nested) => nested,
            MimeBodyNode::Leaf(_) | MimeBodyNode::Unparsed(_) => unreachable!("a parsed path"),
        };
    }

    entity
}

/// What the structural fields make of a body.
enum Kind<'a> {
    Multipart {
        boundary: Cow<'a, [u8]>,
        digest: bool,
    },
    Message,
    Leaf,
}

impl<'a> Kind<'a> {
    fn of(header: &MimeHeaderBlock<'a>, digest: bool) -> Self {
        let Some(field) = header.get_all("Content-Type").last() else {
            return if digest { Self::Message } else { Self::Leaf };
        };

        // NOTE: a type opening with any letter but `m` is neither a multipart
        // nor a message, which spares the common leaf its lexing.
        let value = field.value();
        let first = value.iter().find(|byte| !matches!(byte, b' ' | b'\t'));

        if matches!(
            first,
            Some(b'a'..=b'l' | b'n'..=b'z' | b'A'..=b'L' | b'N'..=b'Z')
        ) {
            return Self::Leaf;
        }

        let mut lex = Lexer::new(value);
        let (top, sub) = media(&mut lex);

        if top.is_empty() {
            return if digest { Self::Message } else { Self::Leaf };
        }

        if !eq_ignore_case(top, b"multipart") {
            let message = eq_ignore_case(top, b"message")
                && (eq_ignore_case(sub, b"rfc822") || eq_ignore_case(sub, b"global"));

            return if message && identity_encoding(header) {
                Self::Message
            } else {
                Self::Leaf
            };
        }

        // NOTE: a boundary is borrowed from the input whenever the field is.
        let digest = eq_ignore_case(sub, b"digest");
        let at = lex.pos;
        let [boundary] = match field.borrowed_value() {
            Some(bytes) => raw_params(Lexer { bytes, pos: at }, ["boundary"]),
            None => raw_params(lex, ["boundary"]).map(|b| b.map(|b| Cow::Owned(b.into_owned()))),
        };

        match boundary.filter(|b| !b.is_empty()) {
            Some(boundary) => Self::Multipart { boundary, digest },
            None => Self::Leaf,
        }
    }
}

/// Whether the entity's transfer encoding leaves its body as written:
/// none, `7bit`, `8bit`, `binary`, or one this crate does not know.
pub(crate) fn identity_encoding(header: &MimeHeaderBlock<'_>) -> bool {
    let Some(field) = header.get_all("Content-Transfer-Encoding").last() else {
        return true;
    };

    !matches!(
        value::transfer_encoding(field.value()),
        MimeTransferEncoding::Base64 | MimeTransferEncoding::QuotedPrintable
    )
}

/// A delimiter line, from the line ending before it to the end of its own
/// line.
#[derive(Debug)]
struct Delimiter {
    start: usize,
    end: usize,
    close: bool,
}

impl Delimiter {
    // NOTE: inlined so the delimiter stays in registers, for the reason the
    // parse steps are.
    #[inline(always)]
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

/// Fills `delimiters` with the delimiter lines among the dash lines of a
/// body, in order, the scan stopping after the first close delimiter.
fn line_delimiters(
    input: &[u8],
    body: &Range<usize>,
    boundary: &[u8],
    candidates: &[usize],
    delimiters: &mut Vec<Delimiter>,
) {
    delimiters.clear();
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

/// Pushes a field cut from its whole line, its line ending included; a
/// line too long for the offsets stays opaque.
fn push_field<'a>(
    line: &'a [u8],
    name_end: usize,
    colon_end: usize,
    value_end: usize,
    items: &mut Vec<MimeHeaderItem<'a>>,
) {
    // NOTE: with the room reserved first, the item is written straight into
    // its slot instead of being staged on the stack and copied, which made
    // the copy wait on the narrower stores staging it.
    items.resize_with(items.len() + 1, || {
        match MimeHeaderField::new(Cow::Borrowed(line), name_end, colon_end, value_end) {
            Some(field) => MimeHeaderItem::Field(field),
            None => MimeHeaderItem::Opaque(Cow::Borrowed(line)),
        }
    });
}
