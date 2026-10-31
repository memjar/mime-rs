//! # Flowed text
//!
//! RFC 3676 `format=flowed` text: soft line breaks (a line ending in a
//! space) joined into paragraphs on reading, and long lines wrapped into
//! them on writing, quote depth kept per paragraph.

use alloc::{string::String, vec::Vec};

/// Unwraps flowed text: a line ending in a space continues into the next
/// line of the same quote depth, the space removed too with `delsp`; one
/// leading space of a space-stuffed line is removed; the signature
/// separator `-- ` never flows. Quoted paragraphs come back prefixed with
/// their `>` marks and a space.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc3676#section-4.2>
pub fn decode(text: &str, delsp: bool) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(text.len());
    let mut paragraph: Option<(usize, String)> = None;
    let ends_with_break = text.ends_with('\n');
    let mut lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();

    if ends_with_break {
        lines.pop();
    }

    for line in lines {
        let depth = line.bytes().take_while(|byte| *byte == b'>').count();
        let mut content = &line[depth..];

        if let Some(stuffed) = content.strip_prefix(' ') {
            content = stuffed;
        }

        let flowed = content.ends_with(' ') && content != "-- ";

        match &mut paragraph {
            Some((open, buffer)) if *open == depth => buffer.push_str(content),
            Some(_) => {
                let (open, buffer) = paragraph.take().unwrap_or_default();
                push_line(&mut out, open, &buffer, eol);
                paragraph = Some((depth, String::from(content)));
            }
            None => paragraph = Some((depth, String::from(content))),
        }

        if flowed {
            if delsp && let Some((_, buffer)) = &mut paragraph {
                buffer.pop();
            }
        } else if let Some((open, buffer)) = paragraph.take() {
            push_line(&mut out, open, &buffer, eol);
        }
    }

    if let Some((open, buffer)) = paragraph.take() {
        push_line(&mut out, open, &buffer, eol);
    }

    if !ends_with_break && out.ends_with(eol) {
        out.truncate(out.len() - eol.len());
    }

    out
}

/// Wraps text as flowed, `DelSp=no`: every line longer than `width` breaks
/// after a space, which stays to mark the soft break; trailing spaces of a
/// hard line are removed, since one would read as a soft break; a line
/// starting with a space, `>` or `From ` is space-stuffed.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc3676#section-4.1>
pub fn encode(text: &str, width: usize, eol: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / width.max(1));
    let ends_with_break = text.ends_with('\n');
    let mut lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();

    if ends_with_break {
        lines.pop();
    }

    for (index, line) in lines.iter().enumerate() {
        let line = if *line == "-- " {
            *line
        } else {
            line.trim_end_matches(' ')
        };
        let mut rest = line;

        loop {
            let cut = if rest.len() > width {
                rest[..width]
                    .rfind(' ')
                    .map(|space| space + 1)
                    .filter(|cut| *cut > 0)
            } else {
                None
            };

            let piece = match cut {
                Some(cut) => &rest[..cut],
                None => rest,
            };

            if piece.starts_with(' ') || piece.starts_with('>') || piece.starts_with("From ") {
                out.push(' ');
            }

            out.push_str(piece);

            match cut {
                Some(cut) => {
                    out.push_str(eol);
                    rest = &rest[cut..];
                }
                None => break,
            }
        }

        if index + 1 < lines.len() || ends_with_break {
            out.push_str(eol);
        }
    }

    out
}

fn push_line(out: &mut String, depth: usize, content: &str, eol: &str) {
    if depth > 0 {
        for _ in 0..depth {
            out.push('>');
        }

        out.push(' ');
    }

    out.push_str(content);
    out.push_str(eol);
}

#[cfg(test)]
mod tests {
    use crate::codec::flowed::*;

    #[test]
    fn soft_breaks_join_and_delsp_removes_the_space() {
        assert_eq!(decode("Hello wor \r\nld\r\n", true), "Hello world\r\n");
        assert_eq!(decode("Hello \r\nworld\r\n", false), "Hello world\r\n");
    }

    #[test]
    fn quote_depth_splits_paragraphs_and_the_signature_never_flows() {
        assert_eq!(
            decode(">a \n>b\nc \n-- \nsig\n", false),
            "> a b\nc -- \nsig\n"
        );
        assert_eq!(decode("-- \nsig\n", false), "-- \nsig\n");
    }

    #[test]
    fn space_stuffing_is_undone() {
        assert_eq!(
            decode("  indented\n From here\n", false),
            " indented\nFrom here\n"
        );
    }

    #[test]
    fn encoding_reads_back() {
        let text = "A rather long line of text that has to wrap somewhere before the margin, really.\nFrom here\n";
        let flowed = encode(text, 40, "\r\n");

        assert!(flowed.split("\r\n").all(|line| line.len() <= 41));
        assert_eq!(decode(&flowed, false).replace("\r\n", "\n"), text);
    }
}
