//! # Entity writer
//!
//! Writing one entity of a decoded [`Mime`]: its header fields in model
//! order, then its body, with the structural fields made to state what the
//! body is written as.
//!
//! - A text leaf is written in its declared charset when the build can
//!   encode it there, else in UTF-8 with `charset=utf-8`; its line breaks
//!   are written as the build's line ending.
//! - A leaf keeps the transfer encoding its model declares when that
//!   encoding is valid for the content and the options; otherwise a text
//!   leaf is `7bit`, else `8bit` when allowed, else the shorter of
//!   quoted-printable and base64, and any other leaf is `binary` when
//!   allowed, else base64 (quoted-printable without the `base64` feature).
//! - A multipart keeps its declared boundary unless it occurs in its
//!   content, and is otherwise given `=_mime-rs_` and a counter, the next
//!   value tried until one does not occur: no randomness.
//! - A multipart or a nested message takes no transfer encoding but an
//!   identity one (RFC 2045 6.4), and a nested message whose fields carry
//!   UTF-8 is `message/global` (RFC 6532 3.7).
//! - The root gets `MIME-Version: 1.0` before its first Content- field when
//!   it has one and no MIME-Version.

use alloc::{borrow::Cow, collections::BTreeSet, format, string::String, vec::Vec};
use core::{iter, str};

use simd_cpu::SimdCpu;

use crate::{
    build::{MimeBuildError, MimeBuildOptions, field, fold::Folder},
    codec::{charset, flowed, quoted_printable},
    header::{MimeHeader, MimeHeaderKind, MimeHeaderName},
    mime::{Mime, MimeBody},
    value::{
        MimeHeaderValue,
        content_type::{MimeContentType, MimeParam},
        transfer_encoding::MimeTransferEncoding,
    },
};

/// Whether the build carries a base64 encoder.
const BASE64: bool = cfg!(feature = "base64");

/// The width flowed text is wrapped at (RFC 3676 4.2).
const FLOWED_WIDTH: usize = 72;

/// The longest boundary RFC 2046 5.1.1 allows.
const BOUNDARY_MAX: usize = 70;

/// What every generated boundary starts with, before its counter.
const PREFIX: &str = "=_mime-rs_";

/// Writes one entity, its header block then its body.
pub(crate) fn write(
    mime: &Mime,
    opts: &MimeBuildOptions,
    root: bool,
    boundaries: &mut Boundaries<'_>,
    out: &mut Vec<u8>,
) -> Result<(), MimeBuildError> {
    let eol = opts.line_ending.as_bytes();
    let declared_type = declared_content_type(mime);
    let declared_encoding = declared_transfer_encoding(mime);

    let body = match &mime.body {
        MimeBody::Text(text) => text_leaf(
            text,
            declared_type,
            declared_encoding,
            opts,
            boundaries.taken,
        )?,
        MimeBody::Binary(bytes) => binary_leaf(
            bytes,
            declared_type,
            declared_encoding,
            opts,
            boundaries.taken,
        ),
        MimeBody::Multipart(parts) => {
            let mut content_type = multipart_type(declared_type)?;
            let declared = content_type.param("boundary").filter(|b| is_boundary(b));
            let digest = content_type.is("multipart", "digest");

            // NOTE: a generated boundary cannot occur in base64 or
            // quoted-printable output, nor in another generated one, so
            // unless some content written as is may hold its prefix, the
            // parts are written straight into the output.
            if declared.is_none()
                && boundaries.regular()
                && !parts.iter().any(|part| tainted(part, opts))
            {
                let boundary = boundaries.next();
                set_param(&mut content_type, "boundary", &boundary);

                let body = Body {
                    bytes: Cow::Borrowed(&[]),
                    content_type: Some(content_type),
                    encoding: declared_encoding.filter(|e| e.is_identity()).cloned(),
                };

                write_header(mime, &body, opts, root, out)?;
                out.extend_from_slice(eol);

                for (index, part) in parts.iter().enumerate() {
                    delimiter(&boundary, index == 0, eol, out);
                    write(&explicit(part, digest), opts, false, boundaries, out)?;
                }

                close(&boundary, parts.is_empty(), eol, out);
                return Ok(());
            }

            multipart(parts, content_type, declared_encoding, opts, boundaries)?
        }
        MimeBody::Message(nested) => {
            message(nested, declared_type, declared_encoding, opts, boundaries)?
        }
    };

    write_header(mime, &body, opts, root, out)?;
    out.extend_from_slice(eol);

    match &body.encoding {
        #[cfg(feature = "base64")]
        Some(MimeTransferEncoding::Base64) => crate::codec::base64::encode(
            opts.cpu.unwrap_or_else(SimdCpu::current),
            &body.bytes,
            eol,
            out,
        ),
        _ => out.extend_from_slice(&body.bytes),
    }

    Ok(())
}

