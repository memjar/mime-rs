//! # Validator
//!
//! The strict half of "liberal in, strict out", as a runtime predicate:
//! [`Mime::check`] lists every problem a message has against its RFCs, and
//! [`Mime::validate`] turns a message with no violation into a
//! [`MimeValid`] proof, the only way to obtain one.
//!
//! Mail's RFCs speak in MUST and in SHOULD, and many real messages break a
//! SHOULD harmlessly, so every problem carries a [`MimeSeverity`]: a
//! violation (MUST, MUST NOT) withholds the proof, an advisory (SHOULD,
//! SHOULD NOT, and the obsolete syntax RFC 5322 4 says not to generate) does
//! not. A conformant message may carry extension fields and media types.
//!
//! The model check reads what the model states: the message fields, the
//! values of the known ones and the multipart structures. What only the
//! wire shows (line lengths, line endings, transfer encodings as written,
//! encoded words, boundaries, parser anomalies) is checked on the syntax
//! tree by `MimeCst::check`, which also computes [`MimeTransport`], what
//! the message needs of a transport.

use alloc::{string::String, vec::Vec};
use core::{error, fmt, ops::Deref};

use crate::{
    header::{MimeHeaderKind, MimeHeaderName, spec::MimeHeaderForm},
    mime::{Mime, MimeBody},
    value::{MimeHeaderValue, content_type::MimeContentType, message_id::MimeMessageId},
};

/// How much a problem weighs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MimeSeverity {
    /// A SHOULD, a SHOULD NOT, or obsolete syntax: the message still
    /// earns its proof.
    Advisory,
    /// A MUST or a MUST NOT: the message does not earn its proof.
    Violation,
}

/// A rule a message breaks.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MimeValidateRule {
    /// A field a message must carry, or should, is missing.
    Missing(MimeHeaderKind),
    /// A field appears more often than allowed.
    Repeated(MimeHeaderKind),
    /// A From of several mailboxes with no Sender.
    SenderRequired,
    /// Resent- fields without their Resent-Date or Resent-From.
    ResentIncomplete,
    /// A trace field below the fields it should be prepended to.
    TraceOrder,
    /// A known field whose value does not read in its form.
    Unreadable(MimeHeaderForm),
    /// A field written in the obsolete syntax.
    Obsolete(MimeHeaderForm),
    /// A mailbox with no address, or an address that is not a local part,
    /// an `@` and an unquoted domain.
    Address,
    /// A message identifier that is not `id-left "@" id-right`.
    MessageId,
    /// A line longer than 998 octets.
    LineTooLong,
    /// A field line longer than 78 characters.
    LineLong,
    /// A CR or an LF not part of a line ending, or a NUL.
    StrayControl,
    /// Lines ending in LF alone rather than CRLF.
    LfLineEndings,
    /// 8-bit bytes in a field that are not UTF-8.
    Raw8bit,
    /// An encoded word longer than 75 characters, or where RFC 2047 5
    /// forbids one: inside a quoted string, an address or a parameter.
    EncodedWord,
    /// Content- fields at the top of a message with no MIME-Version.
    MimeVersion,
    /// A multipart with no boundary, or one RFC 2046 does not allow.
    Boundary,
    /// A multipart or a nested message under an encoding other than
    /// `7bit`, `8bit` or `binary`.
    CompositeEncoding,
    /// A `message/partial` or `message/external-body` not under `7bit`.
    PartialEncoding,
    /// Content declared `7bit` holding 8-bit bytes or long lines.
    NotSevenBit,
    /// Content declared `8bit` holding a NUL, a stray CR or LF, or long
    /// lines.
    NotEightBit,
    /// Quoted-printable with an invalid sequence or a line over 76
    /// characters.
    QuotedPrintable,
    /// Base64 holding bytes outside its alphabet.
    Base64,
    /// A `multipart/related` with no `type`.
    RelatedType,
    /// A `multipart/signed` without `protocol` and `micalg`, or without
    /// exactly two parts.
    Signed,
    /// A `multipart/encrypted` without `protocol`, or without exactly two
    /// parts.
    Encrypted,
    /// A `multipart/report` with no `report-type`.
    ReportType,
    /// A Content-ID used twice in one message.
    ContentIdRepeated,
    /// Something the parser worked around.
    #[cfg(feature = "parser")]
    Anomaly(crate::tree::anomaly::MimeAnomaly),
}

