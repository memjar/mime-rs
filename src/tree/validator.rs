//! # Tree validation
//!
//! The validator over the syntax tree: the model rules of
//! [`validator`](crate::validator), read from the fields as written, plus
//! what only the wire shows: line lengths and line endings, stray control
//! bytes, 8-bit bytes in fields, encoded words, content against its
//! declared transfer encoding, boundaries, and what the parser worked
//! around. The same walk computes
//! [`MimeTransport`](crate::validator::MimeTransport).

use alloc::{string::String, vec::Vec};
use core::str;

use simd_cpu::SimdCpu;

use crate::{
    build::entity::is_boundary,
    codec::{quoted_printable, rfc2047},
    header::{MimeHeaderKind, MimeHeaderName, spec::MimeHeaderForm},
    tree::{
        body::MimeBodyNode,
        cst::MimeCst,
        entity::MimeEntity,
        header::{MimeHeaderField, MimeHeaderItem},
    },
    validator::{
        MimeSeverity, MimeValid, MimeValidateOutput, MimeValidateRule, check_content_ids,
        check_fields, check_structure,
    },
    value::{MimeHeaderValue, transfer_encoding::MimeTransferEncoding},
};

impl MimeCst<'_> {
    /// Every problem the message has, as written, and what it needs of a
    /// transport.
    pub fn check(&self) -> MimeValidateOutput {
        let mut output = MimeValidateOutput::default();
        let eol = self.root.header.line_ending();

        if eol == b"\n" {
            output.push(
                0,
                None,
                MimeValidateRule::LfLineEndings,
                MimeSeverity::Advisory,
            );
        }

        let mut walk = Walk {
            eol,
            index: 0,
            output,
        };

        walk.entity(&self.root, true, false, &mut Vec::new());
        walk.output
    }

    /// The message as a proof of validity, or every problem it has when
    /// one is a violation.
    pub fn validate(self) -> Result<MimeValid<Self>, MimeValidateOutput> {
        let output = self.check();

        if output.is_valid() {
            Ok(MimeValid::new(self, output))
        } else {
            Err(output)
        }
    }
}

struct Walk {
    eol: &'static [u8],
    index: usize,
    output: MimeValidateOutput,
}

impl Walk {
    fn entity(
        &mut self,
        entity: &MimeEntity<'_>,
        top: bool,
        nested: bool,
        content_ids: &mut Vec<String>,
    ) {
        let here = self.index;
        self.index += 1;

        let names: Vec<MimeHeaderName> = entity
            .header
            .fields()
            .map(|field| MimeHeaderName::new(&field.name_str()))
            .collect();
        let values: Vec<MimeHeaderValue> =
            entity.header.fields().map(MimeHeaderField::read).collect();
        let fields: Vec<(&MimeHeaderName, &MimeHeaderValue)> = names.iter().zip(&values).collect();

        check_fields(here, &fields, top, nested, &mut self.output);
        check_content_ids(here, &fields, content_ids, &mut self.output);

        for anomaly in entity.anomalies.iter() {
            self.push(
                here,
                None,
                MimeValidateRule::Anomaly(anomaly),
                MimeSeverity::Violation,
            );
        }

        for item in &entity.header.items {
            match item {
                MimeHeaderItem::Field(field) => self.field(here, field, nested),
                MimeHeaderItem::Opaque(line) => self.lines(here, None, line, true),
            }
        }

        if top {
            let content = names.iter().any(|name| {
                name.as_str()
                    .get(..8)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("content-"))
            });
            let version = names.iter().any(|name| name.is("mime-version"));

            if content && !version {
                let weight = if nested {
                    MimeSeverity::Advisory
                } else {
                    MimeSeverity::Violation
                };
                self.push(here, None, MimeValidateRule::MimeVersion, weight);
            }
        }

        let media = entity.content_type();
        let content_type = media.to_content_type();
        let encoding = entity.transfer_encoding();
        let composite = media.is_top("multipart")
            || media.is("message", "rfc822")
            || media.is("message", "global");

        let parts = match &entity.body {
            MimeBodyNode::Multipart(multipart) => Some(multipart.parts.len()),
            _ => None,
        };

        check_structure(here, &content_type, parts, nested, &mut self.output);

        if composite
            && !encoding.is_identity()
            && !matches!(encoding, MimeTransferEncoding::Unknown(_))
        {
            self.push(
                here,
                None,
                MimeValidateRule::CompositeEncoding,
                MimeSeverity::Violation,
            );
        }

        let partial = media.is("message", "partial") || media.is("message", "external-body");

        if partial && encoding != MimeTransferEncoding::SevenBit {
            self.push(
                here,
                None,
                MimeValidateRule::PartialEncoding,
                MimeSeverity::Violation,
            );
        }

