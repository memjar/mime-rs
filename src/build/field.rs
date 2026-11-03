//! # Field writers
//!
//! The writing side of every header value form, each read back as written by
//! its `tree::field` reader, folded by [`fold`](super::fold): encoded words
//! in text and phrases where RFC 2047 5 allows them, quoted phrases when
//! they hold specials, RFC 2231 for values not plain ASCII or over a line.

use alloc::{format, string::String, vec::Vec};
use core::mem;

use crate::{
    build::{
        MimeBuildError, MimeBuildOptions,
        fold::{Folder, LINE_TARGET},
    },
    codec::quoted_printable,
    value::{
        MimeHeaderValue,
        address::{MimeAddress, MimeAddressList, MimeMailbox},
        content_type::MimeParam,
    },
};

/// The longest RFC 2047 encoded word (RFC 2047 2).
const WORD_MAX: usize = 75;

/// The `=?utf-8?X?` and `?=` around an encoded word's text.
const WORD_OVERHEAD: usize = 12;

/// The longest parameter section on one line before RFC 2231 continuations.
const SECTION_MAX: usize = 60;

/// Writes a field value in the form its arm states.
pub(crate) fn write(
    folder: &mut Folder<'_>,
    value: &MimeHeaderValue,
    opts: &MimeBuildOptions,
) -> Result<(), MimeBuildError> {
    let utf8 = opts.allow_utf8_headers;

    match value {
        MimeHeaderValue::Raw(raw) => folder.raw(raw)?,
        MimeHeaderValue::Text(text) => write_text(folder, text, utf8)?,
        MimeHeaderValue::Addresses(list) => write_addresses(folder, list, utf8)?,
        MimeHeaderValue::MessageIds(ids) => {
            // NOTE: an identifier holds no bracket and no blank (RFC 5322
            // 3.6.4), and one that did would not read back.
            for id in ids {
                let id: String = id
                    .chars()
                    .filter(|c| !matches!(c, '<' | '>') && !c.is_whitespace())
                    .collect();

                if !id.is_empty() {
                    folder.word(format!("<{id}>").as_bytes())?;
                }
            }
        }
        MimeHeaderValue::Date(date) => {
            for word in format!("{date}").split(' ') {
                folder.word(word.as_bytes())?;
            }
        }
        MimeHeaderValue::Urls(urls) => {
            for (index, url) in urls.iter().enumerate() {
                if index > 0 {
                    folder.glue(b",")?;
                }

                folder.word(format!("<{url}>").as_bytes())?;
            }
        }
        MimeHeaderValue::CommaList(items) => {
            let items = items
                .iter()
                .filter(|item| !item.trim_matches([' ', '\t']).is_empty());

            for (index, item) in items.enumerate() {
                if index > 0 {
                    folder.glue(b",")?;
                }

                write_phrase(folder, item, utf8, false)?;
            }
        }
        MimeHeaderValue::ContentType(content_type) => {
            let media = format!("{}/{}", content_type.top, content_type.sub);
            folder.word(media.as_bytes())?;

            for param in content_type
                .params
                .iter()
                .filter(|param| is_param_name(&param.name) && !param.value.is_empty())
            {
                let legacy = opts.legacy_name
                    && param.name.eq_ignore_ascii_case("name")
                    && !param.value.is_ascii();

                if legacy {
                    write_legacy_name(folder, &param.value, utf8)?;
                } else {
                    folder.glue(b";")?;
                    write_param(folder, &param.name, &param.value, utf8)?;
                }
            }
        }
        MimeHeaderValue::Disposition(disposition) => {
            folder.word(disposition.kind.as_bytes())?;
            write_params(folder, &disposition.params, utf8)?;
        }
        MimeHeaderValue::TransferEncoding(encoding) => folder.word(encoding.as_bytes())?,
    }

    Ok(())
}

