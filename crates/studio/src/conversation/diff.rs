//! The lines a change to a file removes and adds, from what the agent sent
//! its edit tools: the text replaced and its replacement, matched line by
//! line, without the lines they share at either end; or a new file's text.

use serde_json::Value;

/// Lines a diff shows at most; the rest is counted.
pub const MAX_LINES: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffLine {
    Removed(String),
    Added(String),
    /// A line both have, between changed ones.
    Kept(String),
}

/// The most cells the line matching fills before it gives up and shows the
/// old lines removed and the new ones added.
const MATCH_LIMIT: usize = 250_000;

/// The lines `tool`'s `input` removes and adds.
pub fn of(tool: &str, input: &Value) -> Vec<DiffLine> {
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    match tool {
        "Write" => text(input, "content")
            .map(|content| {
                content
                    .lines()
                    .map(|line| DiffLine::Added(line.to_owned()))
                    .collect()
            })
            .unwrap_or_default(),
        "MultiEdit" => (input.get("edits").and_then(Value::as_array))
            .into_iter()
            .flatten()
            .flat_map(|edit| {
                replace(
                    &text(edit, "old_string").unwrap_or_default(),
                    &text(edit, "new_string").unwrap_or_default(),
                )
            })
            .collect(),
        "NotebookEdit" => text(input, "new_source")
            .map(|source| {
                source
                    .lines()
                    .map(|line| DiffLine::Added(line.to_owned()))
                    .collect()
            })
            .unwrap_or_default(),
        _ => replace(
            &text(input, "old_string").unwrap_or_default(),
            &text(input, "new_string").unwrap_or_default(),
        ),
    }
}

/// `old` turning into `new`, without the lines both start and end with.
fn replace(old: &str, new: &str) -> Vec<DiffLine> {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = (old[start..].iter().rev())
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (old, new) = (&old[start..old.len() - end], &new[start..new.len() - end]);
    if old.len() * new.len() > MATCH_LIMIT {
        let removed = old.iter().map(|line| DiffLine::Removed((*line).to_owned()));
        let added = new.iter().map(|line| DiffLine::Added((*line).to_owned()));
        return removed.chain(added).collect();
    }
    // The longest run of lines both keep, from the end, then walked forward.
    let mut kept = vec![vec![0u32; new.len() + 1]; old.len() + 1];
    for a in (0..old.len()).rev() {
        for b in (0..new.len()).rev() {
            kept[a][b] = match old[a] == new[b] {
                true => kept[a + 1][b + 1] + 1,
                false => kept[a + 1][b].max(kept[a][b + 1]),
            };
        }
    }
    let (mut a, mut b, mut lines) = (0, 0, Vec::new());
    while a < old.len() || b < new.len() {
        if a < old.len() && b < new.len() && old[a] == new[b] {
            lines.push(DiffLine::Kept(old[a].to_owned()));
            (a, b) = (a + 1, b + 1);
        } else if b < new.len() && (a == old.len() || kept[a][b + 1] > kept[a + 1][b]) {
            lines.push(DiffLine::Added(new[b].to_owned()));
            b += 1;
        } else {
            lines.push(DiffLine::Removed(old[a].to_owned()));
            a += 1;
        }
    }
    lines
}

/// How many lines `lines` adds and removes.
pub fn stats(lines: &[DiffLine]) -> (usize, usize) {
    let count = |kind: fn(&DiffLine) -> bool| lines.iter().filter(|line| kind(line)).count();
    (
        count(|line| matches!(line, DiffLine::Added(_))),
        count(|line| matches!(line, DiffLine::Removed(_))),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn shared_lines_at_either_end_are_left_out() {
        let lines = of(
            "Edit",
            &json!({"old_string": "a\nb\nc", "new_string": "a\nx\ny\nc"}),
        );
        assert_eq!(
            lines,
            [
                DiffLine::Removed("b".into()),
                DiffLine::Added("x".into()),
                DiffLine::Added("y".into())
            ]
        );
        assert_eq!(stats(&lines), (2, 1));
    }

    #[test]
    fn lines_kept_between_changes_are_matched() {
        let lines = of(
            "Edit",
            &json!({"old_string": "a\nb\nc", "new_string": "a\nB\nc\nd"}),
        );
        assert_eq!(
            lines,
            [
                DiffLine::Removed("b".into()),
                DiffLine::Added("B".into()),
                DiffLine::Kept("c".into()),
                DiffLine::Added("d".into())
            ]
        );
    }

    #[test]
    fn a_new_file_is_all_added() {
        assert_eq!(stats(&of("Write", &json!({"content": "one\ntwo"}))), (2, 0));
    }

    #[test]
    fn several_edits_add_up() {
        let edits = json!({"edits": [
            {"old_string": "a", "new_string": "b"},
            {"old_string": "c", "new_string": "c\nd"}
        ]});
        assert_eq!(stats(&of("MultiEdit", &edits)), (2, 1));
    }
}
