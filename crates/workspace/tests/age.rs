use workspace::{Timestamp, compact_age};

const SECOND: u64 = 1000;
const MINUTE: u64 = 60 * SECOND;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

fn age(elapsed: u64) -> String {
    let now = 10_000 * DAY;
    compact_age(Timestamp(now - elapsed), Timestamp(now))
}

#[test]
fn boundaries_switch_units() {
    let cases = [
        (0, "now"),
        (MINUTE - 1, "now"),
        (MINUTE, "1m"),
        (5 * MINUTE + 59 * SECOND, "5m"),
        (HOUR - 1, "59m"),
        (HOUR, "1h"),
        (DAY - 1, "23h"),
        (DAY, "1d"),
        (7 * DAY - 1, "6d"),
        (7 * DAY, "1w"),
        (365 * DAY - 1, "52w"),
        (365 * DAY, "1y"),
        (3 * 365 * DAY, "3y"),
    ];
    for (elapsed, label) in cases {
        assert_eq!(age(elapsed), label, "elapsed {elapsed} ms");
    }
}

#[test]
fn future_times_are_now() {
    assert_eq!(compact_age(Timestamp(5 * DAY), Timestamp(DAY)), "now");
}