/// Writes unstructured text (RFC 5322 3.2.5), its ends trimmed.
///
/// Words split at single spaces; each run of words that needs it is written as
/// encoded words, the spaces inside the run encoded with it.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2047#section-5>
pub(crate) fn write_text(
    folder: &mut Folder<'_>,
    text: &str,
    utf8: bool,
) -> Result<(), MimeBuildError> {
    let text = text.trim_matches([' ', '\t']);
    let words = words(text);
    let mut i = 0;

    while i < words.len() {
        let (start, end) = words[i];

        if !needs_encoding(&text[start..end], utf8) {
            folder.word(&text.as_bytes()[start..end])?;
            i += 1;
            continue;
        }

        let mut last = i;

        while last + 1 < words.len() {
            let (start, end) = words[last + 1];

            if !needs_encoding(&text[start..end], utf8) {
                break;
            }

            last += 1;
        }

        write_encoded(folder, &text[start..words[last].1])?;
        i = last + 1;
    }

    Ok(())
}

/// Writes a phrase (RFC 5322 3.2.5) as atoms, a quoted string or encoded words.
///
/// Atoms when every word is one, else a quoted string when printable ASCII
/// (or UTF-8 when UTF-8 headers are allowed), else encoded words. With
/// `quote`, never bare atoms, so it cannot be read as an address.
pub(crate) fn write_phrase(
    folder: &mut Folder<'_>,
    phrase: &str,
    utf8: bool,
    quote: bool,
) -> Result<(), MimeBuildError> {
    let phrase = phrase.trim_matches([' ', '\t']);

    if phrase.contains("=?") || phrase.chars().any(|c| c.is_control()) {
        return write_encoded(folder, phrase);
    }

    let atoms = !quote
        && !phrase.is_empty()
        && !phrase.contains("  ")
        && phrase.split(' ').all(|word| {
            !word.is_empty() && word.chars().all(|c| is_atext(c) || (utf8 && !c.is_ascii()))
        });

    if atoms {
        for word in phrase.split(' ') {
            folder.word(word.as_bytes())?;
        }

        return Ok(());
    }

    if phrase.is_ascii() || utf8 {
        return folder.word(&quoted(phrase));
    }

    write_encoded(folder, phrase)
}

/// Writes an address list: mailboxes and groups separated by commas.
///
/// A group's members go between its colon and its semicolon.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.4>
pub(crate) fn write_addresses(
    folder: &mut Folder<'_>,
    list: &MimeAddressList,
    utf8: bool,
) -> Result<(), MimeBuildError> {
    for (index, item) in list.items.iter().enumerate() {
        if index > 0 {
            folder.glue(b",")?;
        }

        match item {
            MimeAddress::Mailbox(mailbox) => write_mailbox(folder, mailbox, utf8)?,
            MimeAddress::Group(group) => {
                write_phrase(
                    folder,
                    group.name.as_deref().unwrap_or_default(),
                    utf8,
                    false,
                )?;
                folder.glue(b":")?;

                for (index, mailbox) in group.mailboxes.iter().enumerate() {
                    if index > 0 {
                        folder.glue(b",")?;
                    }

                    write_mailbox(folder, mailbox, utf8)?;
                }

                folder.glue(b";")?;
            }
        }
    }

    Ok(())
}

fn write_mailbox(
    folder: &mut Folder<'_>,
    mailbox: &MimeMailbox,
    utf8: bool,
) -> Result<(), MimeBuildError> {
    let name = mailbox
        .name
        .as_deref()
        .filter(|name| !name.trim_matches([' ', '\t']).is_empty());

    let address = mailbox.address.as_deref().map(angle_safe);

    match (name, address.as_deref()) {
        (Some(name), Some(address)) => {
            write_phrase(folder, name, utf8, false)?;
            folder.word(format!("<{address}>").as_bytes())
        }
        (None, Some(address)) if is_plain_address(address) => folder.word(address.as_bytes()),
        (None, Some(address)) if !address.is_empty() => {
            folder.word(format!("<{address}>").as_bytes())
        }
        (Some(name), None) => write_phrase(folder, name, utf8, true),
        (None, _) => folder.word(b"<>"),
    }
}