impl MimeValidateRule {
    /// The RFC section stating the rule.
    pub fn rfc(&self) -> &'static str {
        match self {
            Self::Missing(kind) | Self::Repeated(kind) => kind.spec().rfc,
            Self::SenderRequired => "RFC 5322 3.6.2",
            Self::ResentIncomplete => "RFC 5322 3.6.6",
            Self::TraceOrder => "RFC 5322 3.6",
            Self::Unreadable(form) | Self::Obsolete(form) => match form {
                MimeHeaderForm::Date => "RFC 5322 3.3",
                MimeHeaderForm::Addresses | MimeHeaderForm::GroupedAddresses => "RFC 5322 3.4",
                MimeHeaderForm::MessageIds => "RFC 5322 3.6.4",
                MimeHeaderForm::Urls => "RFC 2369 2",
                MimeHeaderForm::ContentType => "RFC 2045 5.1",
                MimeHeaderForm::Disposition => "RFC 2183 2",
                MimeHeaderForm::TransferEncoding => "RFC 2045 6.1",
                _ => "RFC 5322 3.6",
            },
            Self::Address => "RFC 5322 3.4.1",
            Self::MessageId => "RFC 5322 3.6.4",
            Self::LineTooLong | Self::LineLong => "RFC 5322 2.1.1",
            Self::StrayControl => "RFC 5322 2.3",
            Self::LfLineEndings => "RFC 5322 2.1",
            Self::Raw8bit => "RFC 6532 3",
            Self::EncodedWord => "RFC 2047 5",
            Self::MimeVersion => "RFC 2045 4",
            Self::Boundary => "RFC 2046 5.1.1",
            Self::CompositeEncoding => "RFC 2045 6.4",
            Self::PartialEncoding => "RFC 2046 5.2.2",
            Self::NotSevenBit => "RFC 2045 2.7",
            Self::NotEightBit => "RFC 2045 2.8",
            Self::QuotedPrintable => "RFC 2045 6.7",
            Self::Base64 => "RFC 2045 6.8",
            Self::RelatedType => "RFC 2387 3.1",
            Self::Signed => "RFC 1847 2.1",
            Self::Encrypted => "RFC 1847 2.2",
            Self::ReportType => "RFC 6522 3",
            Self::ContentIdRepeated => "RFC 2045 7",
            #[cfg(feature = "parser")]
            Self::Anomaly(_) => "RFC 2046 5.1.1",
        }
    }
}

impl fmt::Display for MimeValidateRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(kind) => write!(f, "Missing {kind} field"),
            Self::Repeated(kind) => write!(f, "Repeated {kind} field"),
            Self::SenderRequired => f.write_str("From of several mailboxes without Sender"),
            Self::ResentIncomplete => {
                f.write_str("Resent block without Resent-Date or Resent-From")
            }
            Self::TraceOrder => f.write_str("Trace field below other fields"),
            Self::Unreadable(form) => write!(f, "Field does not read {form}"),
            Self::Obsolete(form) => write!(f, "Field written in obsolete syntax {form}"),
            Self::Address => f.write_str("Mailbox without a full address"),
            Self::MessageId => f.write_str("Invalid message identifier"),
            Self::LineTooLong => f.write_str("Line longer than 998 octets"),
            Self::LineLong => f.write_str("Field line longer than 78 characters"),
            Self::StrayControl => f.write_str("Stray CR, LF or NUL"),
            Self::LfLineEndings => f.write_str("Lines ending in LF alone"),
            Self::Raw8bit => f.write_str("8-bit bytes in a field that are not UTF-8"),
            Self::EncodedWord => f.write_str("Encoded word too long or misplaced"),
            Self::MimeVersion => f.write_str("Content fields without MIME-Version"),
            Self::Boundary => f.write_str("Missing or invalid boundary"),
            Self::CompositeEncoding => f.write_str("Composite type under a non-identity encoding"),
            Self::PartialEncoding => f.write_str("Partial or external message not under 7bit"),
            Self::NotSevenBit => f.write_str("Content declared 7bit is not"),
            Self::NotEightBit => f.write_str("Content declared 8bit is not"),
            Self::QuotedPrintable => f.write_str("Malformed quoted-printable"),
            Self::Base64 => f.write_str("Malformed base64"),
            Self::RelatedType => f.write_str("Related multipart without type"),
            Self::Signed => f.write_str("Malformed signed multipart"),
            Self::Encrypted => f.write_str("Malformed encrypted multipart"),
            Self::ReportType => f.write_str("Report multipart without report-type"),
            Self::ContentIdRepeated => f.write_str("Repeated Content-ID"),
            #[cfg(feature = "parser")]
            Self::Anomaly(anomaly) => write!(f, "Parser recovered from {anomaly}"),
        }
    }
}

