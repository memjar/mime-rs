//! The differential oracle: one input read by this crate and by mail-parser
//! 1.0, compared on the part tree, the decoded content and decode problems
//! of each leaf, the decoded header values and the body lists.
//!
//! mail-parser's reading is its serde form, the shape of the frozen answers
//! under tests/corpus. Every difference this crate makes on purpose is let
//! through by a named [`Rule`] stating its reason, and a part a rule
//! explains is left out of the comparisons below it. A header value is
//! compared only where the validator finds its syntax sound and its RFC
//! grammar holds: on malformed values both parsers recover by policy, and
//! tests/vectors pins this crate's.

// NOTE: included by path from the integration tests and the fuzz crate, so
// it names this crate `mime` and nothing of the tests; each uses part of it.
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};

use mime::{
    codec::base64,
    header::{MimeHeaderKind, spec::MimeHeaderForm},
    tree::{body::MimeBodyNode, cst::MimeCst, entity::MimeEntity, header::MimeHeaderItem},
    validator::MimeValidateRule,
    value::{MimeHeaderValue, transfer_encoding::MimeTransferEncoding},
};
use serde_json::{Value, json};

/// A deliberate difference, with its reason; tests/corpus/divergence.tsv
/// records it per fixture where a vendored fixture shows it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Rule {
    /// Any input is a message; mail-parser refuses an input holding no
    /// part at all.
    Refused,
    /// A block of lines holding no field is no header block: the entity is
    /// all body (RFC 5322 3.5); mail-parser makes it a header block of no
    /// field when an empty line follows.
    NoFieldBlock,
    /// The line ending before a delimiter line belongs to the delimiter
    /// (RFC 2046 5.1.1), also when a part's header block runs into it;
    /// mail-parser gives it to the header block.
    HeaderIntoDelimiter,
    /// An open delimiter ending its multipart opens an empty part (RFC 2046
    /// 5.1.1); mail-parser drops it.
    OpenAtEnd,
    /// A nested multipart reusing an enclosing boundary (RFC 2046 5.1.2
    /// forbids it) gives the shared delimiter lines to the outer one;
    /// mail-parser lets the innermost claim them.
    ReusedBoundary,
    /// A base64 or quoted-printable `message/rfc822` (RFC 2046 5.2.1
    /// forbids it) stays a leaf, parsed when read; mail-parser parses it
    /// at once.
    EncodedMessage,
    /// A parameter written with no `;` before it is read (RFC 2045 5.1
    /// requires one, mailers omit it); mail-parser reads one only after a
    /// fold.
    ParamWithoutSemicolon,
    /// UTF-7 (RFC 2152) is no WHATWG encoding, so it reads as UTF-8 (D3);
    /// mail-parser decodes it.
    Utf7,
    /// A `format=flowed` part reads unwrapped (RFC 3676 4.2); mail-parser
    /// returns it as written.
    Flowed,
    /// A byte outside the base64 alphabet, or data after the padding, is
    /// reported malformed (RFC 2045 6.8); mail-parser skips some silently.
    Base64Stray,
    /// Malformed quoted-printable (lowercase hex digits, raw 8-bit or
    /// control bytes, bare CRs, an `=` before no hex digits) decodes by the
    /// codec's rules and is reported (RFC 2045 6.7); mail-parser decodes it
    /// its own way, silently.
    QuotedPrintableReported,
    /// A text leaf's leading byte order mark names its charset and is no
    /// text (spec content-decoding); mail-parser keeps it and reads the
    /// label.
    ByteOrderMark,
    /// US-ASCII text holding 8-bit bytes decodes as UTF-8, its superset,
    /// the bytes that are not UTF-8 replaced and reported; mail-parser
    /// decodes it as windows-1252, as WHATWG labels it.
    AsciiAsUtf8,
    /// A legacy charset decodes by its WHATWG table (encoding_rs), where
    /// windows-1252 maps 0x81, 0x8D, 0x8F, 0x90 and 0x9D to C1 controls;
    /// mail-parser's tables leave them holes, replaced and reported.
    WhatwgTable,
    /// A charset label that is no plain token reads by this crate's rules
    /// (an RFC 2231 language suffix such as `us-ascii*en` dropped);
    /// mail-parser knows no such charset.
    OddCharset,
    /// A Content-Transfer-Encoding that is no single token (RFC 2045 6.1)
    /// reads by this crate's recovery rules; mail-parser takes the whole
    /// value for an unknown mechanism.
    MalformedTransferEncoding,
    /// A fold inside a quoted parameter value unfolds to its blank (RFC
    /// 5322 2.2.3); mail-parser drops the blank with the fold.
    QuotedFold,
    /// A subtype after a fold behind its `/` is read, RFC 2045 5.1 letting
    /// CFWS between tokens; mail-parser ends the subtype at the fold.
    FoldedSubtype,
    /// A parameter with a fold before or after its `=` is read, RFC 2045
    /// 5.1 letting CFWS between tokens; mail-parser drops the parameter.
    FoldAroundEquals,
    /// An address list outside the RFC 5322 3.4 grammar is read by this
    /// crate's recovery rules (tests/vectors pins them: a lone word is an
    /// address, text after the domain is dropped); mail-parser recovers its
    /// own way.
    MalformedAddresses,
    /// An address keeps no blank, comment or obsolete route inside it (RFC
    /// 5322 3.4.1, 4.4); mail-parser keeps them.
    AddressBlanks,
    /// A quoted local part keeps its parentheses and quoted pairs as
    /// written (RFC 5322 3.2.4); mail-parser reads a comment there, or
    /// drops the backslash.
    QuotedLocalPart,
    /// A display name is trimmed, its blank runs collapse to one space and
    /// its words join by one; mail-parser keeps quoted blanks as written
    /// and glues words written with none between them.
    NameBlanks,
    /// A year of five digits or more reads whole (RFC 5322 3.3 year is
    /// 4*DIGIT); mail-parser keeps its first four.
    LongYear,
    /// An encoded word inside an angle address, which RFC 2047 5 forbids,
    /// is part of the address; mail-parser lifts it into the name.
    WordInAddress,
    /// An identifier list outside the RFC 5322 3.6.4 msg-id grammar is read
    /// by this crate's recovery rules (an identifier keeps no blank or
    /// fold inside its brackets); mail-parser recovers its own way.
    MalformedIds,
    /// A parameter with an empty value is dropped; mail-parser carries an
    /// empty RFC 2231 section into the next parameter.
    EmptyParameter,
    /// A `=?` that starts no word this build decodes stays as written (RFC
    /// 2047 6.3); mail-parser decodes what it can.
    BrokenWord,
    /// A folded text value reads unfolded, a run of blanks holding a fold
    /// collapsing to one space (RFC 5322 2.2.3); mail-parser keeps some
    /// fields as written.
    FoldedText,
    /// A line of blanks after a line holding no field folds into it (RFC
    /// 5322 2.2.3), as after a field; mail-parser ends the header block on
    /// it.
    BlankFold,
    /// A last part holding no field, unterminated, is a part, all body, the
    /// tree holding every byte; mail-parser drops it.
    NoFieldPartAtEnd,
    /// A boundary made only of dashes never matches, a delimiter of it
    /// reading as the close of a shorter one, so its multipart holds no part
    /// (spec parsing); mail-parser makes the entity a leaf.
    DashBoundary,
    /// A `message/rfc822` whose content opens with no field stays a leaf;
    /// mail-parser nests a message of no field.
    MessageWithoutField,
    /// A `--` and the boundary followed by other text, or found mid-line,
    /// is content (RFC 2046 5.1.1) but for the fallbacks of the spec;
    /// mail-parser ends an unterminated part on it.
    LooseDelimiter,
    /// A first line starting with a blank continues no field, so it holds
    /// none (RFC 5322 2.2.3); mail-parser skips the blanks and reads one.
    IndentedFirstLine,
    /// A field this crate's validator finds at fault is read by its
    /// recovery rules (tests/vectors pins them); mail-parser recovers its
    /// own way. A field the validator accepts must read alike.
    Faulted,
    /// A field name holding a control byte is kept as written and matches
    /// no known field, so it is never smuggled; mail-parser skips a leading
    /// CR or form feed, and reads no field from a name of nothing else.
    ControlInName,
    /// A Content-Type outside the RFC 2045 5.1 grammar, or a
    /// Content-Disposition outside RFC 2183 2, is read by this crate's
    /// recovery rules (tests/vectors pins them, an invalid type reading as
    /// text/plain per RFC 2045 5.2); mail-parser recovers its own way.
    MalformedContentType,
    /// A field read in another form than mail-parser's: Return-Path as an
    /// address (RFC 5322 3.6.7), List-Id as a phrase and an identifier
    /// (RFC 2919 3), Disposition-Notification-To as mailboxes (RFC 8098).
    OtherForm,
}

