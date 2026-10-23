use workspace::{Timestamp, age_changes_in, compact_age};

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

#[test]
fn the_next_change_lands_on_the_next_label() {
    let now = 10_000 * DAY;
    let cases = [
        (0, MINUTE),
        (59 * SECOND, SECOND),
        (MINUTE, MINUTE),
        (5 * MINUTE + 20 * SECOND, 40 * SECOND),
        (HOUR - 1, 1),
        (HOUR + 1, HOUR - 1),
        (DAY + 2 * HOUR, 22 * HOUR),
        (7 * DAY + DAY, 6 * DAY),
        (52 * 7 * DAY, DAY),
        (365 * DAY + DAY, 364 * DAY),
    ];
    for (elapsed, wait) in cases {
        let then = Timestamp(now - elapsed);
        assert_eq!(
            age_changes_in(then, Timestamp(now)),
            wait,
            "after {elapsed} ms"
        );
        let before = compact_age(then, Timestamp(now + wait - 1));
        let after = compact_age(then, Timestamp(now + wait));
        assert_eq!(
            before,
            compact_age(then, Timestamp(now)),
            "after {elapsed} ms"
        );
        assert_ne!(before, after, "after {elapsed} ms");
    }
    // A time ahead of the clock reads "now" until a minute past it.
    assert_eq!(
        age_changes_in(Timestamp(now + 5 * SECOND), Timestamp(now)),
        65 * SECOND
    );
}
