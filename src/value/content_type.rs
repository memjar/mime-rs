//! # Content type and disposition
//!
//! The decoded value of a Content-Type or a Content-Disposition field: a
//! media type (or a disposition type) and its parameters, RFC 2231
//! continuations already joined and decoded.

use alloc::{string::String, vec::Vec};

/// A decoded Content-Type value.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2045#section-5>
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeContentType {
    /// The top-level type (`text`, `multipart`...), as written.
    pub top: String,
    /// The subtype (`plain`, `mixed`...), as written, empty when missing.
    pub sub: String,
    /// The parameters, in source order, continuations joined.
    pub params: Vec<MimeParam>,
}

/// A decoded Content-Disposition value.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2183#section-2>
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeDisposition {
    /// The disposition type (`inline`, `attachment`...), as written.
    pub kind: String,
    /// The parameters, in source order, continuations joined.
    pub params: Vec<MimeParam>,
}

/// A content type or disposition parameter.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2231>
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MimeParam {
    /// The parameter name, as written, without any RFC 2231 section suffix.
    pub name: String,
    /// The value, unquoted, unescaped and decoded.
    pub value: String,
}

impl MimeContentType {
    /// Whether the type is `top/sub`, compared ASCII case-insensitively.
    pub fn is(&self, top: &str, sub: &str) -> bool {
        self.top.eq_ignore_ascii_case(top) && self.sub.eq_ignore_ascii_case(sub)
    }

    /// Whether the top-level type is `top`, ASCII case-insensitively.
    pub fn is_top(&self, top: &str) -> bool {
        self.top.eq_ignore_ascii_case(top)
    }

    /// The value of the first `name` parameter, ASCII case-insensitively.
    pub fn param(&self, name: &str) -> Option<&str> {
        param(&self.params, name)
    }
}

impl MimeDisposition {
    /// Whether the disposition type is `kind`, ASCII case-insensitively.
    pub fn is(&self, kind: &str) -> bool {
        self.kind.eq_ignore_ascii_case(kind)
    }

    /// The value of the first `name` parameter, ASCII case-insensitively.
    pub fn param(&self, name: &str) -> Option<&str> {
        param(&self.params, name)
    }
}

fn param<'p>(params: &'p [MimeParam], name: &str) -> Option<&'p str> {
    params
        .iter()
        .find(|param| param.name.eq_ignore_ascii_case(name))
        .map(|param| param.value.as_str())
}
