mod support;

use support::{TempDir, load_json};
use workspace::{OnLaunch, Page, Preferences, ProjectId, SessionId, Tab, Timestamp, Workspace};

/// Project 1 holds session 3, project 2 holds 4; both with activity. Both
/// sessions and Notes are open, with 4 on screen.
fn fixture(preferences: &str) -> String {
    format!(
        r#"{{"version": 1, "next_id": 20,
        "projects": [
            {{"id": 1, "title": "one", "path": "/one", "created_at": 0}},
            {{"id": 2, "title": "two", "path": "/two", "created_at": 0}}
        ],
        "sessions": [
            {{"id": 3, "project": 1, "title": "a", "created_at": 1, "last_activity_at": 10}},
            {{"id": 4, "project": 2, "title": "b", "created_at": 2, "last_activity_at": 20}}
        ],
        "tabs": [3, 4], "pages": [{{"page": "notes", "at": 1}}],
        "selected_session": 4, "project_on_screen": 2{preferences}}}"#
    )
}

#[test]
fn a_file_without_preferences_keeps_the_defaults() {
    let workspace = load_json(&fixture(""));
    assert_eq!(workspace.preferences(), &Preferences::default());
    assert_eq!(workspace.preferences().on_launch, OnLaunch::LastTabs);
    assert!(workspace.preferences().confirm_quit);
}

#[test]
fn preferences_save_and_load() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = load_json(&fixture(""));
    workspace.set_on_launch(OnLaunch::NewSession);
    workspace.set_confirm_quit(false);
    workspace.save(&path).unwrap();

    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        json["preferences"],
        serde_json::json!({"on_launch": "new-session", "confirm_quit": false})
    );
    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.preferences(), workspace.preferences());
}

#[test]
fn a_preference_this_build_cannot_read_falls_back_alone() {
    // A newer build's choice, a wrong type, and a key unknown here.
    let workspace = load_json(&fixture(
        r#", "preferences": {"on_launch": "split-view", "confirm_quit": "yes", "theme": "dark"}"#,
    ));
    assert_eq!(workspace.preferences(), &Preferences::default());
    assert_eq!(
        workspace.tabs().len(),
        3,
        "the rest of the file still loads"
    );

    let workspace = load_json(&fixture(r#", "preferences": 7"#));
    assert_eq!(workspace.preferences(), &Preferences::default());

    let workspace = load_json(&fixture(r#", "preferences": {"confirm_quit": false}"#));
    assert_eq!(workspace.preferences().on_launch, OnLaunch::LastTabs);
    assert!(!workspace.preferences().confirm_quit);
}

#[test]
fn last_tabs_opens_the_tabs_as_they_were() {
    let mut workspace = load_json(&fixture(""));
    let before = workspace.tabs().to_vec();
    assert!(!workspace.open_on_launch(Timestamp(100)));
    assert_eq!(workspace.tabs(), before);
    assert_eq!(workspace.selected_session(), Some(SessionId(4)));
}

#[test]
fn new_session_closes_every_tab_and_opens_the_draft_of_the_project_on_screen() {
    let mut workspace = load_json(&fixture(r#", "preferences": {"on_launch": "new-session"}"#));
    assert!(workspace.open_on_launch(Timestamp(100)));
    let draft = workspace.selected_session().unwrap();
    let session = workspace.session(draft).unwrap();
    assert!(session.is_empty());
    assert_eq!(session.project, ProjectId(2));
    assert_eq!(workspace.tabs(), [Tab::Session(draft)]);
    assert!(!workspace.tabs().contains(&Tab::Page(Page::Notes)));

    // Started again, it reuses that draft.
    workspace.open_on_launch(Timestamp(200));
    assert_eq!(workspace.tabs(), [Tab::Session(draft)]);
}

#[test]
fn new_session_without_projects_leaves_nothing_open() {
    let mut workspace = Workspace::default();
    workspace.open_page(Page::Usage);
    workspace.set_on_launch(OnLaunch::NewSession);
    assert!(workspace.open_on_launch(Timestamp(100)));
    assert!(workspace.tabs().is_empty());
    assert_eq!(workspace.selected_tab(), None);
}