        if media.is_top("multipart") {
            let boundary = content_type.param("boundary");
            let valid = boundary.is_some_and(is_boundary);

            if !valid {
                self.push(
                    here,
                    None,
                    MimeValidateRule::Boundary,
                    MimeSeverity::Violation,
                );
            }
        }

        match encoding {
            MimeTransferEncoding::EightBit => self.output.transport.eight_bit = true,
            MimeTransferEncoding::Binary => self.output.transport.binary = true,
            _ => {}
        }

        match &entity.body {
            MimeBodyNode::Leaf(bytes) => self.leaf(here, bytes, &encoding),
            MimeBodyNode::Unparsed(unparsed) => self.lines(here, None, &unparsed.bytes, false),
            MimeBodyNode::Multipart(multipart) => {
                self.lines(here, None, &multipart.preamble, false);

                for part in &multipart.parts {
                    self.entity(&part.entity, false, nested, content_ids);
                }

                self.lines(here, None, &multipart.epilogue, false);
            }
            MimeBodyNode::Message(inner) => self.entity(inner, true, true, &mut Vec::new()),
        }
    }

    fn field(&mut self, entity: usize, field: &MimeHeaderField<'_>, nested: bool) {
        let name = field.name_str();
        let line = field.line();

        self.lines(entity, Some(&name), line, true);

        if !line.is_ascii() {
            match str::from_utf8(line) {
                Ok(_) => self.output.transport.utf8_fields = true,
                Err(_) => self.push(
                    entity,
                    Some(&name),
                    MimeValidateRule::Raw8bit,
                    MimeSeverity::Violation,
                ),
            }
        }

        let form = name
            .parse::<MimeHeaderKind>()
            .map_or(MimeHeaderForm::Raw, |kind| kind.spec().form);

        // NOTE: a date the reader cannot make out is already reported by the
        // model rules, so the syntax adds only what reads despite it.
        if form == MimeHeaderForm::Date {
            let readable = matches!(field.read(), MimeHeaderValue::Date(date) if date.is_valid());

            match date_syntax(field.value()) {
                DateSyntax::Current => {}
                DateSyntax::Obsolete => self.push(
                    entity,
                    Some(&name),
                    MimeValidateRule::Obsolete(form),
                    MimeSeverity::Advisory,
                ),
                DateSyntax::Invalid if readable => self.push(
                    entity,
                    Some(&name),
                    MimeValidateRule::Unreadable(form),
                    if nested {
                        MimeSeverity::Advisory
                    } else {
                        MimeSeverity::Violation
                    },
                ),
                DateSyntax::Invalid => {}
            }
        }

        if misplaced_words(field.value(), form) {
            self.push(
                entity,
                Some(&name),
                MimeValidateRule::EncodedWord,
                MimeSeverity::Violation,
            );
        }
    }

    /// Checks content against its declared transfer encoding (RFC 2045 2.7,
    /// 2.8, 6.7, 6.8), and what it needs of a transport.
    fn leaf(&mut self, entity: usize, bytes: &[u8], encoding: &MimeTransferEncoding) {
        let eight = !bytes.is_ascii();
        let lines = Lines::of(bytes, self.eol);

        if eight {
            self.output.transport.eight_bit = true;
        }

        if lines.long {
            self.output.transport.long_lines = true;
        }

        if lines.stray || bytes.contains(&0) {
            self.output.transport.binary = true;
        }

        match encoding {
            MimeTransferEncoding::SevenBit => {
                if eight || lines.long || lines.stray || bytes.contains(&0) {
                    self.push(
                        entity,
                        None,
                        MimeValidateRule::NotSevenBit,
                        MimeSeverity::Violation,
                    );
                }
            }
            MimeTransferEncoding::EightBit => {
                if lines.long || lines.stray || bytes.contains(&0) {
                    self.push(
                        entity,
                        None,
                        MimeValidateRule::NotEightBit,
                        MimeSeverity::Violation,
                    );
                }
            }
            MimeTransferEncoding::QuotedPrintable => {
                let mut out = Vec::new();
                let clean = quoted_printable::decode(SimdCpu::current(), bytes, &mut out);
                let long = bytes
                    .split(|byte| *byte == b'\n')
                    .any(|line| line.strip_suffix(b"\r").unwrap_or(line).len() > 76);

                if !clean || long {
                    self.push(
                        entity,
                        None,
                        MimeValidateRule::QuotedPrintable,
                        MimeSeverity::Violation,
                    );
                }
            }
            MimeTransferEncoding::Base64 => {
                let alphabet = bytes.iter().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b'+' | b'/' | b'=' | b'\r' | b'\n' | b' ' | b'\t')
                });

                if !alphabet {
                    self.push(
                        entity,
                        None,
                        MimeValidateRule::Base64,
                        MimeSeverity::Violation,
                    );
                }
            }
            MimeTransferEncoding::Binary | MimeTransferEncoding::Unknown(_) => {}
        }

        // NOTE: under `binary` long lines are a transport need (RFC 3030),
        // not a problem.
        if lines.long && *encoding != MimeTransferEncoding::Binary {
            self.push(
                entity,
                None,
                MimeValidateRule::LineTooLong,
                MimeSeverity::Violation,
            );
        }
    }

    /// Checks the lines of a header item or of preamble and epilogue bytes:
    /// 998 octets, and 78 characters for a field; stray CR, LF and NUL.
    fn lines(&mut self, entity: usize, field: Option<&str>, bytes: &[u8], header: bool) {
        let lines = Lines::of(bytes, self.eol);

        if lines.long {
            self.output.transport.long_lines = true;
            self.push(
                entity,
                field,
                MimeValidateRule::LineTooLong,
                MimeSeverity::Violation,
            );
        } else if header && lines.over_78 {
            self.push(
                entity,
                field,
                MimeValidateRule::LineLong,
                MimeSeverity::Advisory,
            );
        }

        if header && (lines.stray || bytes.contains(&0)) {
            self.push(
                entity,
                field,
                MimeValidateRule::StrayControl,
                MimeSeverity::Violation,
            );
        }
    }

    fn push(
        &mut self,
        entity: usize,
        field: Option<&str>,
        rule: MimeValidateRule,
        severity: MimeSeverity,
    ) {
        self.output.push(entity, field, rule, severity);
    }
}

