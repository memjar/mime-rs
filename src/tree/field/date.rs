//! # Date reader
//!
//! The RFC 5322 3.3 date-time with the RFC 5322 4.3 obsolete syntax, read
//! liberally: an optional day name, two- and three-digit years, optional
//! seconds, named and military zones, comments anywhere.

use crate::{tree::field::lex::Lexer, value::datetime::MimeDateTime};

/// Reads a date-time, `None` when the value names no real day and time.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.3>
pub fn parse_date(value: &[u8]) -> Option<MimeDateTime> {
    // NOTE: a Received field carries its date after its last `;`.
    let value = match value.iter().rposition(|byte| *byte == b';') {
        Some(at) => &value[at + 1..],
        None => value,
    };

    let mut lex = Lexer::new(value);
    let mut token = || {
        loop {
            lex.skip_cfws();

            match lex.peek()? {
                b',' => {
                    lex.bump();
                }
                _ => break,
            }
        }

        let first = lex.peek()?;
        let token = if first == b'+' || first == b'-' {
            lex.bump();
            let digits = lex.until(|byte| !byte.is_ascii_digit());
            (Some(first), digits)
        } else {
            let token = lex.until(|byte| {
                matches!(
                    byte,
                    b' ' | b'\t' | b'\r' | b'\n' | b',' | b'(' | b'+' | b'-'
                )
            });

            if token.is_empty() {
                lex.bump();
            }

            (None, token)
        };

        Some(token)
    };

    let mut day = None;
    let mut month = None;
    let mut year = None;
    let mut time = None;
    let mut offset = None;

    while let Some((sign, token)) = token() {
        if let Some(sign) = sign {
            if offset.is_none() && time.is_some() && (1..=4).contains(&token.len()) {
                // NOTE: `+5` and `+530` as broken mailers write them: the
                // hours first, the minutes in the last two digits.
                let split = if token.len() <= 2 {
                    token.len()
                } else {
                    token.len() - 2
                };
                let hours = number(&token[..split])? as i16;
                let minutes = number(&token[split..]).unwrap_or(0) as i16;
                let magnitude = hours * 60 + minutes;
                offset = Some(if sign == b'-' { -magnitude } else { magnitude });
            }

            continue;
        }

        if token.contains(&b':') {
            if time.is_none() {
                time = clock(token);
            }

            continue;
        }

        if token.iter().all(u8::is_ascii_digit) && !token.is_empty() {
            let n = number(token)?;

            if day.is_none() && month.is_none() && token.len() <= 2 {
                day = Some(n);
            } else if month.is_none() && day.is_some() && token.len() <= 2 && (1..=12).contains(&n)
            {
                month = Some(n as u8);
            } else if year.is_none() && (day.is_some() || month.is_some()) {
                year = Some(match token.len() {
                    1 | 2 if n < 50 => n + 2000,
                    1 | 2 => n + 1900,
                    3 => n + 1900,
                    _ => n,
                });
            }

            continue;
        }

        if month.is_none()
            && let Some(m) = month_of(token)
        {
            month = Some(m);
            continue;
        }

        if time.is_some() && offset.is_none() {
            offset = zone(token);
        }
    }

    let (hour, minute, second) = time?;
    let date = MimeDateTime {
        year: u16::try_from(year?).ok()?,
        month: month?,
        day: u8::try_from(day?).ok()?,
        hour,
        minute,
        second,
        offset: offset.unwrap_or(0),
    };

    date.is_valid().then_some(date)
}

fn number(digits: &[u8]) -> Option<u32> {
    core::str::from_utf8(digits).ok()?.parse().ok()
}

fn clock(token: &[u8]) -> Option<(u8, u8, u8)> {
    let mut parts = token.split(|byte| *byte == b':');
    let hour = number(parts.next()?)?;
    let minute = number(parts.next()?)?;
    let second = parts.next().map_or(Some(0), number)?;

    Some((
        u8::try_from(hour).ok()?,
        u8::try_from(minute).ok()?,
        u8::try_from(second).ok()?,
    ))
}

fn month_of(token: &[u8]) -> Option<u8> {
    const MONTHS: [&[u8; 3]; 12] = [
        b"jan", b"feb", b"mar", b"apr", b"may", b"jun", b"jul", b"aug", b"sep", b"oct", b"nov",
        b"dec",
    ];

    let prefix = token.get(..3)?;

    MONTHS
        .iter()
        .position(|month| prefix.eq_ignore_ascii_case(*month))
        .map(|index| index as u8 + 1)
}

/// The offset of an obsolete zone name (RFC 5322 4.3): the US zones by
/// their offsets, UT and GMT as UTC, and a military letter as `-0000`,
/// since its sign was used both ways.
fn zone(token: &[u8]) -> Option<i16> {
    const ZONES: [(&[u8], i16); 12] = [
        (b"UT", 0),
        (b"UTC", 0),
        (b"GMT", 0),
        (b"Z", 0),
        (b"EST", -300),
        (b"EDT", -240),
        (b"CST", -360),
        (b"CDT", -300),
        (b"MST", -420),
        (b"MDT", -360),
        (b"PST", -480),
        (b"PDT", -420),
    ];

    // NOTE: a POSIX zone such as `EST5EDT` names its standard zone first.
    let letters = token
        .iter()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    let token = &token[..letters];

    ZONES
        .iter()
        .find(|(name, _)| token.eq_ignore_ascii_case(name))
        .map(|(_, offset)| *offset)
        .or_else(|| (token.len() == 1 && token[0].is_ascii_alphabetic()).then_some(0))
}

#[cfg(test)]
mod tests {
    use crate::tree::field::date::*;

    #[test]
    fn the_rfc_spelling_and_its_obsolete_variants_read() {
        let date = parse_date(b"Sat, 20 Nov 2021 14:22:01 -0800").expect("a date");
        assert_eq!(date.to_unix_seconds(), 1_637_446_921);

        let date = parse_date(b"Thu, 13 Feb 1969 23:32 EST").expect("a date");
        assert_eq!(
            (date.year, date.hour, date.minute, date.offset),
            (1969, 23, 32, -300)
        );

        let date = parse_date(b"1 Jan 99 00:00:00 GMT (comment)").expect("a date");
        assert_eq!(date.year, 1999);
    }

    #[test]
    fn a_day_that_does_not_exist_reads_as_nothing() {
        assert_eq!(parse_date(b"30 Feb 2024 10:00:00 +0000"), None);
        assert_eq!(parse_date(b"not a date"), None);
    }
}
