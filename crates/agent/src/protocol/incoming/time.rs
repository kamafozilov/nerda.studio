//! Reset times as the agent writes them.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Seconds since the UNIX epoch from a time the agent wrote: seconds or
/// milliseconds, as a number or a string of digits, or an RFC 3339 date.
pub(crate) fn epoch_seconds(value: &Value) -> Option<u64> {
    let number = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse().ok());
    if let Some(number) = number.filter(|number| number.is_finite() && *number > 0.0) {
        // Ten digits of seconds last until 2286; anything longer is milliseconds.
        let seconds = if number >= 1e10 {
            number / 1000.0
        } else {
            number
        };
        return Some(seconds as u64);
    }
    rfc3339(value.as_str()?)
}

/// For a field that holds such a time; one that doesn't read as one is none.
pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    Ok(epoch_seconds(&Value::deserialize(deserializer)?))
}

/// `2026-10-23T18:30:00Z`, with fractions of a second or an offset such as
/// `+05:00`.
fn rfc3339(text: &str) -> Option<u64> {
    let text = text.trim();
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = text.get(range)?;
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| digits.parse().ok())?
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let separators = [(4, b'-'), (7, b'-'), (13, b':'), (16, b':')];
    let bytes = text.as_bytes();
    if separators.iter().any(|&(at, byte)| bytes[at] != byte)
        || !matches!(bytes[10], b'T' | b't' | b' ')
    {
        return None;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        rest = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (hours, minutes) = rest[1..].split_once(':')?;
            sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60)
        }
    };
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(seconds).ok()
}

/// Days from 1970-01-01 to a date of the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_reset_time_reads_in_every_form_the_agent_writes() {
        let seconds = 1_792_600_200;
        for written in [
            json!(1_792_600_200),
            json!(1_792_600_200_000_u64),
            json!(1_792_600_200.5),
            json!("1792600200"),
            json!("1792600200000"),
            json!("2026-10-21T16:30:00Z"),
            json!("2026-10-21T16:30:00.250Z"),
            json!("2026-10-21T21:30:00+05:00"),
        ] {
            assert_eq!(epoch_seconds(&written), Some(seconds), "{written}");
        }
        for nonsense in [
            json!(null),
            json!(0),
            json!(-5),
            json!("soon"),
            json!("2026-13-01T00:00:00Z"),
            json!({}),
        ] {
            assert_eq!(epoch_seconds(&nonsense), None, "{nonsense}");
        }
        assert_eq!(epoch_seconds(&json!("1970-01-01T00:00:01Z")), Some(1));
        assert_eq!(
            epoch_seconds(&json!("2000-02-29T00:00:00Z")),
            Some(951_782_400)
        );
    }
}