/// What the lines of some bytes break.
struct Lines {
    long: bool,
    over_78: bool,
    stray: bool,
}

impl Lines {
    fn of(bytes: &[u8], eol: &[u8]) -> Self {
        let mut lines = Self {
            long: false,
            over_78: false,
            stray: false,
        };

        let mut pieces = bytes.split(|byte| *byte == b'\n').peekable();

        while let Some(piece) = pieces.next() {
            let last = pieces.peek().is_none();
            let line = match piece.strip_suffix(b"\r") {
                Some(line) if !last && eol == b"\r\n" => line,
                _ => {
                    if !last && eol == b"\r\n" {
                        lines.stray = true;
                    }

                    piece
                }
            };

            if line.contains(&b'\r') {
                lines.stray = true;
            }

            lines.long |= line.len() > 998;
            lines.over_78 |= line.len() > 78
                && str::from_utf8(line).map_or(true, |line| line.chars().count() > 78);
        }

        lines
    }
}

/// How a Date value stands against RFC 5322 3.3.
enum DateSyntax {
    /// The current syntax of RFC 5322 3.3.
    Current,
    /// The obsolete syntax of RFC 5322 4.3: a two- or three-digit year, a
    /// named or military zone, a comment or blanks inside.
    Obsolete,
    /// Neither: time fields not of two digits, a zone neither `+hhmm` nor
    /// named, a four-digit year before 1900, missing or stray parts.
    Invalid,
}

