//! # Date and time
//!
//! The decoded value of a date field: a civil date and time with its UTC
//! offset, as RFC 5322 3.3 writes it.

use alloc::{format, string::String};
use core::fmt;

/// A date-time with its UTC offset.
///
/// Refs: <https://datatracker.ietf.org/doc/html/rfc5322#section-3.3>
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct MimeDateTime {
    /// The year, four digits.
    pub year: u16,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, 1 to 31.
    pub day: u8,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
    /// The second, 0 to 60 (a leap second).
    pub second: u8,
    /// The offset from UTC, in minutes east.
    pub offset: i16,
}

const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl MimeDateTime {
    /// The date-time of a Unix timestamp, at the given offset in minutes.
    pub fn from_unix_seconds(seconds: i64, offset: i16) -> Self {
        let local = seconds + i64::from(offset) * 60;
        let days = local.div_euclid(86_400);
        let rest = local.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);

        Self {
            year: year as u16,
            month: month as u8,
            day: day as u8,
            hour: (rest / 3600) as u8,
            minute: (rest / 60 % 60) as u8,
            second: (rest % 60) as u8,
            offset,
        }
    }

    /// The instant as Unix seconds, the offset applied.
    pub fn to_unix_seconds(&self) -> i64 {
        let days = days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let local = days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second);

        local - i64::from(self.offset) * 60
    }

    /// The date-time as RFC 3339 text (`2021-11-20T14:22:01-08:00`).
    pub fn to_rfc3339(&self) -> String {
        let (sign, offset) = sign(self.offset);

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{sign}{:02}:{:02}",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            offset / 60,
            offset % 60
        )
    }

    /// Whether the fields name a real calendar day and time.
    pub fn is_valid(&self) -> bool {
        (1..=12).contains(&self.month)
            && self.day >= 1
            && self.day <= days_in_month(i64::from(self.year), self.month)
            && self.hour < 24
            && self.minute < 60
            && self.second <= 60
            && self.offset.unsigned_abs() < 24 * 60
    }

    /// The day of the week, 0 for Monday.
    pub fn weekday(&self) -> u8 {
        let days = days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        ((days + 3).rem_euclid(7)) as u8
    }
}

/// The RFC 5322 3.3 spelling: `Sat, 20 Nov 2021 14:22:01 -0800`.
impl fmt::Display for MimeDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days = days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let (sign, offset) = sign(self.offset);

        write!(
            f,
            "{}, {} {} {:04} {:02}:{:02}:{:02} {sign}{:02}{:02}",
            DAYS[days.rem_euclid(7) as usize],
            self.day,
            MONTHS[usize::from(self.month.clamp(1, 12) - 1)],
            self.year,
            self.hour,
            self.minute,
            self.second,
            offset / 60,
            offset % 60
        )
    }
}

fn sign(offset: i16) -> (char, u16) {
    if offset < 0 {
        ('-', offset.unsigned_abs())
    } else {
        ('+', offset.unsigned_abs())
    }
}

pub(crate) fn days_in_month(year: i64, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;

    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use crate::value::datetime::*;

    #[test]
    fn a_date_converts_both_ways() {
        let date = MimeDateTime {
            year: 2021,
            month: 11,
            day: 20,
            hour: 14,
            minute: 22,
            second: 1,
            offset: -480,
        };

        assert_eq!(date.to_unix_seconds(), 1_637_446_921);
        assert_eq!(MimeDateTime::from_unix_seconds(1_637_446_921, -480), date);
        assert_eq!(date.to_rfc3339(), "2021-11-20T14:22:01-08:00");
        assert_eq!(date.to_string(), "Sat, 20 Nov 2021 14:22:01 -0800");
    }

    #[test]
    fn february_the_thirtieth_is_not_a_day() {
        let date = MimeDateTime {
            year: 2024,
            month: 2,
            day: 30,
            ..Default::default()
        };

        assert!(!date.is_valid());
        assert!(MimeDateTime { day: 29, ..date }.is_valid());
    }
}