/// One difference no rule explains.
#[derive(Clone, Debug)]
pub struct Difference {
    /// Where: `structure`, `part[N].text`, `part[N].<field>`, a body list.
    pub path: String,
    pub ours: String,
    pub theirs: String,
}

/// What a comparison found.
#[derive(Debug, Default)]
pub struct Outcome {
    pub differences: Vec<Difference>,
    /// The rules that let a difference through, once per use.
    pub rules: Vec<Rule>,
}

/// One part of a reading, with its offsets into the input and its index:
/// this crate's entity index, or mail-parser's part id.
struct Node {
    header: usize,
    body: usize,
    end: usize,
    kind: Kind,
    children: Vec<Node>,
    index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Leaf,
    Multipart,
    Message,
}

/// Compares this crate's reading of `input` with mail-parser's, `answer`
/// being its serde form, or `None` when it refused the input. In `strict`
/// mode no rule applies, and content, headers and body lists are compared
/// only when the whole structure agrees.
pub fn compare(input: &[u8], answer: Option<&Value>, strict: bool) -> Outcome {
    let mut outcome = Outcome::default();

    let Some(answer) = answer else {
        outcome.rules.push(Rule::Refused);
        return outcome;
    };

    let cst = MimeCst::parse(input);
    let entities = cst.entities();
    let ours = ours(&cst.root, &mut 0, &mut 0);
    let roots: HashMap<u64, u64> = answer["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| Some((message["container"].as_u64()?, message["root"].as_u64()?)))
        .collect();
    let theirs = theirs(answer, &roots, 0);

    let mut walk = Walk {
        input,
        answer,
        entities: &entities,
        strict,
        enclosing: Vec::new(),
        loose: 0,
        aligned: Vec::new(),
        // NOTE: only the problems of a value's syntax; a long line says
        // nothing of how it reads.
        faulted: cst
            .check()
            .problems
            .into_iter()
            .filter(|problem| {
                matches!(
                    problem.rule,
                    MimeValidateRule::Unreadable(_)
                        | MimeValidateRule::Obsolete(_)
                        | MimeValidateRule::Address
                        | MimeValidateRule::MessageId
                        | MimeValidateRule::Raw8bit
                        | MimeValidateRule::StrayControl
                )
            })
            .filter_map(|problem| Some((problem.entity, problem.field?.to_ascii_lowercase())))
            .collect(),
        outcome,
    };

    walk.node(&ours, &theirs);

    let parts = answer["parts"].as_array().map_or(0, Vec::len);
    let isomorphic = walk.outcome.differences.is_empty()
        && walk.outcome.rules.is_empty()
        && walk.aligned.len() == entities.len()
        && entities.len() == parts;

    if !strict || isomorphic {
        for (ours, theirs) in std::mem::take(&mut walk.aligned) {
            walk.content(ours, theirs);

            walk.headers(ours, theirs);
        }
    }

    if isomorphic {
        walk.lists(&cst);
    }

    walk.outcome
}

fn ours(entity: &MimeEntity<'_>, offset: &mut usize, count: &mut usize) -> Node {
    let index = *count;
    *count += 1;

    let header = *offset;
    *offset += entity
        .header
        .items
        .iter()
        .map(|item| match item {
            MimeHeaderItem::Field(field) => field.line().len(),
            MimeHeaderItem::Opaque(line) => line.len(),
        })
        .sum::<usize>()
        + entity.header.end.len();
    let body = *offset;
    let mut children = Vec::new();

    let kind = match &entity.body {
        MimeBodyNode::Leaf(bytes) => {
            *offset += bytes.len();
            Kind::Leaf
        }
        MimeBodyNode::Unparsed(unparsed) => {
            *offset += unparsed.bytes.len();
            Kind::Leaf
        }
        MimeBodyNode::Multipart(multipart) => {
            *offset += multipart.preamble.len();

            for part in &multipart.parts {
                *offset += part.delimiter.len();
                children.push(ours(&part.entity, offset, count));
            }

            *offset += multipart.close.len() + multipart.epilogue.len();
            Kind::Multipart
        }
        MimeBodyNode::Message(nested) => {
            children.push(ours(nested, offset, count));
            Kind::Message
        }
    };

    Node {
        header,
        body,
        end: *offset,
        kind,
        children,
        index,
    }
}

fn theirs(answer: &Value, roots: &HashMap<u64, u64>, id: usize) -> Node {
    let part = &answer["parts"][id];
    let offset = |name: &str| part[name].as_u64().unwrap_or_default() as usize;

    let (kind, ids) = match part["kind"].as_str() {
        Some("multipart") => (
            Kind::Multipart,
            part["children"]
                .as_array()
                .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
        ),
        Some("message") => (
            Kind::Message,
            roots.get(&(id as u64)).copied().into_iter().collect(),
        ),
        _ => (Kind::Leaf, Vec::new()),
    };

    Node {
        header: offset("offset_header"),
        body: offset("offset_body"),
        end: offset("offset_end"),
        kind,
        children: ids
            .into_iter()
            .map(|child| theirs(answer, roots, child as usize))
            .collect(),
        index: id,
    }
}

struct Walk<'w, 't, 'a> {
    input: &'w [u8],
    answer: &'w Value,
    entities: &'w [&'t MimeEntity<'a>],
    strict: bool,
    /// The boundaries of the multiparts enclosing the part walked.
    enclosing: Vec<String>,
    /// How many of them hold a loose delimiter.
    loose: usize,
    /// The pairs of parts read alike: this crate's index, then theirs.
    aligned: Vec<(usize, usize)>,
    /// The fields the validator finds at fault: entity index, lowercase
    /// name.
    faulted: BTreeSet<(usize, String)>,
    outcome: Outcome,
}

