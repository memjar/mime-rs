//! # Building
//!
//! Writing a decoded [`Mime`] as canonical bytes: every field encoded and
//! folded by [`field`] and [`fold`], every body given its transfer encoding,
//! its charset and its boundary by [`entity`]. The output depends on the
//! model and the [`MimeBuildOptions`] alone: no clock, no randomness, no
//! hostname, so the same model always builds the same bytes.
//!
//! Writing needs no parser; [`MimeBuilder`](crate::builder::MimeBuilder)
//! composes a model to write, and the syntax tree reads the bytes back.

pub mod entity;
pub mod field;
pub mod fold;

use alloc::{string::String, vec::Vec};
use core::{error, fmt};

use simd_cpu::SimdCpu;

use crate::{
    header::{MimeHeaderKind, spec::MimeHeaderForm},
    mime::Mime,
};

/// The line ending a build writes.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MimeLineEnding {
    /// CRLF, as RFC 5322 2.1 and every transport require.
    #[default]
    Crlf,
    /// LF alone, for a local store that keeps messages that way.
    Lf,
}

impl MimeLineEnding {
    /// The line ending's bytes.
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Crlf => b"\r\n",
            Self::Lf => b"\n",
        }
    }
}

/// What a build may rely on from the transport, and how it writes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeBuildOptions {
    /// The line ending written.
    pub line_ending: MimeLineEnding,
    /// Whether text may be written `8bit` (RFC 6152 8BITMIME).
    pub allow_8bit: bool,
    /// Whether any leaf may be written `binary` (RFC 3030 BINARYMIME).
    pub allow_binary: bool,
    /// Whether header fields may carry UTF-8 instead of encoded words (RFC
    /// 6532, SMTPUTF8 transports).
    pub allow_utf8_headers: bool,
    /// Whether `text/plain` without a `format` parameter is written as
    /// RFC 3676 flowed text.
    pub flowed: bool,
    /// Whether a part whose Content-Disposition carries a non-ASCII
    /// `filename` also gets an encoded `name` parameter on its
    /// Content-Type, for readers that know neither RFC 2231 nor RFC 2183.
    pub legacy_name: bool,
    /// The CPU features bodies are encoded under, or none, the default, for
    /// [`SimdCpu::current`]: the token the application installed, or the
    /// build target's baseline.
    pub cpu: Option<SimdCpu>,
}

/// Why a message could not be built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MimeBuildError {
    /// A known field was given a value of a form other than its own.
    Form {
        /// The field.
        kind: MimeHeaderKind,
        /// The form its value has.
        form: MimeHeaderForm,
    },
    /// A second instance of a field RFC 5322 3.6 allows once.
    Repeated(MimeHeaderKind),
    /// A field the builder writes itself (Content-Type,
    /// Content-Transfer-Encoding, MIME-Version).
    Reserved(MimeHeaderKind),
    /// A message identifier that is not RFC 5322 3.6.4 `id-left "@"
    /// id-right`.
    MessageId(String),
    /// A field name holding a byte RFC 5322 2.2 does not allow, or none.
    FieldName(String),
    /// A raw value holding a line break no blank follows, which would end
    /// its field early.
    RawValue(String),
    /// A token longer than a line may ever be (RFC 5322 2.1.1), which no
    /// fold can split.
    LineTooLong,
    /// A token holding a line break, which would end its field early.
    LineBreak,
    /// A multipart body under a content type that is not `multipart`, or a
    /// nested message under one that is not `message`.
    Structure,
    /// An inline resource with no Content-ID to be referenced by.
    InlineWithoutId,
}

impl fmt::Display for MimeBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Form { kind, form } => write!(f, "Field {kind} cannot take a value {form}"),
            Self::Repeated(kind) => write!(f, "Field {kind} may appear only once"),
            Self::Reserved(kind) => write!(f, "Field {kind} is written by the builder"),
            Self::MessageId(id) => write!(f, "Invalid message identifier {id:?}"),
            Self::FieldName(name) => write!(f, "Invalid field name {name:?}"),
            Self::RawValue(name) => write!(f, "Raw value of field {name} ends the field early"),
            Self::LineTooLong => f.write_str("Token longer than 998 octets"),
            Self::LineBreak => f.write_str("Token holding a line break"),
            Self::Structure => f.write_str("Body does not match its content type"),
            Self::InlineWithoutId => f.write_str("Inline resource without Content-ID"),
        }
    }
}

impl error::Error for MimeBuildError {}

impl Mime {
    /// Writes the message as bytes.
    pub fn to_bytes(&self, opts: &MimeBuildOptions) -> Result<Vec<u8>, MimeBuildError> {
        let mut out = Vec::with_capacity(entity::estimate(self));
        let mut boundaries = entity::Boundaries::new(&[]);
        entity::write(self, opts, true, &mut boundaries, &mut out)?;
        Ok(out)
    }
}
