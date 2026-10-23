use crate::Timestamp;

const MINUTE: u64 = 60 * 1000;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;
const YEAR: u64 = 365 * DAY;

/// Short age label for list rows: "now" under a minute (and for future times), then "5m", "3h", "2d", "4w", "1y".
pub fn compact_age(then: Timestamp, now: Timestamp) -> String {
    let elapsed = now.0.saturating_sub(then.0);
    if elapsed < MINUTE {
        "now".to_owned()
    } else if elapsed < HOUR {
        format!("{}m", elapsed / MINUTE)
    } else if elapsed < DAY {
        format!("{}h", elapsed / HOUR)
    } else if elapsed < WEEK {
        format!("{}d", elapsed / DAY)
    } else if elapsed < YEAR {
        format!("{}w", elapsed / WEEK)
    } else {
        format!("{}y", elapsed / YEAR)
    }
}

/// Milliseconds until `compact_age(then, now)` next reads differently, so a
/// list can redraw its ages exactly when one changes, and not in between.
pub fn age_changes_in(then: Timestamp, now: Timestamp) -> u64 {
    if then.0 > now.0 {
        // A future time reads "now" until a minute after it.
        return then.0 - now.0 + MINUTE;
    }
    let elapsed = now.0 - then.0;
    let unit = if elapsed < HOUR {
        MINUTE
    } else if elapsed < DAY {
        HOUR
    } else if elapsed < WEEK {
        DAY
    } else if elapsed < YEAR {
        WEEK
    } else {
        YEAR
    };
    let next = (elapsed / unit + 1) * unit;
    // A year is not a whole number of weeks: "52w" turns to "1y" at the year.
    let next = if elapsed < YEAR { next.min(YEAR) } else { next };
    next - elapsed
}