/// What an entity's body is written as, and the structural fields stating
/// it; under base64, its bytes are encoded as they are written out.
struct Body<'m> {
    bytes: Cow<'m, [u8]>,
    content_type: Option<MimeContentType>,
    encoding: Option<MimeTransferEncoding>,
}

fn text_leaf<'m>(
    text: &'m str,
    declared: Option<&MimeContentType>,
    declared_encoding: Option<&MimeTransferEncoding>,
    opts: &MimeBuildOptions,
    taken: &[String],
) -> Result<Body<'m>, MimeBuildError> {
    let eol = opts.line_ending.as_bytes();
    let mut content_type = declared.cloned();

    let plain = content_type
        .as_ref()
        .is_none_or(|content_type| content_type.is("text", "plain"));
    let format = content_type
        .as_ref()
        .and_then(|content_type| content_type.param("format"))
        .map(String::from);
    let flowed = plain
        && match &format {
            Some(format) => format.eq_ignore_ascii_case("flowed"),
            None => opts.flowed,
        };

    // NOTE: a leading U+FEFF reads back as a byte order mark, not as text.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    // NOTE: line breaks are settled before wrapping, since a CR dropped
    // after it could leave a line ending in the space of a soft break.
    let text: Cow<'m, str> = match line_breaks(Cow::Borrowed(text.as_bytes()), eol) {
        Cow::Borrowed(_) => Cow::Borrowed(text),
        Cow::Owned(bytes) => Cow::Owned(
            String::from_utf8(bytes)
                .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned()),
        ),
    };

    let text: Cow<'m, str> = if flowed {
        let content_type = content_type.get_or_insert_with(text_plain);
        set_param(content_type, "format", "flowed");
        content_type
            .params
            .retain(|param| !param.name.eq_ignore_ascii_case("delsp"));
        let eol = str::from_utf8(eol).unwrap_or("\r\n");
        Cow::Owned(flowed::encode(&text, FLOWED_WIDTH, eol))
    } else {
        text
    };

    let label = content_type
        .as_ref()
        .and_then(|content_type| content_type.param("charset"))
        .map(String::from);

    // NOTE: a legacy charset is kept when the build can write the text in
    // it; anything else is written as UTF-8 and labelled so.
    let legacy = label
        .as_deref()
        .filter(|label| !charset::is_utf8_label(label))
        .and_then(|label| charset::encode(label, &text).map(Cow::into_owned));
    let labelled = legacy.is_some()
        || label
            .as_deref()
            .is_some_and(|label| charset::encode(label, &text).is_some());

    let bytes: Cow<'m, [u8]> = match (legacy, text) {
        (Some(bytes), _) => Cow::Owned(bytes),
        (None, Cow::Borrowed(text)) => Cow::Borrowed(text.as_bytes()),
        (None, Cow::Owned(text)) => Cow::Owned(text.into_bytes()),
    };

    let is_text = content_type
        .as_ref()
        .is_none_or(|content_type| content_type.is_top("text"));

    if is_text && !labelled && (label.is_some() || !bytes.is_ascii()) {
        let content_type = content_type.get_or_insert_with(text_plain);
        set_param(content_type, "charset", "utf-8");
    }

    let bytes = line_breaks(bytes, eol);
    // NOTE: text under a multipart type is a damaged multipart, written in
    // base64 as a binary one is.
    let damaged = content_type
        .as_ref()
        .is_some_and(|content_type| content_type.is_top("multipart"));

    let (bytes, encoding) = if damaged && BASE64 {
        (bytes, Some(MimeTransferEncoding::Base64))
    } else {
        leaf(bytes, true, declared_encoding, opts, taken)
    };

    Ok(Body {
        bytes,
        content_type,
        encoding,
    })
}

