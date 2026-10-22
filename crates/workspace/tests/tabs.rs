mod support;

use support::load_json;
use workspace::{ProjectId, SessionId, Timestamp, Visits, Workspace};

/// Project 1 holds sessions 3, 4, 5 and 6, all with activity; project 2 holds 7
/// and its draft 8.
const FIXTURE: &str = r#"{"version": 1, "next_id": 20,
    "projects": [
        {"id": 1, "title": "one", "path": "/one", "created_at": 0},
        {"id": 2, "title": "two", "path": "/two", "created_at": 0}
    ],
    "sessions": [
        {"id": 3, "project": 1, "title": "a", "created_at": 1, "last_activity_at": 10},
        {"id": 4, "project": 1, "title": "b", "created_at": 2, "last_activity_at": 20},
        {"id": 5, "project": 1, "title": "c", "created_at": 3, "last_activity_at": 30},
        {"id": 6, "project": 1, "title": "d", "created_at": 4, "last_activity_at": 40},
        {"id": 7, "project": 2, "title": "e", "created_at": 5, "last_activity_at": 50},
        {"id": 8, "project": 2, "title": "New session", "created_at": 6, "last_activity_at": null}
    ]}"#;

fn ids(raw: &[u64]) -> Vec<SessionId> {
    raw.iter().copied().map(SessionId).collect()
}

/// Project 1's sessions 3..=6 open in order with `selected` on screen, and
/// project 2's session 7 open between them.
fn with_tabs(selected: u64) -> Workspace {
    let mut workspace = load_json(FIXTURE);
    for id in [3, 4, 7, 5, 6] {
        workspace.open_session(SessionId(id));
    }
    workspace.open_session(SessionId(selected));
    workspace
}

#[test]
fn only_the_selected_projects_tabs_show() {
    let mut workspace = with_tabs(4);
    assert_eq!(workspace.open_tabs(), ids(&[3, 4, 5, 6]));
    workspace.open_session(SessionId(8));
    assert_eq!(workspace.open_tabs(), ids(&[7, 8]));
    assert_eq!(workspace.tabs(), ids(&[3, 4, 7, 5, 6, 8]));
}

#[test]
fn closing_the_selection_selects_the_right_neighbor_then_the_left() {
    let mut workspace = with_tabs(5);
    workspace.close_tab(SessionId(5), Timestamp(100));
    assert_eq!(workspace.selected_session(), Some(SessionId(6)));
    workspace.close_tab(SessionId(6), Timestamp(100));
    assert_eq!(workspace.selected_session(), Some(SessionId(4)));
    assert_eq!(workspace.open_tabs(), ids(&[3, 4]));
}

#[test]
fn closing_another_tab_keeps_the_selection() {
    let mut workspace = with_tabs(4);
    workspace.close_tab(SessionId(6), Timestamp(100));
    assert_eq!(workspace.selected_session(), Some(SessionId(4)));
    workspace.close_tab(SessionId(99), Timestamp(100));
    assert_eq!(workspace.open_tabs(), ids(&[3, 4, 5]));
}

#[test]
fn closing_the_last_tab_opens_the_projects_draft() {
    let mut workspace = with_tabs(3);
    workspace.close_all_tabs(Timestamp(100));
    // Project 1 had no draft, so one is made; project 2's tab stays open.
    let draft = workspace.selected_session().unwrap();
    assert_eq!(workspace.session(draft).unwrap().project, ProjectId(1));
    assert!(workspace.session(draft).unwrap().is_empty());
    assert_eq!(workspace.open_tabs(), [draft]);
    assert_eq!(workspace.tabs(), [SessionId(7), draft]);

    // Closing the draft itself reopens it rather than making another.
    workspace.close_tab(draft, Timestamp(200));
    assert_eq!(workspace.open_tabs(), [draft]);
}

