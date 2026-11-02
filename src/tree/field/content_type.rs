//! # Content-Type and Content-Disposition reader
//!
//! The RFC 2045 5.1 and RFC 2183 2 grammars, read liberally: comments and
//! folding whitespace around every token, a missing subtype, a missing `;`
//! between parameters, quoted values, RFC 2231 continuations and RFC 2047
//! encoded words inside values.

use alloc::{borrow::Cow, string::String, vec::Vec};
use core::ops::Range;

use crate::{
    codec::{
        rfc2047,
        rfc2231::{self, MimeRawParam},
    },
    tree::field::lex::{Lexer, eq_ignore_case},
    value::content_type::{MimeContentType, MimeDisposition, MimeParam},
};

/// A Content-Type value read in place: the type and subtype borrowed from
/// the field, the parameters parsed only when asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MimeMediaType<'v> {
    /// The top-level type, as written, empty when the value has none.
    pub top: &'v [u8],
    /// The subtype, as written, empty when missing.
    pub sub: &'v [u8],
    /// The raw field value, read again for its parameters.
    pub raw: &'v [u8],
}

impl<'v> MimeMediaType<'v> {
    /// The `text/plain` an entity with no Content-Type is (RFC 2045 5.2).
    pub const TEXT_PLAIN: MimeMediaType<'static> = MimeMediaType {
        top: b"text",
        sub: b"plain",
        raw: b"",
    };

    /// Reads the type and subtype of a raw Content-Type value, allocating
    /// nothing: comments and blanks skipped, the subtype stopping at `;`.
    pub fn new(raw: &'v [u8]) -> Self {
        let (top, sub) = media(&mut Lexer::new(raw));
        Self { top, sub, raw }
    }

    /// Whether the type is `top/sub`, compared ASCII case-insensitively.
    pub fn is(&self, top: &str, sub: &str) -> bool {
        eq_ignore_case(self.top, top.as_bytes()) && eq_ignore_case(self.sub, sub.as_bytes())
    }

    /// Whether the top-level type is `top`, compared ASCII case-insensitively.
    pub fn is_top(&self, top: &str) -> bool {
        eq_ignore_case(self.top, top.as_bytes())
    }

    /// The decoded value of the first parameter named `name`, compared
    /// ASCII case-insensitively; parses the parameters.
    pub fn param(&self, name: &str) -> Option<String> {
        let [value] = self.raw_params([name]);
        value.map(|value| {
            String::from_utf8(value.into_owned())
                .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
        })
    }

    /// The raw values of the first parameters named `names`, read in one
    /// pass, borrowed from the field whenever they need no decoding.
    pub(crate) fn raw_params<const N: usize>(
        &self,
        names: [&str; N],
    ) -> [Option<Cow<'v, [u8]>>; N] {
        let mut lex = Lexer::new(self.raw);
        media(&mut lex);
        raw_params(lex, names)
    }

    /// The whole value decoded, parameters included.
    pub fn to_content_type(&self) -> MimeContentType {
        if self.raw.is_empty() && !self.top.is_empty() {
            return MimeContentType {
                top: String::from_utf8_lossy(self.top).into_owned(),
                sub: String::from_utf8_lossy(self.sub).into_owned(),
                params: Vec::new(),
            };
        }

        parse_content_type(self.raw)
    }
}

/// Reads a Content-Type value. A value with no type before its `/` or its
/// first `;` reads as the default (empty) content type.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-5.1>
pub fn parse_content_type(value: &[u8]) -> MimeContentType {
    let mut lex = Lexer::new(value);
    let (top, sub) = media(&mut lex);

    if top.is_empty() {
        return MimeContentType::default();
    }

    MimeContentType {
        top: String::from_utf8_lossy(top).into_owned(),
        sub: String::from_utf8_lossy(sub).into_owned(),
        params: params(&mut lex),
    }
}

/// Reads a Content-Disposition value.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2183#section-2>
pub fn parse_disposition(value: &[u8]) -> MimeDisposition {
    let mut lex = Lexer::new(value);
    lex.skip_cfws();

    let kind = lex.until(|byte| matches!(byte, b';' | b'(') || is_blank(byte));

    if kind.is_empty() {
        return MimeDisposition::default();
    }

    MimeDisposition {
        kind: String::from_utf8_lossy(kind).into_owned(),
        params: params(&mut lex),
    }
}

