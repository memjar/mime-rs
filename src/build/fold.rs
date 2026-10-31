//! # Folding
//!
//! The writer of one header field line: words separated by single spaces,
//! folded before a separating space whenever the next word would push the
//! line past [`LINE_TARGET`] (RFC 5322 2.2.3), so a continuation line always
//! starts with that space, is never empty, and no line ends in a blank
//! before its fold.
//!
//! [`LINE_TARGET`] is 76, not RFC 5322 2.1.1's 78, so that a line holding an
//! encoded word also keeps RFC 2047 2's 76-character limit.

use alloc::vec::Vec;

use crate::build::MimeBuildError;

/// The column a written line stays within when a fold point allows it.
pub const LINE_TARGET: usize = 76;

/// The longest line RFC 5322 2.1.1 allows, line ending excluded.
pub const LINE_MAX: usize = 998;

/// One header field being written into a buffer.
pub(crate) struct Folder<'o> {
    out: &'o mut Vec<u8>,
    eol: &'static [u8],
    column: usize,
    words: usize,
}

impl<'o> Folder<'o> {
    /// Starts a field: its name and colon.
    pub(crate) fn new(out: &'o mut Vec<u8>, name: &str, eol: &'static [u8]) -> Self {
        out.extend_from_slice(name.as_bytes());
        out.push(b':');

        Self {
            out,
            eol,
            column: name.len() + 1,
            words: 0,
        }
    }

    /// The columns left on the current line for a word written after a
    /// separating space.
    pub(crate) fn room(&self) -> usize {
        LINE_TARGET.saturating_sub(self.column + 1)
    }

    /// Writes a word after a separating space, folding before that space
    /// when the word does not fit what is left of the line. Fails when the
    /// word alone is longer than a line may ever be, or holds a line break.
    pub(crate) fn word(&mut self, word: &[u8]) -> Result<(), MimeBuildError> {
        if word.contains(&b'\r') || word.contains(&b'\n') {
            return Err(MimeBuildError::LineBreak);
        }

        if word.len() + 1 > LINE_MAX {
            return Err(MimeBuildError::LineTooLong);
        }

        let fits = self.column + 1 + word.len() <= LINE_TARGET;

        if !fits && self.words > 0 {
            self.fold();
        }

        self.out.push(b' ');
        self.out.extend_from_slice(word);
        self.column += 1 + word.len();
        self.words += 1;

        if self.column > LINE_MAX {
            return Err(MimeBuildError::LineTooLong);
        }

        Ok(())
    }

    /// Appends bytes to the last word, with no space and no fold point
    /// before them, as the comma ending a list item.
    pub(crate) fn glue(&mut self, bytes: &[u8]) -> Result<(), MimeBuildError> {
        if bytes.contains(&b'\r') || bytes.contains(&b'\n') {
            return Err(MimeBuildError::LineBreak);
        }

        self.out.extend_from_slice(bytes);
        self.column += bytes.len();

        if self.column > LINE_MAX {
            return Err(MimeBuildError::LineTooLong);
        }

        Ok(())
    }

    /// Ends the current line here, so the next word starts a continuation.
    pub(crate) fn fold(&mut self) {
        if self.words == 0 && self.column == 0 {
            return;
        }

        self.out.extend_from_slice(self.eol);
        self.column = 0;
    }

    /// Writes raw bytes, the value of a raw field, as given: every line
    /// break written as the field's line ending, and one not followed by a
    /// blank given a space so it cannot end the field early. Fails on a line
    /// longer than a line may ever be.
    pub(crate) fn raw(&mut self, value: &[u8]) -> Result<(), MimeBuildError> {
        let mut i = 0;

        while let Some(&byte) = value.get(i) {
            match byte {
                b'\r' if value.get(i + 1) == Some(&b'\n') => i += 1,
                b'\r' | b'\n' => {
                    self.out.extend_from_slice(self.eol);
                    self.column = 0;

                    if !matches!(value.get(i + 1), Some(b' ' | b'\t')) {
                        self.out.push(b' ');
                        self.column = 1;
                    }

                    i += 1;
                }
                _ => {
                    self.out.push(byte);
                    self.column += 1;
                    i += 1;
                }
            }

            if self.column > LINE_MAX {
                return Err(MimeBuildError::LineTooLong);
            }
        }

        Ok(())
    }

    /// Ends the field with its line ending.
    pub(crate) fn finish(self) {
        self.out.extend_from_slice(self.eol);
    }
}
