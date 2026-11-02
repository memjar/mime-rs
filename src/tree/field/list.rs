//! # List readers
//!
//! The list-shaped field values: message identifiers (RFC 5322 3.6.4), URLs
//! in angle brackets (RFC 2369 2) and comma-separated phrases (RFC 5322
//! 3.6.5 keywords, RFC 3282 language tags).

use alloc::{string::String, vec::Vec};

use crate::{codec::rfc2047, tree::field::lex::Lexer};

/// Reads message identifiers in order, without their angle brackets,
/// comments and blanks skipped, an identifier written without brackets
/// kept whole.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.6.4>
pub fn parse_message_ids(value: &[u8]) -> Vec<String> {
    let mut lex = Lexer::new(value);
    let mut ids = Vec::new();
    let mut bare = Vec::new();

    loop {
        lex.skip_cfws();

        match lex.peek() {
            None => break,
            Some(b'<') => {
                while lex.peek() == Some(b'<') {
                    lex.bump();
                }

                let id = lex.until(|byte| byte == b'>');

                // NOTE: an unclosed bracket at the end holds no identifier.
                if lex.bump().is_some() {
                    push_id(id, &mut ids);
                }
            }
            Some(b',') => {
                lex.bump();
            }
            Some(_) => {
                let id = lex.until(|byte| {
                    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'<' | b'(' | b',')
                });
                push_id(id, &mut bare);
            }
        }
    }

    // NOTE: words outside brackets are identifiers only when no bracketed
    // one exists, as a broken mailer writes them.
    if ids.is_empty() { bare } else { ids }
}

/// Reads the URLs of an RFC 2369 list in order, without their angle
/// brackets; anything outside brackets is ignored.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2369#section-2>
pub fn parse_urls(value: &[u8]) -> Vec<String> {
    let mut lex = Lexer::new(value);
    let mut urls = Vec::new();

    loop {
        lex.skip_cfws();

        match lex.peek() {
            None => break,
            Some(b'<') => {
                lex.bump();
                let url: Vec<u8> = lex
                    .until(|byte| byte == b'>')
                    .iter()
                    .copied()
                    .filter(|byte| !byte.is_ascii_whitespace())
                    .collect();
                lex.bump();

                if !url.is_empty() {
                    urls.push(String::from_utf8_lossy(&url).into_owned());
                }
            }
            Some(_) => {
                lex.bump();
            }
        }
    }

    urls
}

/// Reads comma-separated phrases, each decoded and trimmed, empty ones
/// dropped; a comma inside a quoted string separates nothing.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.6.5>
pub fn parse_comma_list(value: &[u8]) -> Vec<String> {
    let mut items = Vec::new();
    let mut item = Vec::new();
    let mut lex = Lexer::new(value);

    while let Some(byte) = lex.peek() {
        // NOTE: an encoded word may hold a comma, so it is read whole.
        if byte == b'='
            && let Some(end) = rfc2047::word_end(lex.bytes, lex.pos)
        {
            item.extend_from_slice(&lex.bytes[lex.pos..end]);
            lex.pos = end;
            continue;
        }

        match byte {
            b',' => {
                lex.bump();
                push_item(&item, &mut items);
                item.clear();
            }
            b'"' => item.extend_from_slice(&lex.quoted()),
            _ => {
                item.push(byte);
                lex.bump();
            }
        }
    }

    push_item(&item, &mut items);
    items
}

fn push_id(id: &[u8], ids: &mut Vec<String>) {
    let mut id = id.to_vec();
    id.retain(|byte| !byte.is_ascii_whitespace());

    if !id.is_empty() {
        ids.push(
            String::from_utf8(id)
                .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned()),
        );
    }
}

fn push_item(item: &[u8], items: &mut Vec<String>) {
    let text = rfc2047::decode_text(item);

    if !text.is_empty() {
        items.push(text.into_owned());
    }
}