#[test]
fn close_others_and_either_side_keep_the_chosen_tab_selected() {
    let mut workspace = with_tabs(3);
    workspace.close_tabs_before(SessionId(5), Timestamp(100));
    assert_eq!(workspace.open_tabs(), ids(&[5, 6]));
    assert_eq!(workspace.selected_session(), Some(SessionId(5)));

    let mut workspace = with_tabs(6);
    workspace.close_tabs_after(SessionId(4), Timestamp(100));
    assert_eq!(workspace.open_tabs(), ids(&[3, 4]));
    assert_eq!(workspace.selected_session(), Some(SessionId(4)));

    let mut workspace = with_tabs(3);
    workspace.close_other_tabs(SessionId(5), Timestamp(100));
    assert_eq!(workspace.open_tabs(), ids(&[5]));
    assert_eq!(workspace.selected_session(), Some(SessionId(5)));
    assert_eq!(workspace.tabs(), ids(&[7, 5]));
}

#[test]
fn moving_a_tab_leaves_other_projects_tabs_in_place() {
    let mut workspace = with_tabs(3);
    workspace.move_tab(SessionId(3), 2);
    assert_eq!(workspace.open_tabs(), ids(&[4, 5, 3, 6]));
    assert_eq!(workspace.tabs(), ids(&[4, 5, 7, 3, 6]));
    workspace.move_tab(SessionId(6), 0);
    assert_eq!(workspace.open_tabs(), ids(&[6, 4, 5, 3]));
    // Past the end means last.
    workspace.move_tab(SessionId(6), 99);
    assert_eq!(workspace.open_tabs(), ids(&[4, 5, 3, 6]));
}

#[test]
fn stepping_through_tabs_wraps_at_either_end() {
    let workspace = with_tabs(6);
    assert_eq!(workspace.adjacent_tab(1), Some(SessionId(3)));
    assert_eq!(workspace.adjacent_tab(-1), Some(SessionId(5)));
    assert_eq!(workspace.tab_at(1), Some(SessionId(4)));
    assert_eq!(workspace.tab_at(4), None);
    assert_eq!(load_json(FIXTURE).adjacent_tab(1), None);
}

#[test]
fn an_older_files_selection_becomes_its_tab() {
    let workspace =
        load_json(&FIXTURE.replace(r#""sessions""#, r#""selected_session": 5, "sessions""#));
    assert_eq!(workspace.tabs(), ids(&[5]));
    assert_eq!(workspace.selected_session(), Some(SessionId(5)));
}

#[test]
fn a_workspace_with_nothing_open_opens_the_latest_tab_or_the_draft() {
    // Nothing open: the most recently active project (2) opens its draft.
    let mut workspace = load_json(FIXTURE);
    assert!(workspace.ensure_open_session(Timestamp(100)));
    assert_eq!(workspace.selected_session(), Some(SessionId(8)));
    assert!(!workspace.ensure_open_session(Timestamp(100)));

    // Tabs saved without a selection: the project's last tab shows.
    let workspace_json = FIXTURE.replace(r#""sessions""#, r#""tabs": [8, 7], "sessions""#);
    let mut workspace = load_json(&workspace_json);
    assert!(workspace.ensure_open_session(Timestamp(100)));
    assert_eq!(workspace.selected_session(), Some(SessionId(7)));

    assert!(!Workspace::default().ensure_open_session(Timestamp(100)));
}

#[test]
fn back_and_forward_retrace_visits_and_a_new_visit_drops_forward() {
    let mut visits = Visits::default();
    for id in [3, 4, 5] {
        visits.record(SessionId(id));
    }
    assert!(!visits.can_go_forward());
    assert_eq!(visits.step(true), Some(SessionId(4)));
    assert_eq!(visits.step(true), Some(SessionId(3)));
    assert_eq!(visits.step(true), None);
    assert_eq!(visits.step(false), Some(SessionId(4)));
    visits.record(SessionId(6));
    assert!(!visits.can_go_forward());
    assert_eq!(visits.step(true), Some(SessionId(4)));
}

#[test]
fn closed_tabs_drop_out_of_the_history() {
    let mut visits = Visits::default();
    for id in [3, 4, 3, 5] {
        visits.record(SessionId(id));
    }
    // 4 closed: Back goes 5, then 3 once, not 3 twice.
    visits.prune(&ids(&[3, 5]), Some(SessionId(5)));
    assert_eq!(visits.step(true), Some(SessionId(3)));
    assert!(!visits.can_go_back());

    // The current session closed: the new selection takes its place.
    visits.prune(&ids(&[5]), Some(SessionId(5)));
    assert!(!visits.can_go_back());
    assert_eq!(visits.step(false), None);
}
