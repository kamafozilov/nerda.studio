mod support;

use support::load_json;
use workspace::{ProjectId, ProjectScope, SessionGroup, SessionSort};

/// Project 1 holds the latest activity, 2 the newest session, 3 nothing yet.
const FIXTURE: &str = r#"{"version": 1, "next_id": 20,
    "projects": [
        {"id": 1, "title": "beta", "path": "/beta", "created_at": 0},
        {"id": 2, "title": "Alpha", "path": "/alpha", "created_at": 0},
        {"id": 3, "title": "gamma", "path": "/gamma", "created_at": 20}
    ],
    "sessions": [
        {"id": 10, "project": 1, "title": "Fix bug", "created_at": 10, "last_activity_at": 300},
        {"id": 11, "project": 1, "title": "Notes", "created_at": 5, "last_activity_at": 50},
        {"id": 12, "project": 2, "title": "Parser notes", "created_at": 40, "last_activity_at": 100}
    ]}"#;

/// Each group as (project id, session ids, collapsed).
fn shape(groups: Vec<SessionGroup>) -> Vec<(u64, Vec<u64>, bool)> {
    groups
        .into_iter()
        .map(|group| {
            let sessions = group.sessions.iter().map(|session| session.id.0).collect();
            (group.project.id.0, sessions, group.collapsed)
        })
        .collect()
}

#[test]
fn projects_rank_by_their_newest_session_under_the_sort() {
    let mut workspace = load_json(FIXTURE);
    assert_eq!(
        shape(workspace.session_groups("")),
        [
            (1, vec![10, 11], false),
            (2, vec![12], false),
            (3, vec![], false),
        ]
    );

    workspace.set_sort(SessionSort::Created);
    assert_eq!(
        shape(workspace.session_groups("")),
        [
            (2, vec![12], false),
            (3, vec![], false),
            (1, vec![10, 11], false),
        ]
    );
}

#[test]
fn equal_ranks_order_by_title_ignoring_case() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 9, "projects": [
            {"id": 1, "title": "beta", "path": "/beta", "created_at": 7},
            {"id": 2, "title": "Alpha", "path": "/alpha", "created_at": 7}
        ]}"#,
    );
    assert_eq!(
        shape(workspace.session_groups("")),
        [(2, vec![], false), (1, vec![], false)]
    );
}

#[test]
fn search_keeps_the_rank_and_drops_projects_without_a_match() {
    let workspace = load_json(FIXTURE);
    // Project 2's match is newer than project 1's, yet project 1 stays first.
    assert_eq!(
        shape(workspace.session_groups("  NOTES ")),
        [(1, vec![11], false), (2, vec![12], false)]
    );
    assert!(workspace.session_groups("nothing").is_empty());
}

#[test]
fn a_collapsed_project_unfolds_while_searching() {
    let mut workspace = load_json(FIXTURE);
    workspace.set_collapsed(ProjectId(1), true);
    assert_eq!(
        shape(workspace.session_groups("")),
        [
            (1, vec![10, 11], true),
            (2, vec![12], false),
            (3, vec![], false),
        ]
    );
    assert_eq!(
        shape(workspace.session_groups("notes")),
        [(1, vec![11], false), (2, vec![12], false)]
    );
    assert!(workspace.is_collapsed(ProjectId(1)));

    workspace.set_collapsed(ProjectId(1), false);
    assert!(!workspace.session_groups("")[0].collapsed);

    workspace.set_collapsed(ProjectId(99), true);
    assert!(!workspace.is_collapsed(ProjectId(99)));
}

#[test]
fn scope_keeps_only_its_project() {
    let mut workspace = load_json(FIXTURE);
    workspace.set_scope(ProjectScope::Project(ProjectId(3)));
    assert_eq!(shape(workspace.session_groups("")), [(3, vec![], false)]);
    assert!(workspace.session_groups("bug").is_empty());
}