impl Walk<'_, '_, '_> {
    fn differ(
        &mut self,
        path: impl Into<String>,
        ours: impl Into<String>,
        theirs: impl Into<String>,
    ) {
        self.outcome.differences.push(Difference {
            path: path.into(),
            ours: ours.into(),
            theirs: theirs.into(),
        });
    }

    /// Lets a difference through by `rule` when it holds, unless strict.
    fn allow(&mut self, rule: Rule, holds: bool) -> bool {
        if holds && !self.strict {
            self.outcome.rules.push(rule);
            return true;
        }

        false
    }

    /// Records a structural difference, unless a multipart reuses an
    /// enclosing boundary, which explains any.
    fn structure(&mut self, ours: &Node, theirs: &Node, what: &str) {
        let entity = self.entities[ours.index];
        // NOTE: a boundary reused anywhere moves the parts after it.
        let reused =
            reuses_boundary(entity, &self.enclosing) || reuses_boundary(self.entities[0], &[]);
        let loose = self.loose > 0
            || (ours.kind == Kind::Multipart
                && entity
                    .content_type()
                    .param("boundary")
                    .is_some_and(|boundary| {
                        loose_delimiter(&self.input[ours.body..ours.end], &boundary)
                    }));

        // NOTE: a malformed Content-Type anywhere hides the boundary a
        // nested multipart may reuse, which moves the parts after it.
        let malformed = self
            .entities
            .iter()
            .any(|entity| malformed_content_type(entity));
        // NOTE: a boundary folded inside its quotes reads otherwise.
        let folded = self.entities.iter().any(|entity| {
            entity
                .header
                .get_all("Content-Type")
                .last()
                .is_some_and(|field| quoted_fold(field.value()))
        });
        let control = self.entities.iter().any(|entity| control_in_name(entity));
        let indented = self
            .entities
            .iter()
            .any(|entity| indented_first_line(entity));

        if self.allow(Rule::ReusedBoundary, reused)
            || self.allow(Rule::LooseDelimiter, loose)
            || self.allow(Rule::MalformedContentType, malformed)
            || self.allow(Rule::QuotedFold, folded)
            || self.allow(Rule::ControlInName, control)
            || self.allow(Rule::IndentedFirstLine, indented)
        {
            return;
        }

        let shape = |node: &Node| {
            format!(
                "part {}: {}..{}..{} {:?} of {}",
                node.index,
                node.header,
                node.body,
                node.end,
                node.kind,
                node.children.len()
            )
        };

        self.differ(
            "structure",
            format!("{} {what}", shape(ours)),
            shape(theirs),
        );
    }

    fn node(&mut self, ours: &Node, theirs: &Node) {
        let entity = self.entities[ours.index];

        if ours.header != theirs.header {
            self.structure(ours, theirs, "starts elsewhere");
            return;
        }

        // NOTE: a header block read differently ends the comparison of the
        // part's body, which starts elsewhere.
        if ours.body != theirs.body {
            if self.allow(
                Rule::HeaderIntoDelimiter,
                ours.body == ours.end && self.runs_into_delimiter(ours.body, theirs),
            ) {
                self.aligned.push((ours.index, theirs.index));
                return;
            }

            let no_field = self.answer["parts"][theirs.index]["headers"]
                .as_array()
                .is_none_or(Vec::is_empty);

            // NOTE: mail-parser's block ends on the line of blanks folding
            // into a line holding no field.
            let line_start = self.input[..theirs.body.saturating_sub(1)]
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |at| at + 1);
            let line = self.input.get(line_start..theirs.body).unwrap_or_default();
            let blank_fold = ours.body > theirs.body
                && line_start > theirs.header
                && matches!(line.first(), Some(b' ' | b'\t'))
                && line
                    .iter()
                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\x0c' | b'\n'));

            let indented = ours.body == ours.header
                && matches!(self.input.get(ours.header), Some(b' ' | b'\t'));

            let explained = self.allow(
                Rule::NoFieldBlock,
                ours.body == ours.header && no_field && theirs.body > theirs.header,
            ) || self.allow(Rule::BlankFold, blank_fold)
                || self.allow(Rule::IndentedFirstLine, indented);

            // NOTE: the blocks read differently, their fields and content
            // compare no further.
            if !explained {
                self.structure(ours, theirs, "body starts elsewhere");
            } else if ours.end != theirs.end
                && !(ours.end < theirs.end && self.runs_into_delimiter(ours.end, theirs))
            {
                self.structure(ours, theirs, "ends elsewhere");
            }

            return;
        }

        let boundary = (ours.kind == Kind::Multipart)
            .then(|| entity.content_type().param("boundary"))
            .flatten();

        if ours.kind != theirs.kind || ours.children.len() != theirs.children.len() {
            let encoded = matches!(
                entity.transfer_encoding(),
                MimeTransferEncoding::Base64 | MimeTransferEncoding::QuotedPrintable
            );
            let extra = (ours.kind == theirs.kind
                && ours.children.len() == theirs.children.len() + 1)
                .then(|| ours.children.last())
                .flatten();
            let open_at_end =
                extra.is_some_and(|last| last.header == last.end && last.end == ours.end);
            let no_field_at_end = extra.is_some_and(|last| last.header == last.body);

            if self.allow(Rule::OpenAtEnd, open_at_end)
                || self.allow(Rule::NoFieldPartAtEnd, no_field_at_end)
            {
                // NOTE: the extra last part left out, the others compare.
                self.aligned.push((ours.index, theirs.index));
                self.children(ours, theirs, boundary);
                return;
            }

            let content = entity.bytes().unwrap_or_default();
            let explained = self.allow(
                Rule::EncodedMessage,
                ours.kind == Kind::Leaf && theirs.kind == Kind::Message && encoded,
            ) || self.allow(
                Rule::MessageWithoutField,
                ours.kind == Kind::Leaf
                    && theirs.kind == Kind::Message
                    && !opens_with_field(content),
            ) || self.allow(
                Rule::DashBoundary,
                ours.kind == Kind::Multipart
                    && ours.children.is_empty()
                    && theirs.kind == Kind::Leaf
                    && boundary.as_deref().is_some_and(|boundary| {
                        !boundary.is_empty() && boundary.bytes().all(|byte| byte == b'-')
                    }),
            ) || self.allow(
                Rule::ParamWithoutSemicolon,
                ours.kind != theirs.kind && param_without_semicolon(entity),
            ) || self
                .allow(Rule::MalformedContentType, malformed_content_type(entity));

            if !explained {
                self.structure(ours, theirs, "differs in kind or parts");
            } else if ours.end != theirs.end {
                self.structure(ours, theirs, "ends elsewhere");
            }

            return;
        }

        if ours.end != theirs.end {
            self.structure(ours, theirs, "ends elsewhere");
            return;
        }

        self.aligned.push((ours.index, theirs.index));
        self.children(ours, theirs, boundary);
    }

    /// Whether mail-parser's part ends past `ours` on the line ending of the
    /// delimiter line that follows, giving it to its header block.
    fn runs_into_delimiter(&self, ours: usize, theirs: &Node) -> bool {
        let gap = self.input.get(ours..theirs.end).unwrap_or_default();

        theirs.body == theirs.end
            && matches!(gap, b"\n" | b"\r\n")
            && self.input[theirs.end..].starts_with(b"--")
    }

    fn children(&mut self, ours: &Node, theirs: &Node, boundary: Option<String>) {
        let loose = boundary
            .as_ref()
            .is_some_and(|boundary| loose_delimiter(&self.input[ours.body..ours.end], boundary));
        let pushed = boundary.is_some();
        self.enclosing.extend(boundary);
        self.loose += usize::from(loose);

        for (ours, theirs) in ours.children.iter().zip(&theirs.children) {
            self.node(ours, theirs);
        }

        self.loose -= usize::from(loose);

        if pushed {
            self.enclosing.pop();
        }
    }

    /// Compares the decoded content and decode problems of a leaf.
    fn content(&mut self, ours: usize, theirs: usize) {
        let entity = self.entities[ours];
        let part = &self.answer["parts"][theirs];
        let content_type = entity.content_type();
        let utf7 = content_type
            .param("charset")
            .is_some_and(|charset| charset.to_ascii_lowercase().contains("utf-7"));
        let flowed = content_type
            .param("format")
            .is_some_and(|format| format.eq_ignore_ascii_case("flowed"));
        let bom = entity.decoded().is_some_and(|bytes| {
            [&b"\xef\xbb\xbf"[..], b"\xfe\xff", b"\xff\xfe"]
                .iter()
                .any(|mark| bytes.starts_with(mark))
        });
        let ascii = content_type.param("charset").is_some_and(|charset| {
            ["us-ascii", "ascii", "us_ascii", "ansi_x3.4-1968"]
                .iter()
                .any(|label| charset.trim().eq_ignore_ascii_case(label))
        }) && entity.decoded().is_some_and(|bytes| !bytes.is_ascii());
        let encoding = entity
            .header
            .get_all("Content-Transfer-Encoding")
            .last()
            .is_some_and(|field| {
                let blank = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\r' | b'\n');
                let value = field.value();
                let start = value.iter().position(|byte| !blank(byte));
                let end = value.iter().rposition(|byte| !blank(byte));

                match (start, end) {
                    (Some(start), Some(end)) => !value[start..=end]
                        .iter()
                        .all(|byte| (0x21..0x7f).contains(byte)),
                    _ => true,
                }
            });
        let malformed = malformed_content_type(entity);
        // NOTE: a charset label that is no plain token, such as `us-ascii*`.
        let odd_charset = content_type.param("charset").is_some_and(|label| {
            label.is_empty()
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:+".contains(&byte))
        });
        let checked = entity.text_checked();
        let qp_recovered = entity.transfer_encoding() == MimeTransferEncoding::QuotedPrintable
            && checked
                .as_ref()
                .is_some_and(|(_, problems)| problems.malformed_encoding);
        let c1 = checked
            .as_ref()
            .is_some_and(|(text, _)| text.chars().any(|c| ('\u{80}'..='\u{9f}').contains(&c)));

        if let Some(expected) = part["text"].as_str() {
            let got = entity.text();
            let c1_holes = c1
                && got.as_deref().is_some_and(|got| {
                    got.chars()
                        .map(|c| match c {
                            '\u{80}'..='\u{9f}' => '\u{fffd}',
                            _ => c,
                        })
                        .eq(expected.chars())
                });

            if got.as_deref() != Some(expected)
                && !self.allow(Rule::Utf7, utf7)
                && !self.allow(Rule::Flowed, flowed)
                && !self.allow(Rule::ByteOrderMark, bom)
                && !self.allow(Rule::AsciiAsUtf8, ascii)
                && !self.allow(Rule::MalformedTransferEncoding, encoding)
                && !self.allow(Rule::MalformedContentType, malformed)
                && !self.allow(Rule::OddCharset, odd_charset)
                && !self.allow(Rule::QuotedPrintableReported, qp_recovered)
                && !self.allow(Rule::WhatwgTable, c1_holes)
            {
                self.differ(
                    format!("part[{ours}].text"),
                    json!(got).to_string(),
                    json!(expected).to_string(),
                );
            }
        } else if let Some(expected) = part["binary"].as_str() {
            let mut got = Vec::new();
            base64::encode_b(&entity.decoded().unwrap_or_default(), &mut got);

            if got != expected.as_bytes()
                && !self.allow(Rule::QuotedPrintableReported, qp_recovered)
                && !self.allow(Rule::MalformedTransferEncoding, encoding)
            {
                self.differ(
                    format!("part[{ours}].binary"),
                    String::from_utf8_lossy(&got),
                    expected,
                );
            }
        }

        let Some(expected) = part["decode_problems"].as_array() else {
            return;
        };

        let mut theirs: Vec<&str> = expected.iter().filter_map(Value::as_str).collect();
        theirs.sort_unstable();

        let problems = if theirs.iter().any(|problem| problem.ends_with("charset"))
            || entity.text().is_some()
        {
            entity.text_checked().map(|(_, problems)| problems)
        } else {
            entity.decoded_checked().map(|(_, problems)| problems)
        }
        .unwrap_or_default();

        let mut got = Vec::new();

        if problems.malformed_charset {
            got.push("malformed_charset");
        }

        if problems.malformed_encoding {
            got.push("malformed_transfer_encoding");
        }

        if problems.unknown_charset {
            got.push("unknown_charset");
        }

        if problems.unknown_encoding {
            got.push("unknown_transfer_encoding");
        }

        got.sort_unstable();

        // NOTE: mail-parser checks the charset of text parts only.
        let kind = part["kind"].as_str().unwrap_or_default();

        if kind != "text" && kind != "html" {
            got.retain(|problem| !problem.ends_with("charset"));
        }

        const MALFORMED: &str = "malformed_transfer_encoding";

        let stray = entity.transfer_encoding() == MimeTransferEncoding::Base64
            && got.contains(&MALFORMED)
            && !theirs.contains(&MALFORMED)
            && entity.bytes().is_some_and(|bytes| {
                let mut padded = false;
                let significant = bytes
                    .iter()
                    .filter(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'));

                // NOTE: an incomplete last quantum too.
                significant.clone().count() % 4 != 0
                    || significant.into_iter().any(|&byte| {
                        let outside =
                            !(byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='));
                        let after_padding = padded && byte != b'=';
                        padded |= byte == b'=';
                        outside || after_padding
                    })
            });

        // NOTE: lowercase hex digits, raw 8-bit bytes and bare CRs.
        let loose_qp = entity.transfer_encoding() == MimeTransferEncoding::QuotedPrintable
            && got.contains(&MALFORMED)
            && !theirs.contains(&MALFORMED)
            && entity.bytes().is_some_and(|bytes| {
                bytes.iter().enumerate().any(|(at, &byte)| {
                    let lower = byte == b'='
                        && bytes[at + 1..]
                            .iter()
                            .take(2)
                            .any(|digit| matches!(digit, b'a'..=b'f'));
                    let bare_cr = byte == b'\r' && bytes.get(at + 1) != Some(&b'\n');
                    let control = byte < 0x20 && !matches!(byte, b'\t' | b'\r' | b'\n');
                    lower || bare_cr || control || byte >= 0x7f
                })
            });

        if self.allow(Rule::Base64Stray, stray)
            || self.allow(Rule::QuotedPrintableReported, loose_qp)
        {
            got.retain(|problem| *problem != MALFORMED);
        }

        // NOTE: a charset hole mail-parser reports, a C1 control for WHATWG.
        let c1_reported =
            c1 && theirs.contains(&"malformed_charset") && !got.contains(&"malformed_charset");

        if self.allow(Rule::WhatwgTable, c1_reported) {
            got.push("malformed_charset");
            got.sort_unstable();
        }

        if got != theirs
            && !self.allow(Rule::Utf7, utf7)
            && !self.allow(Rule::ByteOrderMark, bom)
            && !self.allow(Rule::AsciiAsUtf8, ascii)
            && !self.allow(Rule::MalformedTransferEncoding, encoding)
            && !self.allow(Rule::MalformedContentType, malformed)
            && !self.allow(Rule::OddCharset, odd_charset)
            && !self.allow(Rule::QuotedPrintableReported, qp_recovered)
        {
            self.differ(
                format!("part[{ours}].problems"),
                format!("{got:?}"),
                format!("{theirs:?}"),
            );
        }
    }

    /// Compares the decoded values of the fields both read in one form,
    /// once both list the same field names.
    fn headers(&mut self, ours: usize, theirs: usize) {
        let entity = self.entities[ours];
        let headers = self.answer["parts"][theirs]["headers"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let names: Vec<String> = entity
            .header
            .fields()
            .map(|field| field.name_str().to_ascii_lowercase())
            .collect();
        let their_names: Vec<String> = headers
            .iter()
            .map(|header| {
                header["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
            })
            .collect();

        // NOTE: mail-parser skips a leading CR or form feed, reads no field
        // from a name of nothing else, and reads one on an indented first
        // line, which has no pair here.
        let trimmed: Vec<&str> = names
            .iter()
            .map(|name| name.trim_start_matches(['\r', '\x0c', ' ', '\t']))
            .filter(|name| !name.is_empty())
            .collect();
        let controls = trimmed.len() < names.len()
            || names
                .iter()
                .any(|name| name.bytes().any(|byte| byte.is_ascii_control()));
        let skip = usize::from(indented_first_line(entity));
        let pairs = their_names.get(skip..).is_some_and(|theirs| {
            theirs
                .iter()
                .map(String::as_str)
                .eq(trimmed.iter().copied())
        });

        if names != their_names
            && !(pairs
                && (self.allow(Rule::ControlInName, controls)
                    | self.allow(Rule::IndentedFirstLine, skip == 1)))
        {
            // NOTE: past an indented first line, or with a control byte in a
            // name, the fields pair no further.
            if !self.allow(Rule::IndentedFirstLine, skip == 1)
                && !self.allow(Rule::ControlInName, controls)
            {
                self.differ(
                    format!("part[{ours}].fields"),
                    format!("{names:?}"),
                    format!("{their_names:?}"),
                );
            }

            return;
        }

        let skip = if names == their_names { 0 } else { skip };

        let fields = entity.header.fields().filter(|field| {
            !field
                .name_str()
                .trim_start_matches(['\r', '\x0c', ' ', '\t'])
                .is_empty()
        });

        for (field, header) in fields.zip(headers.iter().skip(skip)) {
            let Ok(kind) = field.name_str().parse::<MimeHeaderKind>() else {
                continue;
            };

            let (Some(ours_value), Some(mut theirs_value)) =
                (our_value(kind, field.read()), their_value(&header["value"]))
            else {
                continue;
            };

            if ours_value == theirs_value {
                continue;
            }

            // NOTE: first what reads by recovery rules or in another form,
            // compared no further.
            let value = field.value();
            let form = kind.spec().form;
            let other_form = matches!(
                (&ours_value, &theirs_value),
                (Value::String(_), Value::Array(_)) | (Value::Array(_), Value::String(_))
            ) || matches!(
                (ours_value.get(0), theirs_value.get(0)),
                (Some(Value::Object(_)), Some(Value::String(_)))
                    | (Some(Value::String(_)), Some(Value::Object(_)))
            );
            let faulted = self
                .faulted
                .contains(&(ours, field.name_str().to_ascii_lowercase()));
            let malformed_type =
                kind == MimeHeaderKind::ContentType && !well_formed_content_type(value);
            let malformed_addresses = matches!(
                form,
                MimeHeaderForm::Addresses | MimeHeaderForm::GroupedAddresses
            ) && !well_formed_addresses(value);
            let malformed_ids = form == MimeHeaderForm::MessageIds && !well_formed_ids(value);
            let long_year = form == MimeHeaderForm::Date
                && value
                    .split(|byte| !byte.is_ascii_digit())
                    .any(|digits| digits.len() > 4);
            let empty_parameter = kind == MimeHeaderKind::ContentType
                && value.windows(3).any(|window| window == b"=\"\"");
            // NOTE: a fold inside a quoted parameter value.
            let folded_quote = kind == MimeHeaderKind::ContentType && quoted_fold(value);
            // NOTE: a fold between the `/` and the subtype.
            let folded_subtype = kind == MimeHeaderKind::ContentType
                && value
                    .iter()
                    .position(|byte| *byte == b'/')
                    .is_some_and(|slash| {
                        value[slash + 1..]
                            .iter()
                            .find(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
                            == Some(&b'\n')
                    });
            // NOTE: a `=?` that starts no word this build decodes.
            let broken_word = value.windows(2).enumerate().any(|(at, pair)| {
                pair == b"=?" && mime::codec::rfc2047::word_end(value, at).is_none()
            });
            let folded_text = match (&ours_value, &theirs_value) {
                (Value::String(ours), Value::String(theirs)) => {
                    unfold(theirs) == *ours && theirs.contains(['\r', '\n'])
                }
                _ => false,
            };
            let utf7 = value
                .windows(8)
                .any(|window| window.eq_ignore_ascii_case(b"=?utf-7?"));
            let mut angle = false;
            let word_in_address = value.windows(2).any(|pair| {
                angle = match pair[0] {
                    b'<' => true,
                    b'>' => false,
                    _ => angle,
                };
                angle && pair == b"=?"
            });
            let significant = |byte: &&u8| !matches!(byte, b' ' | b'\t' | b'\r');
            let folded_value = kind == MimeHeaderKind::ContentType
                && value.iter().enumerate().any(|(at, byte)| {
                    *byte == b'='
                        && (value[at + 1..].iter().find(significant) == Some(&b'\n')
                            || value[..at].iter().rev().find(significant) == Some(&b'\n'))
                });

            if self.allow(Rule::OtherForm, other_form)
                || self.allow(Rule::Faulted, faulted)
                || self.allow(Rule::MalformedContentType, malformed_type)
                || self.allow(Rule::MalformedAddresses, malformed_addresses)
                || self.allow(Rule::Utf7, utf7)
                || self.allow(Rule::WordInAddress, word_in_address)
                || self.allow(Rule::FoldAroundEquals, folded_value)
                || self.allow(Rule::FoldedSubtype, folded_subtype)
                || self.allow(Rule::QuotedFold, folded_quote)
                || self.allow(Rule::MalformedIds, malformed_ids)
                || self.allow(Rule::LongYear, long_year)
                || self.allow(Rule::EmptyParameter, empty_parameter)
                || self.allow(Rule::BrokenWord, broken_word)
                || self.allow(Rule::FoldedText, folded_text)
            {
                continue;
            }

            // NOTE: then, item by item, mail-parser's reading is given this
            // crate's where a rule explains the difference.
            if !self.strict {
                let mut rules = Vec::new();

                for (rule, normalize) in [
                    (
                        Rule::AddressBlanks,
                        address_blanks as fn(&Value, &mut Value) -> bool,
                    ),
                    (Rule::QuotedLocalPart, quoted_local_part),
                    (Rule::NameBlanks, name_blanks),
                ] {
                    if normalize(&ours_value, &mut theirs_value) {
                        rules.push(rule);
                    }
                }

                if !rules.is_empty() && ours_value == theirs_value {
                    self.outcome.rules.extend(rules);
                    continue;
                }
            }

            self.differ(
                format!("part[{ours}].{}", field.name_str().to_ascii_lowercase()),
                ours_value.to_string(),
                theirs_value.to_string(),
            );
        }
    }

    /// Compares the body lists of the root message.
    fn lists(&mut self, cst: &MimeCst<'_>) {
        let index: HashMap<*const MimeEntity<'_>, u64> = self
            .entities
            .iter()
            .enumerate()
            .map(|(at, entity)| (*entity as *const MimeEntity<'_>, at as u64))
            .collect();
        let lists = cst.root.body_lists();
        let message = &self.answer["messages"][0];

        // NOTE: an invalid Content-Type reads as text/plain (RFC 2045 5.2),
        // which moves its part between the lists.
        let malformed = self.entities.iter().any(|entity| {
            malformed_content_type(entity)
                || entity
                    .header
                    .get_all("Content-Disposition")
                    .last()
                    .is_some_and(|field| !well_formed_disposition(field.value()))
        });

        let control = self.entities.iter().any(|entity| control_in_name(entity));
        let indented = self
            .entities
            .iter()
            .any(|entity| indented_first_line(entity));

        for (name, list) in [
            ("text_body", &lists.text),
            ("html_body", &lists.html),
            ("attachments", &lists.attachments),
        ] {
            let got: Vec<u64> = list
                .iter()
                .map(|part| index[&(*part as *const MimeEntity<'_>)])
                .collect();
            let expected: Vec<u64> = message[name]
                .as_array()
                .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default();

            if got != expected
                && !self.allow(Rule::MalformedContentType, malformed)
                && !self.allow(Rule::ControlInName, control)
                && !self.allow(Rule::IndentedFirstLine, indented)
            {
                self.differ(name, format!("{got:?}"), format!("{expected:?}"));
            }
        }

        if let Some(expected) = message["has_attachments"].as_bool()
            && lists.has_attachment() != expected
            && !self.allow(Rule::MalformedContentType, malformed)
            && !self.allow(Rule::ControlInName, control)
            && !self.allow(Rule::IndentedFirstLine, indented)
        {
            self.differ(
                "has_attachments",
                lists.has_attachment().to_string(),
                expected.to_string(),
            );
        }
    }
}

/// Whether `--` and `boundary` occur in a multipart body other than as a
/// delimiter line this crate reads: mid-line, or followed by other text.
fn loose_delimiter(body: &[u8], boundary: &str) -> bool {
    let needle = [b"--", boundary.as_bytes()].concat();

    body.windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle.as_slice())
        .any(|(at, _)| {
            let rest = &body[at + needle.len()..];
            let line_start = at == 0 || body[at - 1] == b'\n';

            !(line_start
                && (rest.is_empty()
                    || rest.starts_with(b"--")
                    || matches!(rest[0], b'\r' | b'\n' | b' ' | b'\t')))
        })
}

/// Whether the entity's last Content-Type is outside the RFC 2045 grammar.
fn malformed_content_type(entity: &MimeEntity<'_>) -> bool {
    entity
        .header
        .get_all("Content-Type")
        .last()
        .is_some_and(|field| !well_formed_content_type(field.value()))
}

/// Whether a Content-Type value follows RFC 2045 5.1: a type and a
/// subtype, then parameters whose values are tokens or quoted strings,
/// comments and folds between them; a trailing `;` is let through.
fn well_formed_content_type(value: &[u8]) -> bool {
    well_formed_params(value, true)
}

/// Whether a Content-Disposition value follows RFC 2183 2: a disposition
/// type then parameters, as [`well_formed_content_type`] reads them.
fn well_formed_disposition(value: &[u8]) -> bool {
    well_formed_params(value, false)
}

/// A media type (`media`) or a disposition type, then parameters.
fn well_formed_params(value: &[u8], media: bool) -> bool {
    fn cfws(value: &[u8], at: &mut usize) -> bool {
        let mut depth = 0usize;

        while let Some(&byte) = value.get(*at) {
            match byte {
                b'(' => depth += 1,
                b')' if depth > 0 => depth -= 1,
                b'\\' if depth > 0 => *at += 1,
                b' ' | b'\t' | b'\r' | b'\n' => {}
                _ if depth > 0 => {}
                _ => break,
            }

            *at += 1;
        }

        depth == 0
    }

    fn token(value: &[u8], at: &mut usize) -> bool {
        let start = *at;

        while value
            .get(*at)
            .is_some_and(|byte| (0x21..0x7f).contains(byte) && !b"()<>@,;:\\\"/[]?=".contains(byte))
        {
            *at += 1;
        }

        *at > start
    }

    fn quoted(value: &[u8], at: &mut usize) -> bool {
        *at += 1;

        while let Some(&byte) = value.get(*at) {
            *at += 1;

            // NOTE: a line break only as a fold, a CR only before it.
            match byte {
                b'"' => return true,
                b'\\' => *at += 1,
                b'\n' if !matches!(value.get(*at), Some(b' ' | b'\t')) => return false,
                b'\r' if value.get(*at) != Some(&b'\n') => return false,
                b'\t' | b'\r' | b'\n' | 0x20.. => {}
                _ => return false,
            }
        }

        false
    }

    let mut at = 0;

    if !(cfws(value, &mut at) && token(value, &mut at) && cfws(value, &mut at)) {
        return false;
    }

    if media {
        if value.get(at) != Some(&b'/') {
            return false;
        }

        at += 1;

        if !(cfws(value, &mut at) && token(value, &mut at) && cfws(value, &mut at)) {
            return false;
        }
    }

    // NOTE: an RFC 2231 name is attribute characters, then a section and an
    // extended mark; a name appears once (RFC 2045 5.1).
    let mut names = Vec::new();

    loop {
        match value.get(at) {
            None => return std::str::from_utf8(value).is_ok() && contiguous(&names),
            Some(b';') => at += 1,
            Some(_) => return false,
        }

        if !cfws(value, &mut at) {
            return false;
        }

        if at == value.len() {
            return std::str::from_utf8(value).is_ok() && contiguous(&names);
        }

        let start = at;

        if !token(value, &mut at) {
            return false;
        }

        let name = &value[start..at];
        let (base, mark) = name
            .iter()
            .position(|byte| *byte == b'*')
            .map_or((name, &name[name.len()..]), |star| name.split_at(star));
        let section = mark
            .strip_prefix(b"*")
            .map(|rest| rest.strip_suffix(b"*").unwrap_or(rest));
        let named = !base.is_empty()
            && !base.iter().any(|byte| matches!(byte, b'\'' | b'%'))
            && section.is_none_or(|digits| digits.iter().all(u8::is_ascii_digit));
        let key = (base.to_ascii_lowercase(), section.map(<[u8]>::to_vec));

        if !named || names.contains(&key) {
            return false;
        }

        names.push(key);

        if !cfws(value, &mut at) || value.get(at) != Some(&b'=') {
            return false;
        }

        at += 1;

        if !cfws(value, &mut at) {
            return false;
        }

        // NOTE: an RFC 2231 extended value is a token, the first section's
        // opening with a charset and a language, each ended by `'`.
        let extended = mark.ends_with(b"*");
        let first = matches!(section, Some(b"" | b"0"));
        let start = at;

        let valued = if value.get(at) == Some(&b'"') {
            !extended && quoted(value, &mut at)
        } else {
            token(value, &mut at) && (!extended || extended_value(&value[start..at], first))
        };

        if !(valued && cfws(value, &mut at)) {
            return false;
        }
    }
}

/// Whether an RFC 2231 extended value is percent escapes and attribute
/// characters, the first section opening with a charset and a language.
fn extended_value(value: &[u8], first: bool) -> bool {
    let rest = if first {
        let mut parts = value.splitn(3, |byte| *byte == b'\'');

        match (parts.next(), parts.next(), parts.next()) {
            (Some(_), Some(_), Some(rest)) => rest,
            _ => return false,
        }
    } else {
        value
    };

    let mut at = 0;

    while at < rest.len() {
        match rest[at] {
            b'%' if rest
                .get(at + 1..at + 3)
                .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit)) =>
            {
                at += 3;
            }
            b'%' | b'\'' | b'*' => return false,
            _ => at += 1,
        }
    }

    true
}

/// Whether the RFC 2231 sections of each parameter run from 0 with no gap,
/// and no parameter is both sectioned and whole.
fn contiguous(names: &[(Vec<u8>, Option<Vec<u8>>)]) -> bool {
    names.iter().all(|(base, _)| {
        let mut sections: Vec<u32> = Vec::new();
        let mut whole = 0;

        for (_, section) in names.iter().filter(|(other, _)| other == base) {
            match section.as_deref() {
                None | Some(b"") => whole += 1,
                Some(digits) => match std::str::from_utf8(digits)
                    .ok()
                    .and_then(|n| n.parse().ok())
                {
                    Some(n) => sections.push(n),
                    None => return false,
                },
            }
        }

        sections.sort_unstable();

        if sections.is_empty() {
            whole <= 1
        } else {
            whole == 0 && sections.iter().copied().eq(0..sections.len() as u32)
        }
    })
}

/// Whether an identifier field value is one or more RFC 5322 3.6.4 msg-ids,
/// comments and folds between them.
fn well_formed_ids(value: &[u8]) -> bool {
    let mut lex = Lex { raw: value, at: 0 };
    let mut ids = 0;

    let dot_atom_text = |lex: &mut Lex<'_>| {
        lex.atext() && {
            while lex.eat(b'.') {
                if !lex.atext() {
                    return false;
                }
            }

            true
        }
    };

    std::str::from_utf8(value).is_ok()
        && loop {
            if !lex.cfws() {
                break false;
            }

            if lex.peek().is_none() {
                break ids > 0;
            }

            let right = |lex: &mut Lex<'_>| {
                if lex.eat(b'[') {
                    while lex
                        .peek()
                        .is_some_and(|byte| !matches!(byte, b'[' | b']' | b'\\') && byte > 0x20)
                    {
                        lex.at += 1;
                    }

                    lex.eat(b']')
                } else {
                    dot_atom_text(lex)
                }
            };

            if !(lex.eat(b'<')
                && dot_atom_text(&mut lex)
                && lex.eat(b'@')
                && right(&mut lex)
                && lex.eat(b'>'))
            {
                break false;
            }

            ids += 1;
        }
}

/// A text with every run of blanks holding a fold collapsed to one space,
/// and trimmed.
fn unfold(text: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();

    for c in text.chars() {
        if c.is_whitespace() {
            run.push(c);
            continue;
        }

        if run.contains(['\r', '\n']) {
            out.push(' ');
        } else {
            out.push_str(&run);
        }

        run.clear();
        out.push(c);
    }

    out.trim_matches([' ', '\t']).to_string()
}

/// Whether an address field value follows the RFC 5322 3.4 address-list
/// grammar in its current syntax, UTF-8 allowed (RFC 6532), an encoded
/// word being an atom.
fn well_formed_addresses(value: &[u8]) -> bool {
    let mut lex = Lex { raw: value, at: 0 };

    std::str::from_utf8(value).is_ok()
        && loop {
            if !lex.address() || !lex.cfws() {
                break false;
            }

            match lex.peek() {
                None => break true,
                Some(b',') => lex.at += 1,
                Some(_) => break false,
            }
        }
}

/// A cursor over an address field value.
struct Lex<'r> {
    raw: &'r [u8],
    at: usize,
}

impl Lex<'_> {
    fn peek(&self) -> Option<u8> {
        self.raw.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        self.at += usize::from(found);
        found
    }

    /// Skips blanks, folds and comments, false on an unterminated comment.
    fn cfws(&mut self) -> bool {
        let mut depth = 0usize;

        while let Some(byte) = self.peek() {
            match byte {
                b'(' => depth += 1,
                b')' if depth > 0 => depth -= 1,
                b'\\' if depth > 0 => self.at += 1,
                b' ' | b'\t' | b'\r' | b'\n' => {}
                _ if depth > 0 => {}
                _ => break,
            }

            self.at += 1;
        }

        depth == 0
    }

    fn atext(&mut self) -> bool {
        let start = self.at;

        while self.peek().is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~".contains(&byte) || byte >= 0x80
        }) {
            self.at += 1;
        }

        self.at > start
    }

    fn dot_atom(&mut self) -> bool {
        if !(self.cfws() && self.atext()) {
            return false;
        }

        while self.peek() == Some(b'.') {
            self.at += 1;

            if !self.atext() {
                return false;
            }
        }

        self.cfws()
    }

    fn quoted(&mut self) -> bool {
        if !(self.cfws() && self.eat(b'"')) {
            return false;
        }

        while let Some(byte) = self.peek() {
            self.at += 1;

            match byte {
                b'"' => return self.cfws(),
                b'\\' => self.at += 1,
                b'\n' if !matches!(self.peek(), Some(b' ' | b'\t')) => return false,
                b'\t' | b'\r' | b'\n' | 0x20.. => {}
                _ => return false,
            }
        }

        false
    }

    fn word(&mut self) -> bool {
        let start = self.at;

        if self.quoted() {
            return true;
        }

        self.at = start;
        self.cfws() && self.atext() && self.cfws()
    }

    fn addr_spec(&mut self) -> bool {
        let start = self.at;
        let local = self.dot_atom() || {
            self.at = start;
            self.quoted()
        };

        if !(local && self.eat(b'@')) {
            return false;
        }

        let domain = self.at;

        self.dot_atom() || {
            self.at = domain;
            self.cfws() && self.eat(b'[') && {
                while self
                    .peek()
                    .is_some_and(|byte| !matches!(byte, b'[' | b']' | b'\\') && byte >= 0x20)
                {
                    self.at += 1;
                }

                self.eat(b']') && self.cfws()
            }
        }
    }

    fn mailbox(&mut self) -> bool {
        let start = self.at;

        if self.addr_spec() && matches!(self.peek(), None | Some(b',' | b';')) {
            return true;
        }

        self.at = start;

        loop {
            let word = self.at;

            if !self.word() {
                self.at = word;
                break;
            }
        }

        self.cfws() && self.eat(b'<') && self.addr_spec() && self.eat(b'>') && self.cfws()
    }

    fn address(&mut self) -> bool {
        let start = self.at;

        if self.mailbox() {
            return true;
        }

        self.at = start;
        let mut words = 0;

        while self.word() {
            words += 1;
        }

        if words == 0 || !self.eat(b':') || !self.cfws() {
            return false;
        }

        if self.peek() != Some(b';') {
            while self.mailbox() {
                if !self.eat(b',') {
                    break;
                }
            }
        }

        self.eat(b';') && self.cfws()
    }
}