fn binary_leaf<'m>(
    bytes: &'m [u8],
    declared: Option<&MimeContentType>,
    declared_encoding: Option<&MimeTransferEncoding>,
    opts: &MimeBuildOptions,
    taken: &[String],
) -> Body<'m> {
    let content_type = Some(declared.cloned().unwrap_or_else(|| MimeContentType {
        top: String::from("application"),
        sub: String::from("octet-stream"),
        params: Vec::new(),
    }));

    // NOTE: a leaf under a multipart type, as a damaged multipart decodes,
    // is written in base64, which never holds the `--` a reader would split
    // it at.
    let damaged = content_type
        .as_ref()
        .is_some_and(|content_type| content_type.is_top("multipart"));

    // NOTE: a message takes no encoding but an identity one (RFC 2045 6.4),
    // so it is written as is whenever one can carry it.
    let message = content_type.as_ref().is_some_and(|content_type| {
        content_type.is("message", "rfc822") || content_type.is("message", "global")
    });
    let identity = [
        MimeTransferEncoding::SevenBit,
        MimeTransferEncoding::EightBit,
        MimeTransferEncoding::Binary,
    ]
    .into_iter()
    .find(|encoding| message && valid(encoding, bytes, opts, taken));

    let (bytes, encoding) = if damaged && BASE64 {
        (Cow::Borrowed(bytes), Some(MimeTransferEncoding::Base64))
    } else if let Some(encoding) = identity {
        let stated = (encoding != MimeTransferEncoding::SevenBit).then_some(encoding);
        (Cow::Borrowed(bytes), stated)
    } else {
        leaf(Cow::Borrowed(bytes), false, declared_encoding, opts, taken)
    };

    Body {
        bytes,
        content_type,
        encoding,
    }
}

/// Writes a multipart's parts apart, then picks a boundary none of them
/// holds: the declared one when it qualifies, else the next generated one.
fn multipart<'m>(
    parts: &[Mime],
    mut content_type: MimeContentType,
    declared_encoding: Option<&MimeTransferEncoding>,
    opts: &MimeBuildOptions,
    boundaries: &mut Boundaries<'_>,
) -> Result<Body<'m>, MimeBuildError> {
    let eol = opts.line_ending.as_bytes();
    let digest = content_type.is("multipart", "digest");

    let mut written = Vec::with_capacity(parts.len());

    for part in parts {
        let mut bytes = Vec::new();
        write(&explicit(part, digest), opts, false, boundaries, &mut bytes)?;
        written.push(bytes);
    }

    let collides = |boundary: &str| {
        written
            .iter()
            .any(|bytes| contains(bytes, boundary.as_bytes()))
    };

    let boundary = match content_type.param("boundary") {
        Some(boundary)
            if is_boundary(boundary) && !collides(boundary) && boundaries.allows(boundary) =>
        {
            String::from(boundary)
        }
        _ => {
            let contents: Vec<&[u8]> = written.iter().map(Vec::as_slice).collect();
            boundaries.fresh(&contents)
        }
    };

    set_param(&mut content_type, "boundary", &boundary);

    let mut bytes = Vec::with_capacity(written.iter().map(|part| part.len() + 80).sum());

    for (index, part) in written.iter().enumerate() {
        delimiter(&boundary, index == 0, eol, &mut bytes);
        bytes.extend_from_slice(part);
    }

    close(&boundary, written.is_empty(), eol, &mut bytes);

    Ok(Body {
        bytes: Cow::Owned(bytes),
        content_type: Some(content_type),
        encoding: declared_encoding
            .filter(|encoding| encoding.is_identity())
            .cloned(),
    })
}

fn message<'m>(
    nested: &Mime,
    declared: Option<&MimeContentType>,
    declared_encoding: Option<&MimeTransferEncoding>,
    opts: &MimeBuildOptions,
    boundaries: &mut Boundaries<'_>,
) -> Result<Body<'m>, MimeBuildError> {
    let mut content_type = declared.cloned().unwrap_or_else(|| MimeContentType {
        top: String::from("message"),
        sub: String::from("rfc822"),
        params: Vec::new(),
    });

    if !content_type.is_top("message") {
        return Err(MimeBuildError::Structure);
    }

    let mut bytes = Vec::new();
    write(nested, opts, true, boundaries, &mut bytes)?;

    let header_end = header_len(&bytes, opts.line_ending.as_bytes());

    if content_type.is("message", "rfc822") && !bytes[..header_end].is_ascii() {
        content_type.sub = String::from("global");
    }

    Ok(Body {
        bytes: Cow::Owned(bytes),
        content_type: Some(content_type),
        encoding: declared_encoding
            .filter(|encoding| encoding.is_identity())
            .cloned(),
    })
}

