//! # Address reader
//!
//! The RFC 5322 3.4 address list grammar with its obsolete syntax, read
//! liberally: groups anywhere (RFC 6854) and without their `;`, comments
//! anywhere, routes dropped, UTF-8 throughout (RFC 6532), encoded words
//! decoded in names and comments (RFC 2047 5).
//!
//! Comments fold into the name: in parentheses after it, or as the name
//! when there is none, which is how `addr (Name)` is read. A bare word with
//! no `@` is an address when it stands alone and a name otherwise.

use alloc::{string::String, vec::Vec};
use core::ops::Range;

use crate::{
    codec::rfc2047,
    tree::field::lex::Lexer,
    value::address::{MimeAddress, MimeAddressList, MimeGroup, MimeMailbox},
};

/// Reads an address list, groups kept as written.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.4>
pub fn parse_addresses(value: &[u8]) -> MimeAddressList {
    let mut lex = Lexer::new(value);
    let mut items = Vec::new();
    let mut group: Option<MimeGroup> = None;
    let mut mailbox = Pending::default();

    loop {
        lex.skip_fws();

        let Some(byte) = lex.peek() else {
            break;
        };

        match byte {
            b'(' => {
                let comment = lex.comment();
                mailbox.comments.push(comment_text(comment));
            }
            b'"' => {
                let quoted = lex.quoted();
                mailbox.words.push(Word::Quoted(quoted));
            }
            b'<' => {
                lex.bump();
                mailbox.address = Some(angle(&mut lex, &mut mailbox.comments));
            }
            b',' => {
                lex.bump();
                finish(&mut mailbox, &mut group, &mut items);
            }
            b';' => {
                lex.bump();
                finish(&mut mailbox, &mut group, &mut items);

                if let Some(group) = group.take() {
                    items.push(MimeAddress::Group(group));
                }
            }
            b':' => {
                lex.bump();

                if mailbox.address.is_some()
                    || (mailbox.words.is_empty() && mailbox.comments.is_empty())
                {
                    continue;
                }

                if let Some(group) = group.take() {
                    items.push(MimeAddress::Group(group));
                }

                let name = core::mem::take(&mut mailbox).name();
                group = Some(MimeGroup {
                    name,
                    mailboxes: Vec::new(),
                });
            }
            b'>' => {
                lex.bump();
            }
            _ => {
                let word = atom(&mut lex);

                if word.is_empty() {
                    lex.bump();
                } else {
                    mailbox.words.push(Word::Atom(word));
                }
            }
        }
    }

    finish(&mut mailbox, &mut group, &mut items);

    if let Some(group) = group {
        items.push(MimeAddress::Group(group));
    }

    MimeAddressList { items }
}

/// Reads an address list with every group flattened into its mailboxes.
///
/// The RFC 8621 4.1.2.3 addresses form.
pub fn parse_mailboxes(value: &[u8]) -> MimeAddressList {
    parse_addresses(value).mailboxes().cloned().collect()
}

/// The byte ranges of an address list's items (mailboxes and groups).
///
/// Split at the commas outside quoted strings, comments, angle addresses and
/// groups, blanks and folds included, commas excluded. An empty item between
/// two commas is an empty range, so ranges and commas alternate.
pub(crate) fn item_spans(value: &[u8]) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut depth = 0usize;
    let mut angle = false;
    let mut group = false;
    let mut i = 0;

    while let Some(&byte) = value.get(i) {
        if quoted || depth > 0 {
            match byte {
                b'\\' => i += 1,
                b'"' if quoted => quoted = false,
                b'(' if depth > 0 => depth += 1,
                b')' if depth > 0 => depth -= 1,
                _ => {}
            }

            i += 1;
            continue;
        }

        match byte {
            b'"' => quoted = true,
            b'(' => depth = 1,
            b'<' => angle = true,
            b'>' => angle = false,
            b':' if !angle && !group => group = true,
            b';' if group => group = false,
            b',' if !angle && !group => {
                spans.push(start..i);
                start = i + 1;
            }
            _ => {}
        }

        i += 1;
    }

    spans.push(start..value.len());
    spans
}