/// Reads the type and subtype opening a Content-Type value, comments and
/// blanks skipped, the subtype stopping at `;`; the lexer is left on what
/// follows, where a parameter may start with no `;`.
pub(crate) fn media<'v>(lex: &mut Lexer<'v>) -> (&'v [u8], &'v [u8]) {
    lex.skip_cfws();
    let top = lex.until(|byte| matches!(byte, b'/' | b';' | b'(') || is_blank(byte));
    lex.skip_cfws();

    if lex.peek() != Some(b'/') {
        return (top, &[]);
    }

    lex.bump();
    lex.skip_cfws();
    let sub = lex.until(|byte| matches!(byte, b';' | b'(') || is_blank(byte));
    (top, sub)
}

/// The values of the first parameters named `names`, read in one pass from
/// a lexer past the media type: unquoted and unescaped, or, for an RFC 2231
/// or encoded-word value, decoded by the full reader.
pub(crate) fn raw_params<'v, const N: usize>(
    mut lex: Lexer<'v>,
    names: [&str; N],
) -> [Option<Cow<'v, [u8]>>; N] {
    let mut out = [const { None }; N];
    let mut settled = [false; N];
    let value = lex.bytes;

    let full = |name: &str| {
        let params = parse_content_type(value).params;
        params
            .into_iter()
            .find(|param| param.name.eq_ignore_ascii_case(name))
            .map(|param| Cow::Owned(param.value.into_bytes()))
    };

    loop {
        while matches!(lex.peek(), Some(b';')) {
            lex.bump();
        }

        lex.skip_cfws();

        if lex.is_done() || !settled.contains(&false) {
            return out;
        }

        let start = lex.pos;
        let found = lex.until(|byte| matches!(byte, b'=' | b';' | b'(') || is_blank(byte));
        lex.skip_cfws();

        if lex.peek() != Some(b'=') {
            if lex.pos == start {
                lex.bump();
            }

            continue;
        }

        lex.bump();
        lex.skip_cfws();

        let value: Cow<'v, [u8]> = if lex.peek() == Some(b'"') {
            lex.quoted_cow()
        } else {
            Cow::Borrowed(lex.until(|byte| matches!(byte, b';' | b'(') || is_blank(byte)))
        };

        for (i, name) in names.iter().enumerate() {
            if settled[i] {
                continue;
            }

            let extended = found.len() > name.len()
                && found[..name.len()].eq_ignore_ascii_case(name.as_bytes())
                && found[name.len()] == b'*';

            if extended || eq_ignore_case(found, name.as_bytes()) {
                settled[i] = true;
                out[i] = if extended || rfc2047::contains_word_start(&value) {
                    full(name)
                } else {
                    Some(value.clone())
                };
            }
        }
    }
}

/// The byte range of the value of the parameter `name`, its quotes
/// included, in a raw Content-Type or Content-Disposition value; `None`
/// when the parameter is absent or written by RFC 2231.
pub(crate) fn param_span(value: &[u8], name: &str) -> Option<Range<usize>> {
    let mut lex = Lexer::new(value);
    media(&mut lex);

    loop {
        while matches!(lex.peek(), Some(b';')) {
            lex.bump();
        }

        lex.skip_cfws();

        if lex.is_done() {
            return None;
        }

        let start = lex.pos;
        let found = lex.until(|byte| matches!(byte, b'=' | b';' | b'(') || is_blank(byte));
        lex.skip_cfws();

        if lex.peek() != Some(b'=') {
            if lex.pos == start {
                lex.bump();
            }

            continue;
        }

        lex.bump();
        lex.skip_cfws();

        let value_start = lex.pos;

        if lex.peek() == Some(b'"') {
            lex.quoted();
        } else {
            lex.until(|byte| matches!(byte, b';' | b'(') || is_blank(byte));
        }

        if found.eq_ignore_ascii_case(name.as_bytes()) {
            return Some(value_start..lex.pos);
        }
    }
}

