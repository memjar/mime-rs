//! # Message identifiers
//!
//! [`MimeMessageId`], an RFC 5322 3.6.4 `msg-id` checked on construction:
//! the unique half and the domain half a caller joins, since a no_std
//! build reads no clock, hostname or randomness to invent one.

use alloc::{format, string::String};
use core::fmt;

use crate::build::MimeBuildError;

/// A checked message identifier, `id-left "@" id-right`, without angle
/// brackets.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.6.4>
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MimeMessageId {
    left: String,
    right: String,
}

impl MimeMessageId {
    /// Joins a unique part and a domain, each checked: the unique part a
    /// `dot-atom-text`, the domain a `dot-atom-text` or a `no-fold-literal`
    /// (`[192.0.2.1]`).
    pub fn new(unique: &str, domain: &str) -> Result<Self, MimeBuildError> {
        let valid = is_dot_atom_text(unique) && (is_dot_atom_text(domain) || is_literal(domain));

        if !valid {
            return Err(MimeBuildError::MessageId(format!("{unique}@{domain}")));
        }

        Ok(Self {
            left: String::from(unique),
            right: String::from(domain),
        })
    }

    /// Reads an identifier written `left@right`, angle brackets optional,
    /// checked as [`new`](Self::new) checks its halves.
    pub fn parse(id: &str) -> Result<Self, MimeBuildError> {
        let id = id.trim();
        let id = id
            .strip_prefix('<')
            .and_then(|id| id.strip_suffix('>'))
            .unwrap_or(id);

        let Some((left, right)) = id.rsplit_once('@') else {
            return Err(MimeBuildError::MessageId(String::from(id)));
        };

        Self::new(left, right)
    }

    /// The unique half.
    pub fn left(&self) -> &str {
        &self.left
    }

    /// The domain half.
    pub fn right(&self) -> &str {
        &self.right
    }
}

impl fmt::Display for MimeMessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.left, self.right)
    }
}

impl From<MimeMessageId> for String {
    fn from(id: MimeMessageId) -> Self {
        format!("{id}")
    }
}

/// RFC 5322 3.2.3 `dot-atom-text`.
fn is_dot_atom_text(text: &str) -> bool {
    let atext = |byte: u8| byte.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~".contains(&byte);

    !text.is_empty()
        && text
            .split('.')
            .all(|atom| !atom.is_empty() && atom.bytes().all(atext))
}

/// RFC 5322 3.6.4 `no-fold-literal`.
fn is_literal(text: &str) -> bool {
    text.strip_prefix('[')
        .and_then(|text| text.strip_suffix(']'))
        .is_some_and(|inner| inner.bytes().all(|byte| matches!(byte, 33..=90 | 94..=126)))
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use crate::value::message_id::*;

    #[test]
    fn halves_are_checked() {
        let id = MimeMessageId::new("1234.5678", "example.com").unwrap();
        assert_eq!(id.to_string(), "1234.5678@example.com");
        assert!(MimeMessageId::new("a..b", "example.com").is_err());
        assert!(MimeMessageId::new("a b", "example.com").is_err());
        assert!(MimeMessageId::new("a", "[192.0.2.1]").is_ok());
        assert!(MimeMessageId::parse("<x@y>").is_ok());
        assert!(MimeMessageId::parse("no-domain").is_err());
    }
}
