//! # Base subject
//!
//! The RFC 5256 2.1 base subject: a subject stripped of the reply and
//! forward markers mailers add, so replies group with what they answer.

use alloc::string::String;

/// The reply prefixes recognised, lowercased: RFC 5256's `re` and the
/// localized spellings mailers write.
///
/// NOTE: this table and [`FORWARDS`] come from mail-parser (Apache-2.0 OR
/// MIT, Copyright (C) 2020 Stalwart Labs LLC).
const REPLIES: &[&str] = &[
    "re",
    "res",
    "resp",
    "resposta",
    "respuesta",
    "rsp",
    "ref",
    "réf",
    "rép",
    "rep",
    "réponse",
    "reponse",
    "aw",
    "antw",
    "antwort",
    "antwoord",
    "sv",
    "svar",
    "svara",
    "vast",
    "vastaus",
    "vastus",
    "vá",
    "va",
    "válasz",
    "valasz",
    "r",
    "ri",
    "rif",
    "odp",
    "odpowiedź",
    "odpowiedz",
    "odpov",
    "odg",
    "odgovor",
    "απ",
    "σχετ",
    "απάντηση",
    "ynt",
    "yn",
    "yan",
    "yanıt",
    "yanit",
    "atb",
    "atb.",
    "atbilde",
    "ateb",
    "ats",
    "ats.",
    "atsakymas",
    "bls",
    "balas",
    "השב",
    "תשובה",
    "رد",
    "پاسخ",
    "отв",
    "ответ",
    "відп",
    "відповідь",
    "отг",
    "отговор",
    "回复",
    "回覆",
    "答复",
    "答覆",
    "返信",
    "회신",
    "답장",
    "ตอบกลับ",
    "பதில்",
    "trả lời",
];

/// The forward prefixes recognised, lowercased.
const FORWARDS: &[&str] = &[
    "fwd",
    "fw",
    "rv",
    "reenviado",
    "enc",
    "encaminhado",
    "tr",
    "transfert",
    "wg",
    "weitergeleitet",
    "doorst",
    "doorgestuurd",
    "vs",
    "videresendt",
    "vb",
    "vidarebefordrat",
    "vl",
    "välitetty",
    "ed",
    "edastatud",
    "fs",
    "framsenda",
    "i",
    "inoltro",
    "inoltrato",
    "pd",
    "podaj dalej",
    "przekazane",
    "prosl",
    "proslijeđeno",
    "πρθ",
    "προωθ",
    "προωθημένο",
    "i\u{307}lt",
    "ilt",
    "i\u{307}let",
    "ilet",
    "i\u{307}letilen",
    "iletilen",
    "yml",
    "ymlaen",
    "pārs",
    "pārs.",
    "pārsūtīts",
    "persiųsta",
    "trs",
    "terusan",
    "teruskan",
    "továbbítás",
    "הועבר",
    "העברה",
    "إعادة توجيه",
    "перес",
    "переслано",
    "пересылка",
    "пре",
    "препратено",
    "转发",
    "转寄",
    "轉寄",
    "転送",
    "전달",
    "ส่งต่อ",
    "முன்னனுப்பு",
    "chuyển tiếp",
];

/// The longest prefix word looked at.
const PREFIX_MAX: usize = 40;

/// The base subject of `subject`: leading reply and forward prefixes
/// (`Re:`, `Fwd:`, `Re[2]:`, their localized spellings, and any word of
/// four letters or fewer before a colon), bracketed list tags before them,
/// `[fwd: ...]` wrappers and trailing `(fwd)` markers removed, repeatedly,
/// blanks trimmed. Borrows from `subject`.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5256#section-2.1>
pub fn base_subject(subject: &str) -> &str {
    let mut text = subject.trim();

    loop {
        let before = text;

        text = strip_trailers(text);
        text = strip_leaders(text);
        text = strip_wrapper(text);

        if text.len() == before.len() {
            return text;
        }
    }
}

