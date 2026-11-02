//! # Lexer
//!
//! The RFC 5322 3.2 lexical pieces every structured field reader shares:
//! folding whitespace, nested comments, quoted strings and their escapes.
//!
//! Readers are liberal: an unclosed quote or comment runs to the end of the
//! value instead of failing, so every value reads as something.

use alloc::{borrow::Cow, vec::Vec};

/// A cursor over one raw field value.
#[derive(Clone, Debug)]
pub(crate) struct Lexer<'v> {
    pub(crate) bytes: &'v [u8],
    pub(crate) pos: usize,
}

impl<'v> Lexer<'v> {
    pub(crate) fn new(bytes: &'v [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    pub(crate) fn is_done(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    pub(crate) fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    /// Skips folding whitespace (blanks and line breaks), not comments.
    pub(crate) fn skip_fws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    /// Skips folding whitespace and comments (RFC 5322 3.2.2 CFWS).
    pub(crate) fn skip_cfws(&mut self) {
        loop {
            self.skip_fws();

            if self.peek() != Some(b'(') {
                return;
            }

            self.comment();
        }
    }

    /// Reads a comment, the cursor on its opening parenthesis, and returns
    /// its raw content (nested comments and escapes included, the outer
    /// parentheses excluded).
    pub(crate) fn comment(&mut self) -> &'v [u8] {
        let mut depth = 0usize;
        let start = self.pos + 1;

        while let Some(byte) = self.bump() {
            match byte {
                b'\\' => {
                    self.bump();
                }
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;

                    if depth == 0 {
                        return &self.bytes[start..self.pos - 1];
                    }
                }
                _ => {}
            }
        }

        &self.bytes[start.min(self.bytes.len())..]
    }

    /// Reads a quoted string, the cursor on its opening quote, unescaping
    /// quoted pairs and unfolding line breaks (RFC 5322 3.2.4).
    pub(crate) fn quoted(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        self.pos += 1;

        while let Some(byte) = self.bump() {
            match byte {
                b'"' => return out,
                b'\\' => {
                    if let Some(escaped) = self.bump() {
                        out.push(escaped);
                    }
                }
                b'\r' | b'\n' => {}
                _ => out.push(byte),
            }
        }

        out
    }

    /// [`quoted`](Self::quoted), borrowed when the string holds no quoted
    /// pair and no line break.
    pub(crate) fn quoted_cow(&mut self) -> Cow<'v, [u8]> {
        let start = self.pos + 1;
        let rest = self.bytes.get(start..).unwrap_or_default();
        let plain = rest
            .iter()
            .position(|byte| matches!(byte, b'"' | b'\\' | b'\r' | b'\n'))
            .filter(|&at| rest[at] == b'"');

        match plain {
            Some(len) => {
                self.pos = start + len + 1;
                Cow::Borrowed(&rest[..len])
            }
            None => Cow::Owned(self.quoted()),
        }
    }

    /// Reads bytes until `stop` matches one or the value ends.
    pub(crate) fn until(&mut self, stop: impl Fn(u8) -> bool) -> &'v [u8] {
        let start = self.pos;

        while let Some(byte) = self.peek() {
            if stop(byte) {
                break;
            }

            self.pos += 1;
        }

        &self.bytes[start..self.pos]
    }
}