/// Whether an item range holds anything but blanks and folds.
pub(crate) fn is_item(value: &[u8]) -> bool {
    value
        .iter()
        .any(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
}

#[derive(Debug)]
enum Word {
    Atom(Vec<u8>),
    Quoted(Vec<u8>),
}

#[derive(Debug, Default)]
struct Pending {
    words: Vec<Word>,
    comments: Vec<String>,
    address: Option<String>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        self.words.is_empty() && self.comments.is_empty() && self.address.is_none()
    }

    /// The phrase and comments as one name.
    fn name(self) -> Option<String> {
        let phrase = phrase(&self.words);
        name(phrase, &self.comments)
    }

    /// Splits bare words holding several addresses with no comma between.
    ///
    /// As `a@b.c\r\n\tjane@y.z`: one mailbox per address is handed to `push`,
    /// the comments staying with the first.
    fn split(self, mut push: impl FnMut(Self)) {
        let is_at = |word: &Word| matches!(word, Word::Atom(atom) if atom.contains(&b'@'));

        if self.address.is_some() || self.words.iter().filter(|word| is_at(word)).count() < 2 {
            return push(self);
        }

        let mut comments = Some(self.comments);
        let mut current = Vec::new();
        let mut seen_at = false;

        for word in self.words {
            let joined = current.last().is_some_and(|last| joins(last, &word));

            if seen_at && !joined {
                push(Self {
                    words: core::mem::take(&mut current),
                    comments: comments.take().unwrap_or_default(),
                    address: None,
                });
                seen_at = false;
            }

            seen_at |= is_at(&word);
            current.push(word);
        }

        push(Self {
            words: current,
            comments: comments.take().unwrap_or_default(),
            address: None,
        });
    }

    fn mailbox(mut self) -> MimeMailbox {
        if self.address.is_some() {
            let address = self.address.take();
            return MimeMailbox {
                name: self.name(),
                address,
            };
        }

        let has_at = self
            .words
            .iter()
            .any(|word| matches!(word, Word::Atom(atom) if atom.contains(&b'@')));
        let single_atom = matches!(self.words.as_slice(), [Word::Atom(atom)] if !rfc2047::contains_word_start(atom));

        if has_at || single_atom {
            // NOTE: the address is the word holding the `@`, with the words a
            // dot joins to it (`jdoe@test . example`); the words before it,
            // as in `John Smith john@example.com`, are the name.
            let at = self
                .words
                .iter()
                .position(|word| matches!(word, Word::Atom(atom) if atom.contains(&b'@')))
                .unwrap_or(0);

            let mut start = at;
            let mut end = at + 1;

            while start > 0 && joins(&self.words[start - 1], &self.words[start]) {
                start -= 1;
            }

            while end < self.words.len() && joins(&self.words[end - 1], &self.words[end]) {
                end += 1;
            }

            let mut address = String::new();

            for word in &self.words[start..end] {
                match word {
                    Word::Atom(atom) => address.push_str(&String::from_utf8_lossy(atom)),
                    Word::Quoted(quoted) => {
                        address.push('"');
                        address.push_str(&String::from_utf8_lossy(quoted));
                        address.push('"');
                    }
                }
            }

            let mut rest = self.words;
            rest.truncate(start);

            return MimeMailbox {
                name: name(phrase(&rest), &self.comments),
                address: Some(address),
            };
        }

        MimeMailbox {
            name: self.name(),
            address: None,
        }
    }
}

fn finish(mailbox: &mut Pending, group: &mut Option<MimeGroup>, items: &mut Vec<MimeAddress>) {
    if mailbox.is_empty() {
        return;
    }

    core::mem::take(mailbox).split(|pending| {
        let mailbox = pending.mailbox();

        match group {
            Some(group) => group.mailboxes.push(mailbox),
            None => items.push(MimeAddress::Mailbox(mailbox)),
        }
    });
}

/// The phrase words joined by single spaces, encoded words decoded.
///
/// Even inside quoted strings, as real mailers write them.
fn phrase(words: &[Word]) -> String {
    let mut raw = Vec::new();

    for word in words {
        if !raw.is_empty() {
            raw.push(b' ');
        }

        match word {
            Word::Atom(atom) | Word::Quoted(atom) => raw.extend_from_slice(atom),
        }
    }

    String::from(rfc2047::decode_text(&raw).trim_matches([' ', '\t']))
}

fn name(phrase: String, comments: &[String]) -> Option<String> {
    let comments: Vec<&str> = comments
        .iter()
        .map(String::as_str)
        .filter(|c| !c.is_empty())
        .collect();

    match (phrase.is_empty(), comments.is_empty()) {
        (true, true) => None,
        (false, true) => Some(phrase),
        (true, false) => Some(comments.join(" ")),
        (false, false) => {
            let mut name = phrase;
            name.push_str(" (");
            name.push_str(&comments.join(" "));
            name.push(')');
            Some(name)
        }
    }
}