fn params(lex: &mut Lexer<'_>) -> Vec<MimeParam> {
    let mut raw: Vec<MimeRawParam<'_>> = Vec::new();
    let mut last_unquoted = false;

    loop {
        let start = lex.pos;

        lex.skip_cfws();

        while lex.peek() == Some(b';') {
            lex.bump();
            lex.skip_cfws();
            last_unquoted = false;
        }

        if lex.is_done() {
            break;
        }

        // NOTE: an encoded word after an unquoted value, as in
        // `name==?utf-8?q?a?= =?utf-8?q?b?=.txt`, continues that value.
        if last_unquoted
            && lex.bytes[lex.pos..].starts_with(b"=?")
            && let Some(param) = raw.last_mut()
        {
            let word = lex.until(|byte| byte == b';' || is_blank(byte));
            let mut value = param.value.to_vec();
            value.push(b' ');
            value.extend_from_slice(word);
            param.value = Cow::Owned(value);
            continue;
        }

        let name = lex.until(|byte| matches!(byte, b'=' | b';' | b'(') || is_blank(byte));
        lex.skip_cfws();

        if lex.peek() != Some(b'=') {
            // NOTE: a bare word after an unquoted value, as in `name=a b`,
            // continues that value.
            if last_unquoted
                && !name.is_empty()
                && let Some(param) = raw.last_mut()
            {
                let mut value = param.value.to_vec();
                value.push(b' ');
                value.extend_from_slice(name);
                param.value = Cow::Owned(value);
            }

            if lex.pos == start {
                lex.bump();
            }

            continue;
        }

        lex.bump();
        lex.skip_cfws();

        let (value, quoted): (Cow<'_, [u8]>, bool) = if lex.peek() == Some(b'"') {
            (Cow::Owned(lex.quoted()), true)
        } else {
            let value = lex.until(|byte| matches!(byte, b';' | b'(') || is_blank(byte));
            (Cow::Borrowed(value), false)
        };

        last_unquoted = !quoted;

        if name.is_empty() || value.is_empty() {
            last_unquoted = false;
            continue;
        }

        raw.push(MimeRawParam::new(name, value));
    }

    rfc2231::join(raw)
}

fn is_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

#[cfg(test)]
mod tests {
    use crate::tree::field::content_type::*;

    #[test]
    fn comments_and_blanks_surround_every_token() {
        let ct =
            parse_content_type(b"(hello) text (plain) / (world) plain (eod); charset=us-ascii");
        assert!(ct.is("text", "plain"));
        assert_eq!(ct.param("charset"), Some("us-ascii"));

        let ct = parse_content_type(b" text/plain; charset =us-ascii (Plain text)\n");
        assert_eq!(ct.param("CHARSET"), Some("us-ascii"));
    }

    #[test]
    fn a_missing_semicolon_still_separates_parameters() {
        let ct = parse_content_type(b"message/external-body;\n    access-type=mail-server\n     server=\"listserv@bogus.bitnet\"");
        assert_eq!(ct.param("access-type"), Some("mail-server"));
        assert_eq!(ct.param("server"), Some("listserv@bogus.bitnet"));
    }

    #[test]
    fn the_readme_continuations_join() {
        let ct = parse_content_type(
            b"image/gif; name*1=\"about \"; name*0=\"Book \";\n              name*2*=utf-8''%e2%98%95 tables.gif",
        );
        assert_eq!(ct.param("name"), Some("Book about ☕ tables.gif"));
    }

    #[test]
    fn a_value_without_a_type_reads_as_nothing() {
        assert_eq!(
            parse_content_type(b"/plain;charset=us-ascii"),
            MimeContentType::default()
        );
        assert!(parse_content_type(b"inline").sub.is_empty());
    }

    #[test]
    fn a_quoted_boundary_keeps_its_spaces() {
        let ct =
            parse_content_type(b" multipart/mixed;\n     boundary=\"---- main boundary ----\"");
        assert_eq!(ct.param("boundary"), Some("---- main boundary ----"));
    }
}