/// A part of a `multipart/digest`, given an explicit `text/plain` when it
/// is text with no Content-Type, which the digest would read as a message
/// (RFC 2046 5.1.5).
pub(crate) fn explicit(part: &Mime, digest: bool) -> Cow<'_, Mime> {
    let implicit = digest
        && matches!(part.body, MimeBody::Text(_))
        && last(part, MimeHeaderKind::ContentType).is_none();

    if !implicit {
        return Cow::Borrowed(part);
    }

    let mut part = part.clone();
    part.headers.push(MimeHeader {
        name: MimeHeaderName::Known(MimeHeaderKind::ContentType),
        value: MimeHeaderValue::ContentType(text_plain()),
    });

    Cow::Owned(part)
}

/// The content type of a multipart: the declared one, which must be a
/// `multipart`, or `multipart/mixed`.
fn multipart_type(declared: Option<&MimeContentType>) -> Result<MimeContentType, MimeBuildError> {
    let content_type = declared.cloned().unwrap_or_else(|| MimeContentType {
        top: String::from("multipart"),
        sub: String::from("mixed"),
        params: Vec::new(),
    });

    if !content_type.is_top("multipart") {
        return Err(MimeBuildError::Structure);
    }

    Ok(content_type)
}

/// The boundaries a build generates and those it must avoid.
pub(crate) struct Boundaries<'t> {
    prefix: String,
    counter: u64,
    taken: &'t [String],
}

impl<'t> Boundaries<'t> {
    /// Generates boundaries avoiding `taken`, the boundaries of the
    /// multiparts enclosing what is written, the counter starting past
    /// every generated one among them, and the prefix changing its first
    /// character when a taken boundary starts it.
    pub(crate) fn new(taken: &'t [String]) -> Self {
        let blocks = |prefix: &str| taken.iter().any(|taken| prefix.starts_with(taken.as_str()));
        let prefix = iter::once(String::from(PREFIX))
            .chain(
                "_.-+0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
                    .chars()
                    .map(|first| format!("{first}mime-rs_")),
            )
            .find(|prefix| !blocks(prefix))
            .unwrap_or_else(|| String::from(PREFIX));

        let counter = taken
            .iter()
            .filter_map(|boundary| boundary.strip_prefix(PREFIX))
            .filter_map(|hex| u64::from_str_radix(hex, 16).ok())
            .map(|counter| counter.saturating_add(1))
            .max()
            .unwrap_or(0);

        Self {
            prefix,
            counter,
            taken,
        }
    }

    /// The next generated boundary: the prefix and a fixed-width counter,
    /// so that no generated boundary starts another, skipping one that
    /// conflicts with a taken boundary.
    pub(crate) fn next(&mut self) -> String {
        loop {
            let boundary = format!("{}{:016x}", self.prefix, self.counter);
            self.counter = self.counter.wrapping_add(1);

            if self.allows(&boundary) {
                return boundary;
            }
        }
    }

    /// The next generated boundary that occurs in none of `contents`, the
    /// counters already there gathered in one pass, so content listing
    /// many generated boundaries costs one scan rather than one per
    /// candidate.
    pub(crate) fn fresh(&mut self, contents: &[&[u8]]) -> String {
        let prefix = self.prefix.as_bytes();
        let mut used = BTreeSet::new();

        for content in contents {
            for (at, _) in content
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == prefix[0])
            {
                let digits = content
                    .get(at..)
                    .and_then(|rest| rest.strip_prefix(prefix))
                    .and_then(|rest| rest.get(..16))
                    .and_then(|hex| str::from_utf8(hex).ok())
                    .and_then(|hex| u64::from_str_radix(hex, 16).ok());

                used.extend(digits);
            }
        }

        loop {
            let boundary = self.next();

            if !used.contains(&self.counter.wrapping_sub(1)) {
                return boundary;
            }
        }
    }

    /// Whether generated boundaries have the regular prefix, which never
    /// occurs in base64 or quoted-printable output.
    pub(crate) fn regular(&self) -> bool {
        self.prefix == PREFIX
    }

    /// Whether `boundary` neither equals, starts nor is started by a taken
    /// boundary, which would make one delimiter line read as the other.
    pub(crate) fn allows(&self, boundary: &str) -> bool {
        !self
            .taken
            .iter()
            .any(|taken| taken.starts_with(boundary) || boundary.starts_with(taken.as_str()))
    }
}

/// Writes the delimiter line opening a part, the line ending before it
/// included past the first.
fn delimiter(boundary: &str, first: bool, eol: &[u8], out: &mut Vec<u8>) {
    if !first {
        out.extend_from_slice(eol);
    }

    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(eol);
}