/// One problem a message has.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MimeValidateError {
    /// The entity, numbered in document order, the root first.
    pub entity: usize,
    /// The field, where the problem is one field's.
    pub field: Option<String>,
    /// The rule broken.
    pub rule: MimeValidateRule,
    /// How much it weighs.
    pub severity: MimeSeverity,
}

impl fmt::Display for MimeValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}), entity {}",
            self.rule,
            self.rule.rfc(),
            self.entity
        )?;

        if let Some(field) = &self.field {
            write!(f, ", field {field}")?;
        }

        Ok(())
    }
}

impl error::Error for MimeValidateError {}

/// What a message needs of a transport, computed beside its problems and
/// not one itself: whether a transport offers it is the caller's to know.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeTransport {
    /// 8-bit bytes in bodies (RFC 6152 8BITMIME).
    pub eight_bit: bool,
    /// Binary bodies: NUL, stray CR or LF, or unlimited lines (RFC 3030
    /// BINARYMIME).
    pub binary: bool,
    /// UTF-8 in fields (RFC 6531 SMTPUTF8).
    pub utf8_fields: bool,
    /// Lines over 998 octets.
    pub long_lines: bool,
}

/// Every problem a message has, and what it needs of a transport.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeValidateOutput {
    /// The problems, violations and advisories, in document order.
    pub problems: Vec<MimeValidateError>,
    /// What the message needs of a transport.
    pub transport: MimeTransport,
}

impl MimeValidateOutput {
    /// Whether no problem is a violation.
    pub fn is_valid(&self) -> bool {
        !self
            .problems
            .iter()
            .any(|problem| problem.severity == MimeSeverity::Violation)
    }

    /// The violations alone.
    pub fn violations(&self) -> impl Iterator<Item = &MimeValidateError> {
        self.problems
            .iter()
            .filter(|problem| problem.severity == MimeSeverity::Violation)
    }

    pub(crate) fn push(
        &mut self,
        entity: usize,
        field: Option<&str>,
        rule: MimeValidateRule,
        severity: MimeSeverity,
    ) {
        self.problems.push(MimeValidateError {
            entity,
            field: field.map(String::from),
            rule,
            severity,
        });
    }
}

/// A value that passed validation, with what the validation found short
/// of a violation.
///
/// The only way to mint one is a validation ([`Mime::validate`] or
/// `MimeCst::validate`), so holding a `MimeValid<T>` is proof the check
/// passed. It derefs for reads and yields the value with
/// [`into_inner`](Self::into_inner).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimeValid<T> {
    inner: T,
    output: MimeValidateOutput,
}

impl<T> MimeValid<T> {
    pub(crate) fn new(inner: T, output: MimeValidateOutput) -> Self {
        Self { inner, output }
    }

    /// Takes the validated value back out.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// The advisories the validation found, and the transport needs.
    pub fn output(&self) -> &MimeValidateOutput {
        &self.output
    }
}

impl<T> Deref for MimeValid<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl Mime {
    /// Every problem the model states: the message fields, the values of
    /// the known ones and the multipart structures. What only the wire
    /// shows is `MimeCst::check`'s.
    pub fn check(&self) -> MimeValidateOutput {
        let mut output = MimeValidateOutput::default();
        let mut index = 0;
        let mut content_ids = Vec::new();

        check_model(self, true, false, &mut index, &mut content_ids, &mut output);
        output
    }