/// Reads an angle address after its `<`, an obsolete route dropped.
///
/// Blanks and comments are dropped, the comments kept for the name.
fn angle(lex: &mut Lexer<'_>, comments: &mut Vec<String>) -> String {
    let mut address = Vec::new();

    while let Some(byte) = lex.peek() {
        match byte {
            b'>' => {
                lex.bump();
                break;
            }
            b'(' => {
                let comment = lex.comment();
                comments.push(comment_text(comment));
            }
            b'"' => {
                address.push(b'"');
                address.extend_from_slice(&lex.quoted());
                address.push(b'"');
            }
            b' ' | b'\t' | b'\r' | b'\n' => {
                lex.bump();
            }
            _ => {
                address.extend_from_slice(lex.until(|byte| {
                    matches!(byte, b'>' | b'(' | b'"' | b' ' | b'\t' | b'\r' | b'\n')
                }));
            }
        }
    }

    // NOTE: an obsolete route (`@a,@b:`) names relays, not the mailbox.
    if address.first() == Some(&b'@')
        && let Some(colon) = address.iter().rposition(|byte| *byte == b':')
    {
        address.drain(..=colon);
    }

    String::from_utf8(address)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

/// Whether two adjacent words of a bare address belong together.
///
/// They do when one ends or the next starts with a dot or an `@`.
fn joins(before: &Word, after: &Word) -> bool {
    let edge = |word: &Word, last: bool| match word {
        Word::Atom(atom) => if last { atom.last() } else { atom.first() }
            .is_some_and(|byte| matches!(byte, b'.' | b'@')),
        Word::Quoted(_) => false,
    };

    edge(before, true) || edge(after, false)
}

/// Reads an atom, a quoted pair outside quotes (`\(`) taken literally.
///
/// The RFC 5322 4.4 obsolete phrase allows it.
fn atom(lex: &mut Lexer<'_>) -> Vec<u8> {
    let mut word = Vec::new();

    loop {
        word.extend_from_slice(lex.until(|byte| {
            matches!(
                byte,
                b'\\'
                    | b'('
                    | b')'
                    | b'<'
                    | b'>'
                    | b','
                    | b';'
                    | b':'
                    | b'"'
                    | b' '
                    | b'\t'
                    | b'\r'
                    | b'\n'
            )
        }));

        if lex.peek() != Some(b'\\') {
            return word;
        }

        lex.bump();
        word.push(lex.bump().unwrap_or(b'\\'));
    }
}

/// A comment's text: escapes undone, folds collapsed, encoded words decoded.
fn comment_text(raw: &[u8]) -> String {
    let mut unescaped = Vec::with_capacity(raw.len());
    let mut i = 0;

    while let Some(&byte) = raw.get(i) {
        if byte == b'\\' && i + 1 < raw.len() {
            unescaped.push(raw[i + 1]);
            i += 2;
        } else {
            unescaped.push(byte);
            i += 1;
        }
    }

    String::from(rfc2047::decode_text(&unescaped).trim_matches([' ', '\t']))
}

#[cfg(test)]
mod tests {
    use crate::{tree::field::address::*, value::address::MimeMailbox};

    fn mailboxes(value: &str) -> Vec<MimeMailbox> {
        parse_addresses(value.as_bytes())
            .mailboxes()
            .cloned()
            .collect()
    }

    #[test]
    fn names_quotes_and_comments() {
        assert_eq!(
            mailboxes("Art Vandelay <art@vandelay.com> (Vandelay Industries)"),
            [MimeMailbox::named(
                "Art Vandelay (Vandelay Industries)",
                "art@vandelay.com"
            )]
        );
        assert_eq!(
            mailboxes("\"Giant; \\\"Big\\\" Box\" <sysservices@example.net>"),
            [MimeMailbox::named(
                "Giant; \"Big\" Box",
                "sysservices@example.net"
            )]
        );
        assert_eq!(
            mailboxes("postmaster (The Postmaster)"),
            [MimeMailbox::named("The Postmaster", "postmaster")]
        );
    }

    #[test]
    fn groups_keep_document_order() {
        let list = parse_addresses(b"a@x.test, Friends: b@x.test, c@x.test;, d@x.test");
        let flat: Vec<_> = list
            .mailboxes()
            .filter_map(|m| m.address.as_deref())
            .collect();

        assert_eq!(flat, ["a@x.test", "b@x.test", "c@x.test", "d@x.test"]);
        assert!(
            matches!(&list.items[1], MimeAddress::Group(group) if group.name.as_deref() == Some("Friends"))
        );
        assert_eq!(list.items.len(), 3);
    }

    #[test]
    fn a_route_is_dropped() {
        assert_eq!(
            mailboxes("Mary <@node.test:mary@example.net>"),
            [MimeMailbox::named("Mary", "mary@example.net")]
        );
    }

    #[cfg(feature = "encoding")]
    #[test]
    fn a_latin_1_encoded_word_decodes_in_a_name() {
        assert_eq!(
            mailboxes("=?ISO-8859-1?Q?Andr=E9?= Pirard <PIRARD@vm1.ulg.ac.be>"),
            [MimeMailbox::named("André Pirard", "PIRARD@vm1.ulg.ac.be")]
        );
    }

    #[test]
    fn a_group_opens_without_the_previous_semicolon() {
        let list = parse_addresses(b" one: a@b.c, two: d@e.f");
        assert_eq!(list.items.len(), 2);
    }
}