/// Writes the close delimiter line, the line ending before it included
/// when a part precedes it.
fn close(boundary: &str, empty: bool, eol: &[u8], out: &mut Vec<u8>) {
    if !empty {
        out.extend_from_slice(eol);
    }

    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"--");
    out.extend_from_slice(eol);
}

/// Whether an entity, once written, may hold the generated boundary prefix
/// at the start of a line: a field name starting with `--`, a declared
/// boundary of the generated shape, or content written as is that holds
/// the prefix anywhere.
fn tainted(mime: &Mime, opts: &MimeBuildOptions) -> bool {
    let names = mime
        .headers
        .iter()
        .any(|header| header.name.as_str().starts_with("--"));

    names
        || match &mime.body {
            MimeBody::Text(text) => text.contains(PREFIX),
            MimeBody::Binary(bytes) => {
                let declared = declared_transfer_encoding(mime)
                    .filter(|encoding| valid(encoding, bytes, opts, &[]));
                let raw = match declared {
                    Some(encoding) => !matches!(
                        encoding,
                        MimeTransferEncoding::QuotedPrintable | MimeTransferEncoding::Base64
                    ),
                    None => valid(&MimeTransferEncoding::Binary, bytes, opts, &[]),
                };

                raw && contains(bytes, PREFIX.as_bytes())
            }
            MimeBody::Multipart(parts) => {
                let boundary = declared_content_type(mime)
                    .and_then(|content_type| content_type.param("boundary"))
                    .is_some_and(|boundary| boundary.starts_with(PREFIX));

                boundary || parts.iter().any(|part| tainted(part, opts))
            }
            MimeBody::Message(nested) => tainted(nested, opts),
        }
}

/// Encodes a leaf's content, keeping the declared transfer encoding when
/// it is valid for the content, choosing one otherwise. Returns the bytes
/// and the encoding to state, `None` for a `7bit` nothing declared.
fn leaf<'b>(
    bytes: Cow<'b, [u8]>,
    text: bool,
    declared: Option<&MimeTransferEncoding>,
    opts: &MimeBuildOptions,
    taken: &[String],
) -> (Cow<'b, [u8]>, Option<MimeTransferEncoding>) {
    let eol = opts.line_ending.as_bytes();
    let cpu = opts.cpu.unwrap_or_else(SimdCpu::current);
    let carries = |encoding: &MimeTransferEncoding| valid(encoding, &bytes, opts, taken);

    if let Some(declared) = declared
        && carries(declared)
    {
        return (
            encode(cpu, declared, bytes, text, eol),
            Some(declared.clone()),
        );
    }

    let chosen = if text && carries(&MimeTransferEncoding::SevenBit) {
        return (bytes, None);
    } else if text && carries(&MimeTransferEncoding::EightBit) {
        MimeTransferEncoding::EightBit
    } else if !text && carries(&MimeTransferEncoding::Binary) {
        MimeTransferEncoding::Binary
    } else if !text && BASE64 {
        MimeTransferEncoding::Base64
    } else {
        let mut quoted = Vec::with_capacity(bytes.len() * 2);
        quoted_printable::encode(cpu, &bytes, text, eol, &mut quoted);

        let base64_len = bytes.len().div_ceil(57) * (76 + eol.len());

        if BASE64 && base64_len < quoted.len() {
            MimeTransferEncoding::Base64
        } else {
            return (
                Cow::Owned(quoted),
                Some(MimeTransferEncoding::QuotedPrintable),
            );
        }
    };

    (encode(cpu, &chosen, bytes, text, eol), Some(chosen))
}

/// Whether `encoding` can carry `bytes` under the options. Content written
/// as is cannot when a line of it would read as a delimiter of a `taken`
/// boundary, nor, ending in a CR, under LF, where a delimiter after it
/// would take that CR for its own line ending.
fn valid(
    encoding: &MimeTransferEncoding,
    bytes: &[u8],
    opts: &MimeBuildOptions,
    taken: &[String],
) -> bool {
    let eol = opts.line_ending.as_bytes();
    let as_is = || !holds_delimiter(bytes, taken);
    let carriage = eol == b"\n" && bytes.ends_with(b"\r");

    match encoding {
        MimeTransferEncoding::SevenBit => fits(bytes, false, eol) && as_is(),
        MimeTransferEncoding::EightBit => opts.allow_8bit && fits(bytes, true, eol) && as_is(),
        MimeTransferEncoding::Binary => opts.allow_binary && !carriage && as_is(),
        MimeTransferEncoding::QuotedPrintable => true,
        MimeTransferEncoding::Base64 => BASE64,
        MimeTransferEncoding::Unknown(_) => !carriage && as_is(),
    }
}