/// Writes the parameters of a content type or disposition, each after a `;`.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2231>
pub(crate) fn write_params(
    folder: &mut Folder<'_>,
    params: &[MimeParam],
    utf8: bool,
) -> Result<(), MimeBuildError> {
    for param in params
        .iter()
        .filter(|param| is_param_name(&param.name) && !param.value.is_empty())
    {
        folder.glue(b";")?;
        write_param(folder, &param.name, &param.value, utf8)?;
    }

    Ok(())
}

fn write_param(
    folder: &mut Folder<'_>,
    name: &str,
    value: &str,
    utf8: bool,
) -> Result<(), MimeBuildError> {
    if !value.is_empty() && value.bytes().all(is_token) {
        return folder.word(format!("{name}={value}").as_bytes());
    }

    let plain = !value.contains("=?")
        && !value.chars().any(|c| c.is_control())
        && (value.is_ascii() || utf8);

    if plain && value.len() <= SECTION_MAX {
        let mut word = Vec::from(format!("{name}=").as_bytes());
        word.extend_from_slice(&quoted(value));
        return folder.word(&word);
    }

    // NOTE: RFC 2231 3 and 4: continuations, the value percent-encoded
    // with its charset when it is not plain, each section fitting a line
    // with its name, number and charset.
    let budget = (LINE_TARGET - 2)
        .saturating_sub(name.len() + "*00*=utf-8''".len())
        .clamp(8, SECTION_MAX);
    let sections = sections(value, !plain, budget);

    if let [section] = sections.as_slice() {
        let word = if plain {
            let mut word = Vec::from(format!("{name}=").as_bytes());
            word.extend_from_slice(&quoted(section));
            word
        } else {
            Vec::from(format!("{name}*=utf-8''{section}").as_bytes())
        };

        return folder.word(&word);
    }

    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            folder.glue(b";")?;
        }

        let word = match (plain, index) {
            (true, _) => {
                let mut word = Vec::from(format!("{name}*{index}=").as_bytes());
                word.extend_from_slice(&quoted(section));
                word
            }
            (false, 0) => Vec::from(format!("{name}*0*=utf-8''{section}").as_bytes()),
            (false, _) => Vec::from(format!("{name}*{index}*={section}").as_bytes()),
        };

        folder.word(&word)?;
    }

    Ok(())
}

/// Cuts a parameter value into sections of at most `budget` written chars.
///
/// Whole characters each, percent-encoded with `encode`.
fn sections(value: &str, encode: bool, budget: usize) -> Vec<String> {
    let mut sections = Vec::new();
    let mut section = String::new();
    let mut width = 0;

    for c in value.chars() {
        let mut piece = String::new();

        if encode {
            let mut bytes = [0; 4];

            for byte in c.encode_utf8(&mut bytes).bytes() {
                if is_attribute_char(byte) {
                    piece.push(byte as char);
                } else {
                    piece.push_str(&format!("%{byte:02X}"));
                }
            }
        } else {
            piece.push(c);
        }

        let piece_width = if encode {
            piece.len()
        } else {
            quoted(&piece).len() - 2
        };

        if width + piece_width > budget && !section.is_empty() {
            sections.push(mem::take(&mut section));
            width = 0;
        }

        section.push_str(&piece);
        width += piece_width;
    }

    sections.push(section);
    sections
}