    /// The message as a proof of validity, or every problem it has when one
    /// is a violation.
    pub fn validate(self) -> Result<MimeValid<Mime>, MimeValidateOutput> {
        let output = self.check();

        if output.is_valid() {
            Ok(MimeValid::new(self, output))
        } else {
            Err(output)
        }
    }
}

impl TryFrom<Mime> for MimeValid<Mime> {
    type Error = MimeValidateOutput;

    fn try_from(mime: Mime) -> Result<Self, Self::Error> {
        mime.validate()
    }
}

fn check_model(
    mime: &Mime,
    top: bool,
    nested: bool,
    index: &mut usize,
    content_ids: &mut Vec<String>,
    output: &mut MimeValidateOutput,
) {
    let here = *index;
    *index += 1;

    let fields: Vec<(&MimeHeaderName, &MimeHeaderValue)> = mime
        .headers
        .iter()
        .map(|header| (&header.name, &header.value))
        .collect();

    check_fields(here, &fields, top, nested, output);
    check_content_ids(here, &fields, content_ids, output);

    let content_type = mime.content_type();
    let parts = match &mime.body {
        MimeBody::Multipart(parts) => Some(parts.len()),
        _ => None,
    };

    check_structure(here, &content_type, parts, nested, output);

    match &mime.body {
        MimeBody::Multipart(parts) => {
            for part in parts {
                check_model(part, false, nested, index, content_ids, output);
            }
        }
        MimeBody::Message(inner) => {
            check_model(inner, true, true, index, &mut Vec::new(), output);
        }
        MimeBody::Text(_) | MimeBody::Binary(_) => {}
    }
}

/// Whether an address reads as an RFC 5322 3.4.1 addr-spec: a local part,
/// an `@`, and a domain that is no quoted string.
fn is_addr_spec(address: &str) -> bool {
    address.rsplit_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && !domain.is_empty() && !domain.contains('"')
    })
}