/// Whether the entity's header block opens with a line starting with a
/// blank, which this crate keeps opaque.
fn indented_first_line(entity: &MimeEntity<'_>) -> bool {
    matches!(
        entity.header.items.first(),
        Some(MimeHeaderItem::Opaque(line)) if matches!(line.first(), Some(b' ' | b'\t'))
    )
}

/// Whether content opens with a field: a first line holding a colon, not
/// starting with a blank.
fn opens_with_field(content: &[u8]) -> bool {
    let line = content
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    !matches!(line.first(), None | Some(b' ' | b'\t')) && line.contains(&b':')
}

/// Gives mail-parser's mailboxes this crate's where this crate's address
/// holds a quoted local part or a quoted pair.
fn quoted_local_part(ours: &Value, theirs: &mut Value) -> bool {
    let (Some(ours), Some(theirs)) = (ours.as_array(), theirs.as_array_mut()) else {
        return false;
    };

    let mut any = false;

    for (ours, theirs) in ours.iter().zip(theirs.iter_mut()) {
        let quoted = ours["address"]
            .as_str()
            .is_some_and(|address| address.starts_with('"') || address.contains('\\'));

        if quoted && ours != theirs {
            *theirs = ours.clone();
            any = true;
        }
    }

    any
}

/// Gives mail-parser's display names this crate's when they differ by
/// blanks alone, an empty name reading as none.
fn name_blanks(ours: &Value, theirs: &mut Value) -> bool {
    let (Some(ours), Some(theirs)) = (ours.as_array(), theirs.as_array_mut()) else {
        return false;
    };

    let bare = |name: &str| name.split_whitespace().collect::<String>();
    let mut any = false;

    for (ours, theirs) in ours.iter().zip(theirs.iter_mut()) {
        let Some(written) = theirs["name"].as_str() else {
            continue;
        };

        let same = match ours["name"].as_str() {
            Some(name) => name != written && bare(name) == bare(written),
            None => written.trim().is_empty(),
        };

        if same {
            theirs["name"] = ours["name"].clone();
            any = true;
        }
    }

    any
}