/// Writes text as RFC 2047 UTF-8 encoded words, B or Q whichever is shorter.
///
/// Each word is at most 75 characters and holds whole characters, so it
/// decodes on its own; the first is sized to what is left of the line.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2047#section-2>
pub(crate) fn write_encoded(folder: &mut Folder<'_>, text: &str) -> Result<(), MimeBuildError> {
    let encoding = WordEncoding::shortest(text.as_bytes());
    let mut rest = text;

    while !rest.is_empty() {
        let mut room = folder.room();

        if room < 2 * WORD_OVERHEAD {
            folder.fold();
            room = LINE_TARGET - 1;
        }

        let budget = room.min(WORD_MAX) - WORD_OVERHEAD;
        let end = chunk_end(encoding, rest, budget);
        let mut word = Vec::with_capacity(WORD_MAX);

        push_word(encoding, &rest.as_bytes()[..end], &mut word);
        folder.word(&word)?;
        rest = &rest[end..];
    }

    Ok(())
}

/// Writes the legacy `name` parameter of a content type from `filename`.
///
/// Encoded words inside a quoted string, which RFC 2047 5 forbids and readers
/// knowing neither RFC 2231 nor RFC 2183 expect; with UTF-8 headers, written
/// as any other parameter.
pub(crate) fn write_legacy_name(
    folder: &mut Folder<'_>,
    filename: &str,
    utf8: bool,
) -> Result<(), MimeBuildError> {
    if utf8 {
        folder.glue(b";")?;
        return write_param(folder, "name", filename, true);
    }

    // NOTE: encoded words read back trimmed.
    let filename = filename.trim_matches([' ', '\t']);
    let encoding = WordEncoding::shortest(filename.as_bytes());
    let mut word = Vec::from(&b"name=\""[..]);
    let mut rest = filename;

    while !rest.is_empty() {
        if word.len() > 6 {
            word.push(b' ');
        }

        let end = chunk_end(encoding, rest, WORD_MAX - WORD_OVERHEAD);
        push_word(encoding, &rest.as_bytes()[..end], &mut word);
        rest = &rest[end..];
    }

    word.push(b'"');
    folder.glue(b";")?;
    folder.word(&word)
}

/// The end of the longest prefix of `text` whose encoding fits `budget`.
///
/// Whole characters, and at least one.
fn chunk_end(encoding: WordEncoding, text: &str, budget: usize) -> usize {
    let mut end = 0;

    for (at, c) in text.char_indices() {
        let next = at + c.len_utf8();

        if end > 0 && encoding.len(&text.as_bytes()[..next]) > budget {
            break;
        }

        end = next;
    }

    end
}

/// Appends one encoded word of `bytes` in UTF-8.
fn push_word(encoding: WordEncoding, bytes: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(b"=?utf-8?");
    out.push(encoding.tag());
    out.push(b'?');
    encoding.encode(bytes, out);
    out.extend_from_slice(b"?=");
}

/// The RFC 2047 4 encodings of an encoded word this build can write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WordEncoding {
    Q,
    #[cfg(feature = "base64")]
    B,
}

impl WordEncoding {
    const ALL: &[Self] = &[
        Self::Q,
        #[cfg(feature = "base64")]
        Self::B,
    ];

    fn shortest(bytes: &[u8]) -> Self {
        Self::ALL
            .iter()
            .copied()
            .min_by_key(|encoding| encoding.len(bytes))
            .unwrap_or(Self::Q)
    }

    fn tag(self) -> u8 {
        match self {
            Self::Q => b'Q',
            #[cfg(feature = "base64")]
            Self::B => b'B',
        }
    }

    fn len(self, bytes: &[u8]) -> usize {
        match self {
            Self::Q => quoted_printable::encoded_q_len(bytes),
            #[cfg(feature = "base64")]
            Self::B => bytes.len().div_ceil(3) * 4,
        }
    }

    fn encode(self, bytes: &[u8], out: &mut Vec<u8>) {
        match self {
            Self::Q => quoted_printable::encode_q(bytes, out),
            #[cfg(feature = "base64")]
            Self::B => crate::codec::base64::encode_b(bytes, out),
        }
    }
}

