mod support;

use support::{TempDir, load_json};
use workspace::{Page, ProjectId, SessionId, Tab, Timestamp, Workspace};

/// Project 1 holds sessions 3 and 4, project 2 holds 5; all with activity.
const FIXTURE: &str = r#"{"version": 1, "next_id": 20,
    "projects": [
        {"id": 1, "title": "one", "path": "/one", "created_at": 0},
        {"id": 2, "title": "two", "path": "/two", "created_at": 0}
    ],
    "sessions": [
        {"id": 3, "project": 1, "title": "a", "created_at": 1, "last_activity_at": 10},
        {"id": 4, "project": 1, "title": "b", "created_at": 2, "last_activity_at": 20},
        {"id": 5, "project": 2, "title": "c", "created_at": 3, "last_activity_at": 30}
    ]}"#;

fn session(id: u64) -> Tab {
    Tab::Session(SessionId(id))
}

const NOTES: Tab = Tab::Page(Page::Notes);
const DESIGN: Tab = Tab::Page(Page::Design);

/// Sessions 3, then Notes, then 4, with 4 on screen.
fn with_notes() -> Workspace {
    let mut workspace = load_json(FIXTURE);
    workspace.open_session(SessionId(3));
    workspace.open_page(Page::Notes);
    workspace.open_session(SessionId(4));
    workspace
}

#[test]
fn a_page_opened_again_keeps_its_one_tab() {
    let mut workspace = with_notes();
    workspace.open_page(Page::Notes);
    assert_eq!(workspace.open_tabs(), [session(3), NOTES, session(4)]);
    assert_eq!(workspace.selected_tab(), Some(NOTES));
    assert_eq!(workspace.selected_session(), None);
}

#[test]
fn pages_show_beside_whichever_project_is_on_screen() {
    let mut workspace = with_notes();
    workspace.open_session(SessionId(5));
    assert_eq!(workspace.open_tabs(), [NOTES, session(5)]);
    // On screen, the page keeps the project's tabs, and new sessions go there.
    workspace.open_page(Page::Notes);
    assert_eq!(workspace.open_tabs(), [NOTES, session(5)]);
    assert_eq!(workspace.context_project(), Some(ProjectId(2)));
}

#[test]
fn closing_a_selected_page_selects_its_neighbor_then_the_draft() {
    let mut workspace = with_notes();
    workspace.open_page(Page::Notes);
    workspace.close_tab(NOTES, Timestamp(100));
    assert_eq!(workspace.selected_tab(), Some(session(4)));

    // A page alone on screen closes into the project's draft.
    let mut workspace = with_notes();
    workspace.close_other_tabs(NOTES, Timestamp(100));
    assert_eq!(workspace.open_tabs(), [NOTES]);
    assert!(workspace.can_close_tab(NOTES));
    workspace.close_tab(NOTES, Timestamp(100));
    let draft = workspace.selected_session().unwrap();
    assert_eq!(workspace.session(draft).unwrap().project, ProjectId(1));
    assert_eq!(workspace.open_tabs(), [Tab::Session(draft)]);
}

#[test]
fn a_draft_beside_a_page_can_close() {
    let mut workspace = load_json(FIXTURE);
    let draft = workspace.new_session(ProjectId(1), Timestamp(100)).unwrap();
    workspace.open_page(Page::Design);
    workspace.open_session(draft);
    assert_eq!(workspace.open_tabs(), [DESIGN, Tab::Session(draft)]);
    workspace.close_tab(Tab::Session(draft), Timestamp(100));
    assert_eq!(workspace.open_tabs(), [DESIGN]);
}

