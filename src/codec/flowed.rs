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
    let (lines, ends_with_break) = lines(text);

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
/// hard line are removed, since one would read as a soft break, and so are
/// trailing CRs under an LF `eol`, which would not read back as content; a
/// line starting with a space, `>` or `From ` is space-stuffed.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc3676#section-4.1>
pub fn encode(text: &str, width: usize, eol: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / width.max(1));
    let (lines, ends_with_break) = lines(text);

    for (index, line) in lines.iter().enumerate() {
        let line = if *line == "-- " {
            *line
        } else {
            line.trim_end_matches(|c| c == ' ' || (c == '\r' && eol == "\n"))
        };
        let mut rest = line;

        loop {
            let cut = if rest.len() > width {
                let mut edge = width;

                while !rest.is_char_boundary(edge) {
                    edge -= 1;
                }

                rest[..edge]
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

/// The lines of text, each without its line ending (an LF, with the CR
/// before it), and whether the text ends with a line ending; a CR not
/// before an LF is content.
fn lines(text: &str) -> (Vec<&str>, bool) {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let last = lines.pop().unwrap_or_default();

    for line in &mut lines {
        *line = line.strip_suffix('\r').unwrap_or(line);
    }

    let ends_with_break = last.is_empty() && !lines.is_empty();

    if !ends_with_break {
        lines.push(last);
    }

    (lines, ends_with_break)
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
    use alloc::format;

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
    fn a_wide_character_at_the_margin_is_not_split() {
        let text = format!("{} é{}\n", "a".repeat(70), "b".repeat(20));
        let flowed = encode(&text, 72, "\n");
        assert_eq!(decode(&flowed, false), text);
    }

    #[test]
    fn encoding_reads_back() {
        let text = "A rather long line of text that has to wrap somewhere before the margin, really.\nFrom here\n";
        let flowed = encode(text, 40, "\r\n");

        assert!(flowed.split("\r\n").all(|line| line.len() <= 41));
        assert_eq!(decode(&flowed, false).replace("\r\n", "\n"), text);
    }
}