/// Gives mail-parser's addresses this crate's when they differ by blanks,
/// comments or a route alone, returning whether one did.
fn address_blanks(ours: &Value, theirs: &mut Value) -> bool {
    let (Some(ours), Some(theirs)) = (ours.as_array(), theirs.as_array_mut()) else {
        return false;
    };

    let mut any = false;

    for (ours, theirs) in ours.iter().zip(theirs.iter_mut()) {
        let (Some(address), Some(written)) = (ours["address"].as_str(), theirs["address"].as_str())
        else {
            continue;
        };

        let mut bare = String::new();
        let mut depth = 0usize;

        for char in written.chars() {
            match char {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ if depth > 0 || char.is_whitespace() => {}
                _ => bare.push(char),
            }
        }

        let bare = match bare.strip_prefix('@') {
            Some(route) => route
                .split_once(':')
                .map_or(bare.clone(), |(_, rest)| rest.into()),
            None => bare,
        };

        if bare == address && written != address {
            theirs["address"] = Value::from(address);
            any = true;
        }
    }

    any
}

/// Whether a parameter of the entity's last Content-Type follows no `;`
/// on its line, as in `multipart/mixed boundary=b`.
fn param_without_semicolon(entity: &MimeEntity<'_>) -> bool {
    let Some(field) = entity.header.get_all("Content-Type").last() else {
        return false;
    };

    // NOTE: quoted strings and comments masked, their `;` and `=` no
    // separators.
    let mut plain = Vec::with_capacity(field.value().len());
    let mut quoted = false;
    let mut comment = 0usize;
    let mut escaped = false;

    for &byte in field.value() {
        let masked = if escaped {
            escaped = false;
            true
        } else {
            match byte {
                b'\\' if quoted || comment > 0 => {
                    escaped = true;
                    true
                }
                b'"' if comment == 0 => {
                    quoted = !quoted;
                    true
                }
                b'(' if !quoted => {
                    comment += 1;
                    true
                }
                b')' if comment > 0 => {
                    comment -= 1;
                    true
                }
                _ => quoted || comment > 0,
            }
        };

        plain.push(if masked { b' ' } else { byte });
    }

    // NOTE: a fold separates a parameter for mail-parser once a subtype was
    // read, not inside the type.
    let slash = plain.iter().position(|byte| *byte == b'/');

    plain.iter().enumerate().any(|(at, byte)| {
        let name_start = plain[..at]
            .iter()
            .rposition(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b';'))
            .map_or(0, |separator| separator + 1);
        let before = plain[..name_start]
            .iter()
            .rposition(|byte| !matches!(byte, b' ' | b'\t' | b'\r'));

        *byte == b'='
            && before.is_some_and(|before| match plain[before] {
                b';' => false,
                b'\n' => slash.is_none_or(|slash| slash > before),
                _ => true,
            })
    })
}