/// The byte ranges of the words of `text`, split at single spaces.
///
/// A run of several blanks stays inside a word, since unfolding it back would
/// shrink it.
fn words(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut words = Vec::new();
    let mut start = 0;

    for (at, &byte) in bytes.iter().enumerate() {
        let single = byte == b' '
            && at > start
            && !matches!(bytes.get(at - 1), Some(b' ' | b'\t'))
            && !matches!(bytes.get(at + 1), None | Some(b' ' | b'\t'));

        if single {
            words.push((start, at));
            start = at + 1;
        }
    }

    if start < bytes.len() {
        words.push((start, bytes.len()));
    }

    words
}

/// Whether a word of unstructured text must be written as an encoded word.
///
/// It holds a control character, a non-ASCII one without UTF-8 headers, or
/// something a reader would take for an encoded word.
fn needs_encoding(word: &str, utf8: bool) -> bool {
    word.contains("=?")
        || word
            .chars()
            .any(|c| (c.is_control() && c != '\t') || (!c.is_ascii() && !utf8))
}

/// A quoted string, `"` and `\` escaped.
fn quoted(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 2);
    out.push(b'"');

    for byte in text.bytes() {
        if matches!(byte, b'"' | b'\\') {
            out.push(b'\\');
        }

        out.push(byte);
    }

    out.push(b'"');
    out
}

/// An address reduced to what an angle address reads back unchanged.
///
/// Brackets, parentheses, backslashes, blanks and controls dropped outside
/// quoted strings, a quoted string kept only when it closes, without its
/// escapes, and an obsolete route dropped as the reader drops it.
fn angle_safe(address: &str) -> String {
    let unquoted = |c: char| {
        !matches!(c, '<' | '>' | '(' | ')' | '\\') && !c.is_whitespace() && !c.is_control()
    };
    let mut out = String::with_capacity(address.len());
    let mut chars = address.chars();

    while let Some(c) = chars.next() {
        if c != '"' {
            if unquoted(c) {
                out.push(c);
            }

            continue;
        }

        let mut inner = String::new();
        let mut closed = false;

        for c in chars.by_ref() {
            if c == '"' {
                closed = true;
                break;
            }

            if !matches!(c, '\\' | '\r' | '\n') {
                inner.push(c);
            }
        }

        if closed {
            out.push('"');
            out.push_str(&inner);
            out.push('"');
        } else {
            out.extend(inner.chars().filter(|c| unquoted(*c)));
        }
    }

    // NOTE: dropping a route may cut a quoted string open, so what is left
    // is reduced again.
    if out.starts_with('@')
        && let Some(colon) = out.rfind(':')
    {
        return angle_safe(&out[colon + 1..]);
    }

    out
}

/// Whether an address can stand bare, a `dot-atom` on each side of one `@`.
///
/// So no reader takes part of it for a separator or a display name.
fn is_plain_address(address: &str) -> bool {
    let dot_atom = |part: &str| {
        !part.is_empty()
            && part
                .split('.')
                .all(|atom| !atom.is_empty() && atom.chars().all(|c| is_atext(c) || !c.is_ascii()))
    };

    address
        .split_once('@')
        .is_some_and(|(local, domain)| dot_atom(local) && dot_atom(domain))
}

/// RFC 5322 3.2.3 `atext`.
fn is_atext(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(c)
}

/// RFC 2045 5.1 `token`: printable ASCII but blanks and `tspecials`.
fn is_token(byte: u8) -> bool {
    matches!(byte, 33..=126) && !b"()<>@,;:\\\"/[]?=".contains(&byte)
}

/// Whether a parameter name is one or more RFC 2231 `attribute-char`s.
///
/// A parameter named otherwise, or with an empty value, would not read back.
fn is_param_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_attribute_char)
}

/// RFC 2231 7 `attribute-char`.
fn is_attribute_char(byte: u8) -> bool {
    is_token(byte) && !matches!(byte, b'*' | b'\'' | b'%')
}