/// Whether a line of `bytes` starts with `--` and a `taken` boundary, as a
/// delimiter line of an enclosing multipart.
pub(crate) fn holds_delimiter(bytes: &[u8], taken: &[String]) -> bool {
    !taken.is_empty()
        && bytes
            .split(|byte| *byte == b'\n')
            .filter_map(|line| line.strip_prefix(b"--"))
            .any(|line| {
                taken
                    .iter()
                    .any(|boundary| line.starts_with(boundary.as_bytes()))
            })
}

fn encode<'b>(
    cpu: SimdCpu,
    encoding: &MimeTransferEncoding,
    bytes: Cow<'b, [u8]>,
    text: bool,
    eol: &[u8],
) -> Cow<'b, [u8]> {
    match encoding {
        MimeTransferEncoding::QuotedPrintable => {
            let mut out = Vec::with_capacity(bytes.len() * 2);
            quoted_printable::encode(cpu, &bytes, text, eol, &mut out);
            Cow::Owned(out)
        }
        _ => bytes,
    }
}

/// Whether bytes can be written as they are under `7bit`, or `8bit` with
/// `eight`: no NUL, no CR or LF but in the line ending, no line over 998
/// octets (RFC 2045 2.7 and 2.8).
pub(crate) fn fits(bytes: &[u8], eight: bool, eol: &[u8]) -> bool {
    if (!eight && !bytes.is_ascii()) || bytes.contains(&0) {
        return false;
    }

    let mut lines = bytes.split(|byte| *byte == b'\n').peekable();

    while let Some(line) = lines.next() {
        let last = lines.peek().is_none();
        let line = match line.strip_suffix(b"\r") {
            Some(line) if eol == b"\r\n" && !last => line,
            _ if eol == b"\r\n" && !last => return false,
            _ => line,
        };

        if line.len() > 998 || line.contains(&b'\r') {
            return false;
        }
    }

    true
}

/// Writes every line break of text (CRLF, or LF alone) as `eol`; a CR
/// alone is content and is kept, except right before a line break written
/// as LF, where it could not be told from the break when read back.
fn line_breaks<'b>(bytes: Cow<'b, [u8]>, eol: &[u8]) -> Cow<'b, [u8]> {
    let mut lines = bytes.split(|byte| *byte == b'\n');
    lines.next_back();

    let clean = if eol == b"\r\n" {
        lines.all(|line| line.ends_with(b"\r"))
    } else {
        lines.all(|line| !line.ends_with(b"\r"))
    };

    if clean {
        return bytes;
    }

    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 32);
    let mut i = 0;

    while let Some(&byte) = bytes.get(i) {
        if byte == b'\r' {
            let crs = bytes[i..].iter().take_while(|byte| **byte == b'\r').count();

            match (bytes.get(i + crs) == Some(&b'\n'), eol == b"\n") {
                (true, true) => out.extend_from_slice(eol),
                (true, false) => {
                    out.extend_from_slice(&bytes[i..i + crs - 1]);
                    out.extend_from_slice(eol);
                }
                (false, _) => out.extend_from_slice(&bytes[i..i + crs]),
            }

            i += crs + usize::from(bytes.get(i + crs) == Some(&b'\n'));
        } else if byte == b'\n' {
            out.extend_from_slice(eol);
            i += 1;
        } else {
            out.push(byte);
            i += 1;
        }
    }

    Cow::Owned(out)
}