/// Reads a Date value against the RFC 5322 3.3 date-time and its RFC 5322
/// 4.3 obsolete forms, names compared ASCII case-insensitively.
fn date_syntax(raw: &[u8]) -> DateSyntax {
    const DAYS: [&[u8]; 7] = [b"mon", b"tue", b"wed", b"thu", b"fri", b"sat", b"sun"];
    const MONTHS: [&[u8]; 12] = [
        b"jan", b"feb", b"mar", b"apr", b"may", b"jun", b"jul", b"aug", b"sep", b"oct", b"nov",
        b"dec",
    ];
    const ZONES: [&[u8]; 10] = [
        b"ut", b"gmt", b"est", b"edt", b"cst", b"cdt", b"mst", b"mdt", b"pst", b"pdt",
    ];

    let mut date = DateCursor {
        raw,
        at: 0,
        obsolete: false,
    };
    let named =
        |word: &[u8], names: &[&[u8]]| names.iter().any(|name| word.eq_ignore_ascii_case(name));

    let parsed = (|| {
        date.cfws(false)?;

        if date.peek().is_some_and(|byte| byte.is_ascii_alphabetic()) {
            if !named(date.run(u8::is_ascii_alphabetic), &DAYS) {
                return None;
            }

            date.cfws(false)?;
            date.eat(b',')?;
            date.cfws(false)?;
        }

        if !(1..=2).contains(&date.run(u8::is_ascii_digit).len()) {
            return None;
        }

        date.cfws(true)?;

        if !named(date.run(u8::is_ascii_alphabetic), &MONTHS) {
            return None;
        }

        date.cfws(true)?;
        let year = date.run(u8::is_ascii_digit);

        match year.len() {
            2..=3 => date.obsolete = true,
            4.. if year.iter().fold(0u32, |n, digit| {
                n.saturating_mul(10).saturating_add(u32::from(digit - b'0'))
            }) >= 1900 => {}
            _ => return None,
        }

        date.cfws(true)?;
        date.digits(2)?;

        for field in 0..2 {
            let before = (date.at, date.obsolete);
            date.cfws(false)?;

            if date.peek() != Some(b':') {
                // NOTE: the seconds are optional, the minutes are not; the
                // blanks looked past belong to the zone.
                if field == 0 {
                    return None;
                }

                (date.at, date.obsolete) = before;
                break;
            }

            date.at += 1;
            date.cfws(false)?;
            date.digits(2)?;
        }

        date.cfws(true)?;

        match date.peek()? {
            b'+' | b'-' => {
                date.at += 1;
                let zone = date.run(u8::is_ascii_digit);

                // NOTE: hours then minutes, the minutes below 60.
                if zone.len() != 4 || zone[2] > b'5' {
                    return None;
                }
            }
            _ => {
                let zone = date.run(u8::is_ascii_alphabetic);
                let military = zone.len() == 1 && !zone.eq_ignore_ascii_case(b"j");

                if !(named(zone, &ZONES) || military) {
                    return None;
                }

                date.obsolete = true;
            }
        }

        // NOTE: a comment may follow the zone in the current syntax.
        let obsolete = date.obsolete;
        date.cfws(false)?;
        date.obsolete = obsolete;
        date.at.eq(&raw.len()).then_some(())
    })();

    match parsed {
        None => DateSyntax::Invalid,
        Some(()) if date.obsolete => DateSyntax::Obsolete,
        Some(()) => DateSyntax::Current,
    }
}

/// A cursor over a Date value, noting whether it met the obsolete syntax.
struct DateCursor<'r> {
    raw: &'r [u8],
    at: usize,
    obsolete: bool,
}

impl<'r> DateCursor<'r> {
    fn peek(&self) -> Option<u8> {
        self.raw.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> Option<()> {
        (self.peek() == Some(byte)).then(|| self.at += 1)
    }

    fn run(&mut self, keep: fn(&u8) -> bool) -> &'r [u8] {
        let start = self.at;

        while self.raw.get(self.at).is_some_and(keep) {
            self.at += 1;
        }

        &self.raw[start..self.at]
    }

    fn digits(&mut self, count: usize) -> Option<()> {
        (self.run(u8::is_ascii_digit).len() == count).then_some(())
    }

    /// Skips blanks, folds and comments, `None` on an unterminated comment;
    /// a comment is the obsolete syntax, and so is no blank where `needed`.
    fn cfws(&mut self, needed: bool) -> Option<()> {
        let start = self.at;

        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => self.at += 1,
                Some(b'(') => {
                    let mut depth = 0usize;

                    loop {
                        match self.peek()? {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            b'\\' => self.at += 1,
                            _ => {}
                        }

                        self.at += 1;

                        if depth == 0 {
                            break;
                        }
                    }

                    self.obsolete = true;
                }
                _ => break,
            }
        }

        if needed && self.at == start {
            self.obsolete = true;
        }

        Some(())
    }
}

/// Whether a value holds an encoded word RFC 2047 forbids: longer than 75
/// characters, or in a form where none may stand (inside a quoted string or
/// an angle address, or in a parameter value).
fn misplaced_words(raw: &[u8], form: MimeHeaderForm) -> bool {
    if !rfc2047::contains_word_start(raw) {
        return false;
    }

    let mut quoted = false;
    let mut angle = false;
    let mut params = false;
    let mut i = 0;

    while let Some(&byte) = raw.get(i) {
        match byte {
            b'\\' if quoted => {
                i += 2;
                continue;
            }
            b'"' => quoted = !quoted,
            b'<' if !quoted => angle = true,
            b'>' if !quoted => angle = false,
            b';' if !quoted => params = true,
            b'=' if raw.get(i + 1) == Some(&b'?') => {
                if let Some(end) = rfc2047::word_end(raw, i) {
                    let addresses = matches!(
                        form,
                        MimeHeaderForm::Addresses | MimeHeaderForm::GroupedAddresses
                    );
                    let structured = matches!(
                        form,
                        MimeHeaderForm::ContentType | MimeHeaderForm::Disposition
                    );

                    if end - i > 75 || (addresses && (quoted || angle)) || (structured && params) {
                        return true;
                    }

                    i = end;
                    continue;
                }
            }
            _ => {}
        }

        i += 1;
    }

    false
}
