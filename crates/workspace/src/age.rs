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