/// Writes the header block: the model's fields in order, the structural
/// ones replaced by what the body is written as, then the empty line.
fn write_header(
    mime: &Mime,
    body: &Body<'_>,
    opts: &MimeBuildOptions,
    root: bool,
    out: &mut Vec<u8>,
) -> Result<(), MimeBuildError> {
    let content_type = body.content_type.clone().map(MimeHeaderValue::ContentType);
    let encoding = body.encoding.clone().map(MimeHeaderValue::TransferEncoding);

    let last_type = last(mime, MimeHeaderKind::ContentType);
    let last_encoding = last(mime, MimeHeaderKind::ContentTransferEncoding);

    let mut fields: Vec<(MimeHeaderName, Cow<'_, MimeHeaderValue>)> =
        Vec::with_capacity(mime.headers.len() + 3);

    for (index, header) in mime.headers.iter().enumerate() {
        match header.name.kind() {
            Some(MimeHeaderKind::ContentType) => {
                if Some(index) == last_type
                    && let Some(value) = &content_type
                {
                    fields.push((header.name.clone(), Cow::Borrowed(value)));
                }
            }
            Some(MimeHeaderKind::ContentTransferEncoding) => {
                if Some(index) == last_encoding
                    && let Some(value) = &encoding
                {
                    fields.push((header.name.clone(), Cow::Borrowed(value)));
                }
            }
            _ => fields.push((header.name.clone(), Cow::Borrowed(&header.value))),
        }
    }

    if last_type.is_none()
        && let Some(value) = &content_type
    {
        let name = MimeHeaderName::Known(MimeHeaderKind::ContentType);
        fields.push((name, Cow::Borrowed(value)));
    }

    if last_encoding.is_none()
        && let Some(value) = &encoding
    {
        let name = MimeHeaderName::Known(MimeHeaderKind::ContentTransferEncoding);
        fields.push((name, Cow::Borrowed(value)));
    }

    // NOTE: a message opens with a field, or a reader takes its body for
    // the start of its header block; its default content type is stated.
    let implicit = content_type
        .is_none()
        .then(|| MimeHeaderValue::ContentType(text_plain()));

    if root
        && fields.is_empty()
        && let Some(value) = &implicit
    {
        let name = MimeHeaderName::Known(MimeHeaderKind::ContentType);
        fields.push((name, Cow::Borrowed(value)));
    }

    let has_version = fields
        .iter()
        .any(|(name, _)| name.kind() == Some(MimeHeaderKind::MimeVersion));
    let first_content = fields.iter().position(|(name, _)| is_content(name));

    if root
        && !has_version
        && let Some(first) = first_content
    {
        let name = MimeHeaderName::Known(MimeHeaderKind::MimeVersion);
        let value = MimeHeaderValue::Text(String::from("1.0"));
        fields.insert(first, (name, Cow::Owned(value)));
    }

    let legacy = opts.legacy_name.then(|| legacy_name(mime, body)).flatten();
    let eol = opts.line_ending.as_bytes();

    for (name, value) in &fields {
        let spelling = name.as_str();

        if !is_field_name(spelling) {
            return Err(MimeBuildError::FieldName(String::from(spelling)));
        }

        let mut folder = Folder::new(out, spelling, eol);
        field::write(&mut folder, value, opts)?;

        if name.kind() == Some(MimeHeaderKind::ContentType)
            && let Some(filename) = &legacy
        {
            field::write_legacy_name(&mut folder, filename, opts.allow_utf8_headers)?;
        }

        folder.finish();
    }

    Ok(())
}

/// The filename to repeat as a legacy `name` parameter: a non-ASCII
/// `filename` of the last Content-Disposition, when the content type
/// written has no `name`.
fn legacy_name(mime: &Mime, body: &Body<'_>) -> Option<String> {
    let index = last(mime, MimeHeaderKind::ContentDisposition)?;
    let filename = mime.headers[index]
        .value
        .as_disposition()?
        .param("filename")
        .filter(|filename| !filename.is_ascii())?;

    let named = body
        .content_type
        .as_ref()
        .is_some_and(|content_type| content_type.param("name").is_some());

    (!named).then(|| String::from(filename))
}

fn declared_content_type(mime: &Mime) -> Option<&MimeContentType> {
    let index = last(mime, MimeHeaderKind::ContentType)?;

    mime.headers[index]
        .value
        .as_content_type()
        .filter(|content_type| !content_type.top.is_empty())
}

fn declared_transfer_encoding(mime: &Mime) -> Option<&MimeTransferEncoding> {
    let index = last(mime, MimeHeaderKind::ContentTransferEncoding)?;
    mime.headers[index].value.as_transfer_encoding()
}

/// The index of the last field of `kind`.
fn last(mime: &Mime, kind: MimeHeaderKind) -> Option<usize> {
    mime.headers
        .iter()
        .rposition(|header| header.name.kind() == Some(kind))
}

fn is_content(name: &MimeHeaderName) -> bool {
    name.as_str()
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("content-"))
}

fn text_plain() -> MimeContentType {
    MimeContentType {
        top: String::from("text"),
        sub: String::from("plain"),
        params: Vec::new(),
    }
}