/// Whether `entity` or a multipart nested in it reuses one of the
/// `enclosing` boundaries, or one of its own: a boundary starting with
/// another, whose delimiter lines read as the other's, which RFC 2046 5.1.1
/// forbids, as `a----` inside `a--` or `foo` inside `foo bar`.
fn reuses_boundary(entity: &MimeEntity<'_>, enclosing: &[String]) -> bool {
    let boundary = entity.content_type().param("boundary");

    if boundary
        .as_deref()
        .is_some_and(|boundary| conflicts(boundary, enclosing))
    {
        return true;
    }

    let mut enclosing = enclosing.to_vec();
    enclosing.extend(boundary);

    match &entity.body {
        MimeBodyNode::Multipart(multipart) => multipart
            .parts
            .iter()
            .any(|part| reuses_boundary(&part.entity, &enclosing)),
        MimeBodyNode::Message(nested) => reuses_boundary(nested, &enclosing),
        // NOTE: a nested message this crate keeps a leaf may declare a
        // multipart that reuses one.
        MimeBodyNode::Leaf(bytes) => declared_boundaries(bytes)
            .iter()
            .any(|declared| conflicts(declared, &enclosing)),
        MimeBodyNode::Unparsed(_) => false,
    }
}

/// Whether `boundary` starts with one of the `enclosing` boundaries, or
/// one of them with it.
fn conflicts(boundary: &str, enclosing: &[String]) -> bool {
    !boundary.is_empty()
        && enclosing.iter().any(|outer| {
            !outer.is_empty()
                && (boundary.starts_with(outer.as_str()) || outer.starts_with(boundary))
        })
}

