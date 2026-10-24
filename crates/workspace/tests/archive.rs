mod support;

use support::{TempDir, load_json};
use workspace::{ProjectId, SessionId, Tab, Timestamp, Workspace};

/// One project with three sessions that have seen work (3, 4, 5) and a
/// draft (6); 3 and 4 open as tabs, 4 selected.
const FIXTURE: &str = r#"{"version": 1, "next_id": 20,
    "projects": [{"id": 1, "title": "one", "path": "/one", "created_at": 0}],
    "sessions": [
        {"id": 3, "project": 1, "title": "Fix the tabs", "created_at": 10, "last_activity_at": 300},
        {"id": 4, "project": 1, "title": "Write the docs", "created_at": 20, "last_activity_at": 200},
        {"id": 5, "project": 1, "title": "Old", "created_at": 5, "last_activity_at": 50},
        {"id": 6, "project": 1, "title": "New session", "created_at": 30, "last_activity_at": null}
    ],
    "project_on_screen": 1, "tabs": [3, 4], "selected_session": 4}"#;

fn ids(sessions: Vec<&workspace::Session>) -> Vec<u64> {
    sessions.iter().map(|session| session.id.0).collect()
}

#[test]
fn an_archived_session_leaves_the_list_for_the_archive() {
    let mut workspace = load_json(FIXTURE);
    assert!(workspace.archive_session(SessionId(3), Timestamp(400)));

    assert_eq!(ids(workspace.visible_sessions("")), [4, 5]);
    assert_eq!(ids(workspace.archived_sessions("")), [3]);
    assert_eq!(ids(workspace.archived_sessions("TABS")), [3]);
    assert!(workspace.archived_sessions("docs").is_empty());
}

#[test]
fn archiving_closes_the_tab_and_selects_its_neighbor() {
    let mut workspace = load_json(FIXTURE);
    workspace.archive_session(SessionId(4), Timestamp(400));

    assert_eq!(workspace.open_tabs(), [Tab::Session(SessionId(3))]);
    assert_eq!(workspace.selected_session(), Some(SessionId(3)));
}

#[test]
fn archiving_the_last_tab_on_screen_opens_the_draft() {
    let mut workspace = load_json(FIXTURE);
    workspace.archive_session(SessionId(3), Timestamp(400));
    workspace.archive_session(SessionId(4), Timestamp(401));

    assert_eq!(workspace.selected_session(), Some(SessionId(6)));
}

#[test]
fn a_draft_an_unknown_or_an_archived_session_is_not_archived_again() {
    let mut workspace = load_json(FIXTURE);
    assert!(!workspace.archive_session(SessionId(6), Timestamp(400)));
    assert!(!workspace.archive_session(SessionId(99), Timestamp(400)));
    assert!(workspace.archive_session(SessionId(5), Timestamp(400)));
    assert!(!workspace.archive_session(SessionId(5), Timestamp(500)));
    assert_eq!(
        workspace.session(SessionId(5)).unwrap().archived_at,
        Some(Timestamp(400))
    );
}

#[test]
fn restoring_or_a_new_message_brings_it_back() {
    let mut workspace = load_json(FIXTURE);
    workspace.archive_session(SessionId(3), Timestamp(400));
    workspace.archive_session(SessionId(5), Timestamp(400));

    workspace.unarchive_session(SessionId(3));
    workspace.record_message(SessionId(5), "again", Timestamp(500));

    assert_eq!(ids(workspace.visible_sessions("")), [5, 3, 4]);
    assert!(workspace.archived_sessions("").is_empty());
}

#[test]
fn the_latest_session_of_a_project_is_never_an_archived_one() {
    let mut workspace = load_json(FIXTURE);
    workspace.archive_session(SessionId(3), Timestamp(400));

    assert_eq!(
        workspace
            .latest_session(ProjectId(1))
            .map(|session| session.id),
        Some(SessionId(4))
    );
}

#[test]
fn the_archive_survives_saving_and_a_listed_session_writes_no_field() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = load_json(FIXTURE);
    workspace.archive_session(SessionId(3), Timestamp(400));
    workspace.save(&path).unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written.matches("archived_at").count(), 1);
    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(ids(loaded.archived_sessions("")), [3]);
}