/// The message field rules (RFC 5322 3.6) at the top of a message, as
/// advisories for a nested one, and the value rules of every known field.
pub(crate) fn check_fields(
    entity: usize,
    fields: &[(&MimeHeaderName, &MimeHeaderValue)],
    top: bool,
    nested: bool,
    output: &mut MimeValidateOutput,
) {
    let weight = if nested {
        MimeSeverity::Advisory
    } else {
        MimeSeverity::Violation
    };

    for (name, value) in fields {
        let Some(kind) = name.kind() else {
            continue;
        };

        let field = Some(name.as_str());
        let form = kind.spec().form;

        match value {
            MimeHeaderValue::Raw(_) if form != MimeHeaderForm::Raw => {
                output.push(entity, field, MimeValidateRule::Unreadable(form), weight);
            }
            MimeHeaderValue::Addresses(list) => {
                // NOTE: Return-Path alone may be the null path, `<>`.
                let path = kind == MimeHeaderKind::ReturnPath;
                let bare = list
                    .mailboxes()
                    .any(|mailbox| match mailbox.address.as_deref() {
                        Some(address) => !is_addr_spec(address) && !(path && address.is_empty()),
                        None => !path,
                    });

                // NOTE: From and Sender name at least one mailbox (RFC 5322
                // 3.6.2), a group of none included.
                let empty = matches!(kind, MimeHeaderKind::From | MimeHeaderKind::Sender)
                    && list.mailboxes().next().is_none();

                if bare || empty {
                    output.push(entity, field, MimeValidateRule::Address, weight);
                }
            }
            MimeHeaderValue::MessageIds(ids) => {
                if ids.iter().any(|id| MimeMessageId::parse(id).is_err()) {
                    output.push(entity, field, MimeValidateRule::MessageId, weight);
                }
            }
            MimeHeaderValue::Date(date) if !date.is_valid() => {
                output.push(entity, field, MimeValidateRule::Unreadable(form), weight);
            }
            _ => {}
        }
    }

    if !top {
        return;
    }

    let count = |kind: MimeHeaderKind| {
        fields
            .iter()
            .filter(|(name, _)| name.kind() == Some(kind))
            .count()
    };

    for kind in [MimeHeaderKind::Date, MimeHeaderKind::From] {
        if count(kind) == 0 {
            output.push(entity, None, MimeValidateRule::Missing(kind), weight);
        }
    }

    if count(MimeHeaderKind::MessageId) == 0 {
        output.push(
            entity,
            None,
            MimeValidateRule::Missing(MimeHeaderKind::MessageId),
            MimeSeverity::Advisory,
        );
    }

    for kind in MimeHeaderKind::ALL {
        let spec = kind.spec();

        if !spec.part && spec.max.is_some_and(|max| count(*kind) > usize::from(max)) {
            output.push(
                entity,
                Some(&**kind),
                MimeValidateRule::Repeated(*kind),
                weight,
            );
        }
    }

    let several = fields.iter().any(|(name, value)| {
        name.kind() == Some(MimeHeaderKind::From)
            && value
                .as_addresses()
                .is_some_and(|list| list.mailboxes().count() > 1)
    });

    if several && count(MimeHeaderKind::Sender) == 0 {
        output.push(entity, None, MimeValidateRule::SenderRequired, weight);
    }

    let resent = fields.iter().any(|(name, _)| {
        name.as_str()
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("resent-"))
    });

    if resent && (count(MimeHeaderKind::ResentDate) == 0 || count(MimeHeaderKind::ResentFrom) == 0)
    {
        output.push(entity, None, MimeValidateRule::ResentIncomplete, weight);
    }

    // NOTE: trace fields are prepended (RFC 5322 3.6), so one below a field
    // that is neither trace nor Resent- was not.
    let trace = |name: &MimeHeaderName| {
        matches!(
            name.kind(),
            Some(MimeHeaderKind::Received | MimeHeaderKind::ReturnPath)
        )
    };
    let first_other = fields.iter().position(|(name, _)| {
        !trace(name)
            && !name
                .as_str()
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("resent-"))
    });

    if let Some(first_other) = first_other
        && fields[first_other..].iter().any(|(name, _)| trace(name))
    {
        output.push(
            entity,
            None,
            MimeValidateRule::TraceOrder,
            MimeSeverity::Advisory,
        );
    }
}

/// The Content-ID uniqueness rule (RFC 2045 7) across one message.
pub(crate) fn check_content_ids(
    entity: usize,
    fields: &[(&MimeHeaderName, &MimeHeaderValue)],
    seen: &mut Vec<String>,
    output: &mut MimeValidateOutput,
) {
    for (name, value) in fields {
        if name.kind() != Some(MimeHeaderKind::ContentId) {
            continue;
        }

        for id in value.as_message_ids().unwrap_or_default() {
            if seen.iter().any(|seen| seen == id) {
                output.push(
                    entity,
                    Some(name.as_str()),
                    MimeValidateRule::ContentIdRepeated,
                    MimeSeverity::Violation,
                );
            } else {
                seen.push(id.clone());
            }
        }
    }
}

/// The rules of the multipart structures RFC 1847, RFC 2387 and RFC 6522
/// define.
pub(crate) fn check_structure(
    entity: usize,
    content_type: &MimeContentType,
    parts: Option<usize>,
    nested: bool,
    output: &mut MimeValidateOutput,
) {
    let weight = if nested {
        MimeSeverity::Advisory
    } else {
        MimeSeverity::Violation
    };

    let has = |name: &str| content_type.param(name).is_some();

    if content_type.is("multipart", "related") && !has("type") {
        output.push(entity, None, MimeValidateRule::RelatedType, weight);
    }

    if content_type.is("multipart", "signed")
        && (!has("protocol") || !has("micalg") || parts.is_some_and(|parts| parts != 2))
    {
        output.push(entity, None, MimeValidateRule::Signed, weight);
    }

    if content_type.is("multipart", "encrypted")
        && (!has("protocol") || parts.is_some_and(|parts| parts != 2))
    {
        output.push(entity, None, MimeValidateRule::Encrypted, weight);
    }

    if content_type.is("multipart", "report") && !has("report-type") {
        output.push(entity, None, MimeValidateRule::ReportType, weight);
    }
}