/// The values of every `boundary=` parameter written in `bytes`, quoted or
/// not, empty ones left out.
fn declared_boundaries(bytes: &[u8]) -> Vec<String> {
    const NAME: &[u8] = b"boundary=";

    bytes
        .windows(NAME.len())
        .enumerate()
        .filter(|(_, window)| window.eq_ignore_ascii_case(NAME))
        .filter_map(|(at, _)| {
            let rest = &bytes[at + NAME.len()..];
            let value = match rest.strip_prefix(b"\"") {
                Some(quoted) => &quoted[..quoted.iter().position(|byte| *byte == b'"')?],
                None => {
                    let end = rest
                        .iter()
                        .position(|byte| matches!(byte, b';' | b' ' | b'\t' | b'\r' | b'\n'))
                        .unwrap_or(rest.len());
                    &rest[..end]
                }
            };

            (!value.is_empty()).then(|| String::from_utf8_lossy(value).into_owned())
        })
        .collect()
}

/// This crate's reading of a field, in the shape of mail-parser's.
pub fn our_value(kind: MimeHeaderKind, value: MimeHeaderValue) -> Option<Value> {
    match value {
        MimeHeaderValue::Text(text) if !matches!(kind, MimeHeaderKind::MimeVersion) => {
            Some(Value::from(text))
        }
        MimeHeaderValue::Addresses(list) => Some(Value::Array(
            list.mailboxes()
                .map(|mailbox| json!({ "name": mailbox.name, "address": mailbox.address }))
                .collect(),
        )),
        MimeHeaderValue::Date(date) => Some(json!([
            date.year,
            date.month,
            date.day,
            date.hour,
            date.minute,
            date.second,
            date.offset,
        ])),
        MimeHeaderValue::MessageIds(ids) => Some(Value::from(ids)),
        MimeHeaderValue::ContentType(content_type) => Some(json!({
            "type": content_type.top.to_ascii_lowercase(),
            "subtype": content_type.sub.to_ascii_lowercase(),
            "attributes": content_type
                .params
                .iter()
                .map(|param| (param.name.to_ascii_lowercase(), param.value.clone()))
                .collect::<Vec<_>>(),
        })),
        _ => None,
    }
}

