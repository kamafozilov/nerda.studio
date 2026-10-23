mod support;

use support::{TempDir, load_json};
use workspace::{ProjectId, SessionId, Timestamp, Workspace};

/// One project with a session that has seen work (2), a draft still called
/// what it was created as (3) and a draft with a title of its own (4).
const FIXTURE: &str = r#"{"version": 1, "next_id": 10,
    "projects": [{"id": 1, "title": "web", "path": "/web", "created_at": 0}],
    "sessions": [
        {"id": 2, "project": 1, "title": "Fix the glide", "created_at": 1, "last_activity_at": 50},
        {"id": 3, "project": 1, "title": "New session", "created_at": 2, "last_activity_at": null},
        {"id": 4, "project": 1, "title": "Kept", "created_at": 3, "last_activity_at": null}
    ]}"#;

fn title(workspace: &Workspace, id: u64) -> &str {
    &workspace.session(SessionId(id)).unwrap().title
}

fn listed(workspace: &Workspace) -> Vec<u64> {
    let sessions = workspace.visible_sessions("");
    sessions.iter().map(|session| session.id.0).collect()
}

#[test]
fn the_first_message_titles_a_draft_and_lists_it_newest() {
    let mut workspace = load_json(FIXTURE);
    workspace.record_message(
        SessionId(3),
        "\n  \n  Rename the sidebar  \nthen test",
        Timestamp(90),
    );

    assert_eq!(title(&workspace, 3), "Rename the sidebar");
    let session = workspace.session(SessionId(3)).unwrap();
    assert_eq!(session.last_activity_at, Some(Timestamp(90)));
    assert!(!session.is_empty());
    assert_eq!(listed(&workspace), [3, 2]);
    // No longer a draft: New session makes another.
    let draft = workspace.new_session(ProjectId(1), Timestamp(91)).unwrap();
    assert_ne!(draft, SessionId(3));
}

#[test]
fn later_messages_move_the_session_up_but_keep_its_title() {
    let mut workspace = load_json(FIXTURE);
    workspace.record_message(SessionId(3), "first", Timestamp(60));
    workspace.record_message(SessionId(2), "Something else entirely", Timestamp(70));

    assert_eq!(title(&workspace, 2), "Fix the glide");
    assert_eq!(title(&workspace, 3), "first");
    assert_eq!(listed(&workspace), [2, 3]);
}

#[test]
fn a_title_of_its_own_stays() {
    let mut workspace = load_json(FIXTURE);
    workspace.record_message(SessionId(4), "Do the thing", Timestamp(60));
    assert_eq!(title(&workspace, 4), "Kept");
    assert_eq!(listed(&workspace), [4, 2]);
}

#[test]
fn a_message_without_words_lists_the_session_untitled() {
    let mut workspace = load_json(FIXTURE);
    workspace.record_message(SessionId(3), " \n\t ", Timestamp(60));
    assert_eq!(title(&workspace, 3), "New session");
    assert_eq!(listed(&workspace), [3, 2]);

    let blank = FIXTURE.replace(r#""title": "New session""#, r#""title": "  ""#);
    let mut workspace = load_json(&blank);
    workspace.record_message(SessionId(3), "Сделай отчёт про oʻquv", Timestamp(60));
    assert_eq!(title(&workspace, 3), "Сделай отчёт про oʻquv");
}

#[test]
fn a_long_first_line_is_cut_by_characters() {
    let mut workspace = load_json(FIXTURE);
    let line = "ё".repeat(500);
    workspace.record_message(SessionId(3), &line, Timestamp(60));
    assert_eq!(title(&workspace, 3), "ё".repeat(200));
}

#[test]
fn an_unknown_session_changes_nothing() {
    let mut workspace = load_json(FIXTURE);
    workspace.record_message(SessionId(99), "hello", Timestamp(60));
    assert_eq!(listed(&workspace), [2]);
}

#[test]
fn the_title_and_the_activity_are_saved() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = load_json(FIXTURE);
    workspace.record_message(SessionId(3), "Ship it", Timestamp(60));
    workspace.save(&path).unwrap();

    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(title(&loaded, 3), "Ship it");
    assert_eq!(
        loaded.session(SessionId(3)).unwrap().last_activity_at,
        Some(Timestamp(60))
    );
}