#[test]
fn a_page_opened_from_the_draft_alone_takes_its_tab() {
    let mut workspace = load_json(FIXTURE);
    workspace.open_session(SessionId(5));
    let draft = workspace.new_session(ProjectId(1), Timestamp(100)).unwrap();
    workspace.open_session(draft);
    assert!(!workspace.can_close_tab(Tab::Session(draft)));
    workspace.open_page(Page::Usage);
    let usage = Tab::Page(Page::Usage);
    assert_eq!(workspace.open_tabs(), [usage]);
    assert_eq!(workspace.selected_tab(), Some(usage));
    // The draft keeps its project on screen, and project 2's tab stays open.
    assert_eq!(workspace.context_project(), Some(ProjectId(1)));
    assert_eq!(workspace.tabs(), [session(5), usage]);

    // Another page then opens beside it, and closing both brings the same draft back.
    workspace.open_page(Page::Notes);
    assert_eq!(workspace.open_tabs(), [usage, NOTES]);
    workspace.close_all_tabs(Timestamp(200));
    assert_eq!(workspace.open_tabs(), [Tab::Session(draft)]);

    // Beside another tab, the draft keeps its own and the page opens last.
    workspace.open_page(Page::Notes);
    workspace.open_session(draft);
    workspace.open_page(Page::Usage);
    assert_eq!(workspace.open_tabs(), [NOTES, Tab::Session(draft), usage]);

    // A used session alone on screen keeps its tab.
    let mut workspace = load_json(FIXTURE);
    workspace.open_session(SessionId(3));
    workspace.open_page(Page::Usage);
    assert_eq!(workspace.open_tabs(), [session(3), usage]);
}

#[test]
fn without_projects_a_page_opens_and_closes_to_nothing() {
    let mut workspace = Workspace::default();
    workspace.open_page(Page::Routines);
    assert_eq!(workspace.open_tabs(), [Tab::Page(Page::Routines)]);
    workspace.close_tab(Tab::Page(Page::Routines), Timestamp(100));
    assert!(workspace.open_tabs().is_empty());
    assert_eq!(workspace.selected_tab(), None);
}

#[test]
fn saved_pages_keep_their_place_and_selection() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = with_notes();
    workspace.open_page(Page::Design);
    workspace.move_tab(DESIGN, 0);
    workspace.save(&path).unwrap();

    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.tabs(), [DESIGN, session(3), NOTES, session(4)]);
    assert_eq!(loaded.selected_tab(), Some(DESIGN));
    assert_eq!(loaded.open_tabs(), loaded.tabs());

    // A build from before pages reads `tabs` as sessions alone.
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["tabs"], serde_json::json!([3, 4]));
    assert_eq!(json["selected_session"], serde_json::Value::Null);
}

#[test]
fn a_page_this_build_does_not_know_is_skipped() {
    let workspace = load_json(&FIXTURE.replace(
        r#""sessions""#,
        r#""tabs": [3], "pages": [{"page": "calendar", "at": 0}, {"page": "notes", "at": 2}],
            "selected_page": "calendar", "sessions""#,
    ));
    assert_eq!(workspace.tabs(), [session(3), NOTES]);
    assert_eq!(workspace.selected_tab(), None);
}

#[test]
fn the_footer_pages_save_under_their_own_names() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = Workspace::default();
    for page in [Page::Usage, Page::PullRequests] {
        workspace.open_page(page);
    }
    workspace.save(&path).unwrap();

    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let keys: Vec<&str> = json["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page["page"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["usage", "pull-requests"]);
    assert_eq!(json["selected_page"], "pull-requests");

    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.tabs(), workspace.tabs());
    assert_eq!(loaded.selected_tab(), Some(Tab::Page(Page::PullRequests)));
}

#[test]
fn a_settings_tab_an_earlier_build_saved_is_skipped() {
    let workspace = load_json(&FIXTURE.replace(
        r#""sessions""#,
        r#""tabs": [3], "pages": [{"page": "settings", "at": 1}],
            "selected_page": "settings", "sessions""#,
    ));
    assert_eq!(workspace.tabs(), [session(3)]);
    assert_eq!(workspace.selected_tab(), None);
}