/// mail-parser's reading of a field, reduced to the shape of
/// [`our_value`].
pub fn their_value(value: &Value) -> Option<Value> {
    if let Some(text) = value.get("text") {
        return Some(text.clone());
    }

    if let Some(list) = value.get("text_list").and_then(Value::as_array) {
        return Some(Value::Array(list.clone()));
    }

    if let Some(items) = value.get("address").and_then(Value::as_array) {
        let mailboxes = items
            .iter()
            .flat_map(|item| match &item["group"] {
                Value::Object(group) => group["mailboxes"].as_array().cloned().unwrap_or_default(),
                _ => vec![item["mailbox"].clone()],
            })
            .map(|mailbox| {
                json!({
                    "name": mailbox.get("name").cloned().unwrap_or(Value::Null),
                    "address": mailbox.get("address").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();

        return Some(Value::Array(mailboxes));
    }

    if let Some(date) = value.get("date_time") {
        let minutes = date["tz_hour"].as_i64()? * 60 + date["tz_minute"].as_i64()?;
        let offset = if date["tz_before_gmt"].as_bool()? {
            -minutes
        } else {
            minutes
        };

        return Some(json!([
            date["year"],
            date["month"],
            date["day"],
            date["hour"],
            date["minute"],
            date["second"],
            offset,
        ]));
    }

    if let Some(content_type) = value.get("content_type") {
        let attributes: Vec<(String, String)> = content_type["attributes"]
            .as_array()
            .map(|pairs| {
                pairs
                    .iter()
                    .filter_map(|pair| {
                        Some((
                            pair[0].as_str()?.to_ascii_lowercase(),
                            pair[1].as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();

        return Some(json!({
            "type": content_type["type"].as_str()?.to_ascii_lowercase(),
            "subtype": content_type["subtype"].as_str().unwrap_or_default().to_ascii_lowercase(),
            "attributes": attributes,
        }));
    }

    None
}

/// Whether a value folds inside a quoted string.
fn quoted_fold(value: &[u8]) -> bool {
    let mut quoted = false;

    value.iter().any(|byte| {
        quoted ^= *byte == b'"';
        quoted && *byte == b'\n'
    })
}

/// Whether a field name of the entity holds a control byte, a field this
/// crate keeps out and mail-parser may read.
fn control_in_name(entity: &MimeEntity<'_>) -> bool {
    entity
        .header
        .fields()
        .any(|field| field.name_str().bytes().any(|byte| byte.is_ascii_control()))
}
