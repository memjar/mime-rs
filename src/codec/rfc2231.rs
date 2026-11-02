//! # Parameter value continuations
//!
//! RFC 2231 parameter values: continuations (`name*0`, `name*1`...) joined
//! in section order, and the `charset'language'` prefixed, percent-encoded
//! form (`name*=utf-8''%e2%98%95`) decoded.
//!
//! Each section counts once and at most [`SECTIONS_MAX`] are joined, so a
//! hostile value cannot make the join quadratic or unbounded.

use alloc::{borrow::Cow, string::String, vec::Vec};

use crate::{
    codec::{charset, rfc2047},
    value::content_type::MimeParam,
};

/// The most sections one parameter joins.
pub const SECTIONS_MAX: u32 = 1_000;

/// One parameter as read from the wire, before continuations are joined.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimeRawParam<'v> {
    /// The name without its RFC 2231 suffix.
    pub name: &'v [u8],
    /// The section number of a continuation, `None` for a whole value.
    pub section: Option<u32>,
    /// Whether the name ended in `*`, the value being percent-encoded.
    pub encoded: bool,
    /// The value, unquoted.
    pub value: Cow<'v, [u8]>,
}

impl<'v> MimeRawParam<'v> {
    /// Splits a raw name (`name`, `name*`, `name*2`, `name*2*`) into its
    /// base, section and encoding marker.
    pub fn new(name: &'v [u8], value: Cow<'v, [u8]>) -> Self {
        let (name, encoded) = match name.strip_suffix(b"*") {
            Some(name) => (name, true),
            None => (name, false),
        };

        let star = name.iter().rposition(|byte| *byte == b'*');
        let section = star.and_then(|star| {
            let digits = &name[star + 1..];
            let all_digits = !digits.is_empty() && digits.iter().all(u8::is_ascii_digit);

            all_digits
                .then(|| core::str::from_utf8(digits).ok()?.parse::<u32>().ok())
                .flatten()
        });

        let name = match (star, section) {
            (Some(star), Some(_)) => &name[..star],
            _ => name,
        };

        Self {
            name,
            section,
            encoded,
            value,
        }
    }
}

/// Joins raw parameters into decoded ones, in the order their names first
/// appear.
///
/// The sections of one name are joined in section order, each counted
/// once; an RFC 2231 value wins over a plain one of the same name; a plain
/// value written as RFC 2047 encoded words, which RFC 2047 5 forbids and
/// real mailers send, is decoded too.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc2231#section-3>
pub fn join(raw: Vec<MimeRawParam<'_>>) -> Vec<MimeParam> {
    // NOTE: the common case, no continuation and no encoding, joins nothing.
    if raw
        .iter()
        .all(|param| !param.encoded && param.section.is_none())
    {
        let mut params: Vec<MimeParam> = Vec::with_capacity(raw.len());

        for param in &raw {
            if !params
                .iter()
                .any(|known| known.name.as_bytes().eq_ignore_ascii_case(param.name))
            {
                params.push(MimeParam {
                    name: String::from_utf8_lossy(param.name).into_owned(),
                    value: plain(&param.value),
                });
            }
        }

        return params;
    }

    let mut names: Vec<&[u8]> = Vec::new();

    for param in &raw {
        if !names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(param.name))
        {
            names.push(param.name);
        }
    }

    names
        .into_iter()
        .map(|name| {
            let mut same: Vec<&MimeRawParam<'_>> = raw
                .iter()
                .filter(|param| param.name.eq_ignore_ascii_case(name))
                .collect();

            let extended = same
                .iter()
                .any(|param| param.encoded || param.section.is_some());

            let value = if extended {
                same.retain(|param| param.encoded || param.section.is_some());
                join_sections(same)
            } else {
                plain(&same[0].value)
            };

            MimeParam {
                name: String::from_utf8_lossy(name).into_owned(),
                value,
            }
        })
        .collect()
}