/// Removes trailing `(fwd)` markers and blanks (RFC 5256 2.1 step 2).
fn strip_trailers(mut text: &str) -> &str {
    loop {
        text = text.trim_end();

        // NOTE: `(fwd)` and its localized spellings, as `(wg)`.
        let marker = text
            .strip_suffix(')')
            .and_then(|head| head.rfind('(').map(|open| (open, &head[open + 1..])))
            .filter(|(_, word)| FORWARDS.contains(&lowercase(word.trim()).as_str()));

        match marker {
            Some((open, _)) => text = &text[..open],
            None => return text,
        }
    }
}

/// Removes leading list tags and reply or forward prefixes (RFC 5256 2.1
/// step 3), a tag only when something follows it.
fn strip_leaders(mut text: &str) -> &str {
    loop {
        text = text.trim_start();

        if let Some(rest) = strip_blob(text)
            && !only_blobs(rest)
        {
            text = rest;
            continue;
        }

        match strip_prefix(text) {
            Some(rest) => text = rest,
            None => return text,
        }
    }
}

/// Removes a `[...]` tag, `None` when the text does not start with one.
fn strip_blob(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('[')?;
    let end = rest.find([']', '['])?;

    (rest[end..].starts_with(']')).then(|| &rest[end + 1..])
}

/// Removes one reply or forward prefix with its counter and colon.
fn strip_prefix(text: &str) -> Option<&str> {
    let end = text.find([':', '[', '('])?;
    let word = text[..end].trim_end();

    if word.is_empty() || word.len() > PREFIX_MAX || !is_prefix(word) {
        return None;
    }

    let mut rest = &text[end..];

    // NOTE: a counter, `Re[2]:` or `Re(2):`.
    for (open, close) in [('[', ']'), ('(', ')')] {
        if let Some(inner) = rest.strip_prefix(open) {
            let stop = inner.find(close)?;
            rest = inner[stop + 1..].trim_start();
        }
    }

    let rest = rest.strip_prefix(':')?;

    // NOTE: `http://` is a link, not a prefix.
    if rest.starts_with("//") {
        return None;
    }

    Some(rest)
}

/// Removes a `[fwd: ...]` wrapper (RFC 5256 2.1 step 5).
fn strip_wrapper(text: &str) -> &str {
    let Some(inner) = text
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    else {
        return text;
    };

    match strip_prefix(inner) {
        Some(rest)
            if FORWARDS
                .contains(&lowercase(inner[..inner.find(':').unwrap_or(0)].trim()).as_str()) =>
        {
            rest.trim()
        }
        _ => text,
    }
}

/// Whether `text` holds nothing but `[...]` tags and blanks.
fn only_blobs(mut text: &str) -> bool {
    loop {
        text = text.trim_start();

        if text.is_empty() {
            return true;
        }

        match strip_blob(text) {
            Some(rest) => text = rest,
            None => return false,
        }
    }
}

fn is_prefix(word: &str) -> bool {
    let lower = lowercase(word);

    if REPLIES.contains(&lower.as_str()) || FORWARDS.contains(&lower.as_str()) {
        return true;
    }

    let letters = word.chars().count();
    letters <= 4 && word.chars().all(char::is_alphabetic)
}

fn lowercase(word: &str) -> String {
    word.chars().flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use crate::value::subject::*;

    #[test]
    fn prefixes_tags_and_trailers_go() {
        assert_eq!(
            base_subject("Re: Fwd: [list] Lunch on Friday"),
            "Lunch on Friday"
        );
        assert_eq!(base_subject("fwd[5]:re[5]: hello"), "hello");
        assert_eq!(base_subject("hello (fwd)(fwd)"), "hello");
        assert_eq!(base_subject("[fwd: hello]"), "hello");
        assert_eq!(base_subject("AW: WG: Termin"), "Termin");
    }

    #[test]
    fn what_is_not_a_prefix_stays() {
        assert_eq!(base_subject("http://example.com"), "http://example.com");
        assert_eq!(
            base_subject("Meeting notes: Monday"),
            "Meeting notes: Monday"
        );
        assert_eq!(base_subject("[only a tag]"), "[only a tag]");
    }
}
