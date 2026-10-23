//! Where closing tabs sit in the row while their slots collapse.

/// The row to draw: the live tabs in order, each closing one kept after the
/// tab that preceded it in `old`, or first if none of those is left.
pub fn with_closing<T: Copy + PartialEq>(
    old: &[T],
    live: &[T],
    closing: impl Fn(&T) -> bool,
) -> Vec<T> {
    let mut order = live.to_vec();
    for (old_index, id) in old.iter().enumerate() {
        if !closing(id) || order.contains(id) {
            continue;
        }
        let at = old[..old_index]
            .iter()
            .rev()
            .find_map(|previous| order.iter().position(|kept| kept == previous))
            .map_or(0, |index| index + 1);
        order.insert(at, *id);
    }
    order
}

#[cfg(test)]
mod tests {
    use workspace::SessionId;

    use super::*;

    fn ids(raw: &[u64]) -> Vec<SessionId> {
        raw.iter().copied().map(SessionId).collect()
    }

    #[test]
    fn closing_tabs_keep_their_place_beside_live_neighbors() {
        let old = ids(&[1, 2, 3, 4]);
        let closing = ids(&[1, 3]);
        let order = with_closing(&old, &ids(&[2, 4, 5]), |id| closing.contains(id));
        assert_eq!(order, ids(&[1, 2, 3, 4, 5]));
    }

    #[test]
    fn a_closing_run_follows_the_last_live_tab_before_it() {
        let old = ids(&[1, 2, 3, 4]);
        let closing = ids(&[2, 3]);
        let order = with_closing(&old, &ids(&[4, 1]), |id| closing.contains(id));
        assert_eq!(order, ids(&[4, 1, 2, 3]));
    }
}