fn join_sections(mut sections: Vec<&MimeRawParam<'_>>) -> String {
    sections.sort_by_key(|param| param.section.unwrap_or(0));
    sections.dedup_by_key(|param| param.section.unwrap_or(0));
    sections.truncate(SECTIONS_MAX as usize);

    let mut label: Option<String> = None;
    let mut out = String::new();
    let mut encoded = Vec::new();

    for section in &sections {
        if !section.encoded {
            flush(&label, &mut encoded, &mut out);
            out.push_str(&String::from_utf8_lossy(&section.value));
            continue;
        }

        let mut value: &[u8] = &section.value;

        // NOTE: RFC 2231 4.1 puts the prefix on the first section only;
        // real mailers put it on the first encoded one, and repeat it.
        match &label {
            None => {
                let mut quotes = value.splitn(3, |byte| *byte == b'\'');

                if let (Some(charset), Some(_language), Some(rest)) =
                    (quotes.next(), quotes.next(), quotes.next())
                {
                    label = Some(String::from_utf8_lossy(charset).into_owned());
                    value = rest;
                }
            }
            Some(known) => {
                if let Some(rest) = value
                    .strip_prefix(known.as_bytes())
                    .and_then(|rest| rest.strip_prefix(b"'"))
                {
                    let language = rest.iter().position(|byte| *byte == b'\'');
                    let percent = rest.iter().position(|byte| *byte == b'%');

                    value = match (language, percent) {
                        (Some(quote), Some(percent)) if quote < percent => &rest[quote + 1..],
                        (Some(quote), None) => &rest[quote + 1..],
                        _ => rest,
                    };
                }
            }
        }

        percent_decode(value, &mut encoded, false);
    }

    flush(&label, &mut encoded, &mut out);
    out
}

/// Transcodes the pending percent-decoded bytes from the label's charset.
fn flush(label: &Option<String>, encoded: &mut Vec<u8>, out: &mut String) {
    if encoded.is_empty() {
        return;
    }

    let label = label
        .as_deref()
        .filter(|label| !label.is_empty())
        .unwrap_or("utf-8");

    match charset::transcode(label, encoded) {
        Some(decoded) => out.push_str(&decoded.text),
        None => out.push_str(&String::from_utf8_lossy(encoded)),
    }

    encoded.clear();
}

fn plain(value: &[u8]) -> String {
    if rfc2047::contains_word_start(value) {
        return String::from(rfc2047::decode_text(value).trim_matches([' ', '\t']));
    }

    String::from_utf8_lossy(value).into_owned()
}

/// Appends `value` to `out`, `%XX` sequences decoded and any other byte
/// kept as written; a broken `%` is kept with `keep_broken`, dropped
/// otherwise, as in a value RFC 2231 says is percent-encoded.
pub fn percent_decode(value: &[u8], out: &mut Vec<u8>, keep_broken: bool) {
    let mut i = 0;

    while i < value.len() {
        if value[i] == b'%'
            && let (Some(high), Some(low)) = (
                value.get(i + 1).and_then(|b| (*b as char).to_digit(16)),
                value.get(i + 2).and_then(|b| (*b as char).to_digit(16)),
            )
        {
            out.push((high << 4 | low) as u8);
            i += 3;
            continue;
        }

        if value[i] != b'%' || keep_broken {
            out.push(value[i]);
        }

        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use alloc::{borrow::Cow, vec};

    use crate::codec::rfc2231::*;

    fn raw<'v>(name: &'v str, value: &'v str) -> MimeRawParam<'v> {
        MimeRawParam::new(name.as_bytes(), Cow::Borrowed(value.as_bytes()))
    }

    #[test]
    fn three_sections_join_in_order_and_decode() {
        let params = join(vec![
            raw("name*1", "about "),
            raw("name*0", "Book "),
            raw("name*2*", "utf-8''%e2%98%95 tables.gif"),
        ]);

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "name");
        assert_eq!(params[0].value, "Book about ☕ tables.gif");
    }

    #[test]
    fn an_extended_value_beats_a_plain_one() {
        let params = join(vec![
            raw("filename", "fallback.txt"),
            raw("filename*", "utf-8''caf%C3%A9.txt"),
        ]);

        assert_eq!(params[0].value, "café.txt");
    }

    #[test]
    fn a_duplicate_section_counts_once() {
        let params = join(vec![raw("a*0", "x"), raw("a*0", "y"), raw("a*1", "z")]);
        assert_eq!(params[0].value, "xz");
    }
}