/// Sets a parameter, replacing the first of its name, dropping the others.
fn set_param(content_type: &mut MimeContentType, name: &str, value: &str) {
    let mut seen = false;

    content_type.params.retain_mut(|param| {
        if !param.name.eq_ignore_ascii_case(name) {
            return true;
        }

        if seen {
            return false;
        }

        seen = true;
        param.value = String::from(value);
        true
    });

    if !seen {
        content_type.params.push(MimeParam {
            name: String::from(name),
            value: String::from(value),
        });
    }
}

/// Whether a boundary is 1 to 70 RFC 2046 5.1.1 `bchars`, not ending in a
/// space.
pub(crate) fn is_boundary(boundary: &str) -> bool {
    let bchar = |byte: u8| byte.is_ascii_alphanumeric() || b"'()+_,-./:=? ".contains(&byte);

    (1..=BOUNDARY_MAX).contains(&boundary.len())
        && !boundary.ends_with(' ')
        && boundary.bytes().all(bchar)
}

/// Whether `needle` occurs in `haystack`.
pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, rest)) = needle.split_first() else {
        return true;
    };

    haystack
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == first)
        .any(|(at, _)| haystack[at + 1..].starts_with(rest))
}

/// The length of the header block at the start of written bytes, its empty
/// line excluded.
fn header_len(bytes: &[u8], eol: &[u8]) -> usize {
    if bytes.starts_with(eol) {
        return 0;
    }

    let mut blank = Vec::with_capacity(eol.len() * 2);
    blank.extend_from_slice(eol);
    blank.extend_from_slice(eol);

    bytes
        .windows(blank.len())
        .position(|window| window == blank.as_slice())
        .map_or(bytes.len(), |at| at + eol.len())
}

/// About how many bytes an entity writes as: its content grown by base64's
/// ratio and line breaks, and room for its header block.
pub(crate) fn estimate(mime: &Mime) -> usize {
    let content = match &mime.body {
        MimeBody::Text(text) => text.len() * 7 / 5,
        MimeBody::Binary(bytes) => bytes.len() * 7 / 5,
        MimeBody::Multipart(parts) => parts.iter().map(|part| estimate(part) + 96).sum(),
        MimeBody::Message(nested) => estimate(nested),
    };

    content + 128 * (mime.headers.len() + 2)
}

/// Writes one header field, its line ending included, as the writer
/// writes it.
#[cfg(feature = "parser")]
pub(crate) fn field_line(
    name: &MimeHeaderName,
    value: &MimeHeaderValue,
    opts: &MimeBuildOptions,
) -> Result<Vec<u8>, MimeBuildError> {
    let spelling = name.as_str();

    if !is_field_name(spelling) {
        return Err(MimeBuildError::FieldName(String::from(spelling)));
    }

    let mut out = Vec::new();
    let mut folder = Folder::new(&mut out, spelling, opts.line_ending.as_bytes());
    field::write(&mut folder, value, opts)?;
    folder.finish();
    Ok(out)
}

/// Encodes the content of a leaf as the writer would: the line breaks of
/// text written as the build's line ending, `declared` kept when it is
/// valid for the content and for the `taken` boundaries enclosing it, an
/// encoding chosen otherwise. Returns the bytes
/// as written and the encoding to state, `None` for a `7bit` nothing
/// declared.
#[cfg(feature = "parser")]
pub(crate) fn encode_leaf(
    content: &[u8],
    text: bool,
    declared: Option<&MimeTransferEncoding>,
    opts: &MimeBuildOptions,
    taken: &[String],
) -> (Vec<u8>, Option<MimeTransferEncoding>) {
    let eol = opts.line_ending.as_bytes();
    let content = if text {
        line_breaks(Cow::Borrowed(content), eol)
    } else {
        Cow::Borrowed(content)
    };

    let (bytes, encoding) = leaf(content, text, declared, opts, taken);
    let mut out = Vec::with_capacity(bytes.len() * 4 / 3 + 4);

    match &encoding {
        #[cfg(feature = "base64")]
        Some(MimeTransferEncoding::Base64) => crate::codec::base64::encode(
            opts.cpu.unwrap_or_else(SimdCpu::current),
            &bytes,
            eol,
            &mut out,
        ),
        _ => out.extend_from_slice(&bytes),
    }

    (out, encoding)
}

/// Whether a field name can be written: RFC 5322 2.2 `ftext`, at least
/// one, and not starting with `--`, where a line would read as a delimiter.
pub(crate) fn is_field_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with("--")
        && name.bytes().all(|byte| matches!(byte, 33..=57 | 59..=126))
}
