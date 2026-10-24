mod support;

use support::{TempDir, load_json};
use workspace::{
    AddedProject, ProjectId, ProjectScope, SessionId, SessionSort, Tab, Timestamp, Workspace,
};

/// Two projects (1, 2) with sessions of mixed activity; ids 3..=7.
const FIXTURE: &str = r#"{"version": 1, "next_id": 20,
    "projects": [
        {"id": 1, "title": "one", "path": "/one", "created_at": 0},
        {"id": 2, "title": "two", "path": "/two", "created_at": 0}
    ],
    "sessions": [
        {"id": 3, "project": 1, "title": "Исправить Баг", "created_at": 10, "last_activity_at": 300},
        {"id": 4, "project": 1, "title": "Fix bug", "created_at": 20, "last_activity_at": 300},
        {"id": 5, "project": 2, "title": "Oʻzbekcha Matn", "created_at": 30, "last_activity_at": 100},
        {"id": 6, "project": 2, "title": "Draft", "created_at": 30, "last_activity_at": null},
        {"id": 7, "project": 1, "title": "Old", "created_at": 5, "last_activity_at": 50}
    ]}"#;

fn ids(sessions: Vec<&workspace::Session>) -> Vec<u64> {
    sessions.iter().map(|session| session.id.0).collect()
}

#[test]
fn new_session_reuses_the_empty_draft() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let AddedProject::Added(project) = workspace.add_project(temp.path(), Timestamp(0)).unwrap()
    else {
        panic!("expected a new project");
    };
    let first = workspace.new_session(project, Timestamp(1)).unwrap();
    let second = workspace.new_session(project, Timestamp(2)).unwrap();

    assert_eq!(first, second);
    let session = workspace.session(first).unwrap();
    assert_eq!(
        (
            session.title.as_str(),
            session.created_at,
            session.last_activity_at
        ),
        ("New session", Timestamp(1), None)
    );
    assert!(workspace.visible_sessions("").is_empty());
    assert_eq!(workspace.selected_session(), None);
}

#[test]
fn new_session_creates_a_fresh_id_once_sessions_have_activity() {
    let mut workspace = load_json(FIXTURE);
    let created = workspace.new_session(ProjectId(1), Timestamp(400)).unwrap();

    assert_eq!(created, SessionId(20));
    assert_eq!(
        workspace.new_session(ProjectId(1), Timestamp(401)),
        Some(created)
    );
    // Project 2's draft belongs to project 2 only.
    assert_eq!(
        workspace.new_session(ProjectId(2), Timestamp(402)),
        Some(SessionId(6))
    );
}

#[test]
fn new_session_for_an_unknown_project_creates_nothing() {
    let mut workspace = load_json(FIXTURE);
    assert_eq!(workspace.new_session(ProjectId(99), Timestamp(1)), None);
    assert!(workspace.session(SessionId(20)).is_none());
}

#[test]
fn search_is_trimmed_and_unicode_case_insensitive() {
    let workspace = load_json(FIXTURE);
    assert_eq!(ids(workspace.visible_sessions("  баг\t")), [3]);
    assert_eq!(ids(workspace.visible_sessions("ИСПРАВ")), [3]);
    assert_eq!(ids(workspace.visible_sessions("oʻzBEK")), [5]);
    assert_eq!(ids(workspace.visible_sessions("BUG")), [4]);
    assert_eq!(
        ids(workspace.visible_sessions("nothing")),
        Vec::<u64>::new()
    );
    assert_eq!(workspace.visible_sessions("   ").len(), 4);
}

#[test]
fn search_matches_the_spellings_people_type() {
    let workspace = load_json(FIXTURE);
    // A plain apostrophe, or a curly one, for the Uzbek ʻ.
    assert_eq!(ids(workspace.visible_sessions("o'zbek")), [5]);
    assert_eq!(ids(workspace.visible_sessions("o’zbek")), [5]);
    assert_eq!(ids(workspace.visible_sessions("oʻzbek")), [5]);
    // е for ё, either way round.
    let workspace = load_json(&FIXTURE.replace("Исправить Баг", "Отчёт о баге"));
    assert_eq!(ids(workspace.visible_sessions("отчет")), [3]);
    assert_eq!(ids(workspace.visible_sessions("ОТЧЁТ")), [3]);
    // The title as shown: runs of whitespace read as one space.
    let workspace = load_json(&FIXTURE.replace("Fix bug", "Fix   the\\tbug"));
    assert_eq!(ids(workspace.visible_sessions("fix the bug")), [4]);
    assert_eq!(ids(workspace.visible_sessions("fix  the")), [4]);
}

#[test]
fn sorts_newest_first_with_ties_by_id() {
    let mut workspace = load_json(FIXTURE);
    assert_eq!(workspace.sort(), SessionSort::LastActivity);
    assert_eq!(ids(workspace.visible_sessions("")), [4, 3, 5, 7]);

    workspace.set_sort(SessionSort::Created);
    assert_eq!(ids(workspace.visible_sessions("")), [5, 4, 3, 7]);
}

#[test]
fn scope_filters_and_unknown_projects_fall_back_to_all() {
    let mut workspace = load_json(FIXTURE);
    workspace.set_scope(ProjectScope::Project(ProjectId(2)));
    assert_eq!(ids(workspace.visible_sessions("")), [5]);

    workspace.set_scope(ProjectScope::Project(ProjectId(99)));
    assert_eq!(workspace.scope(), ProjectScope::All);
    assert_eq!(workspace.visible_sessions("").len(), 4);
}

#[test]
fn a_draft_stays_out_of_the_list_until_its_first_message() {
    let mut workspace = load_json(FIXTURE);
    let draft = workspace.new_session(ProjectId(1), Timestamp(900)).unwrap();
    workspace.open_session(draft);

    assert_eq!(workspace.selected_session(), Some(draft));
    assert!(!ids(workspace.visible_sessions("")).contains(&draft.0));
    assert_eq!(
        ids(workspace.visible_sessions("new session")),
        Vec::<u64>::new()
    );
    // A draft is not activity: it doesn't move its project ahead of one with later work.
    let order = |workspace: &Workspace| -> Vec<u64> {
        workspace
            .projects()
            .iter()
            .map(|project| project.id.0)
            .collect()
    };
    let worked =
        load_json(&FIXTURE.replace("\"last_activity_at\": 300", "\"last_activity_at\": 40"));
    let mut drafted = worked.clone();
    drafted.new_session(ProjectId(1), Timestamp(900));
    assert_eq!(order(&worked), [2, 1]);
    assert_eq!(order(&drafted), [2, 1]);

    let active =
        load_json(&FIXTURE.replace("\"last_activity_at\": null", "\"last_activity_at\": 400"));
    assert_eq!(ids(active.visible_sessions(""))[0], 6);
}

#[test]
fn opening_an_unknown_session_changes_nothing() {
    let mut workspace = load_json(FIXTURE);
    workspace.open_session(SessionId(5));
    workspace.open_session(SessionId(99));
    assert_eq!(workspace.selected_session(), Some(SessionId(5)));
    assert_eq!(workspace.tabs(), [Tab::Session(SessionId(5))]);
}

#[test]
fn context_project_prefers_selection_then_scope_then_activity() {
    let mut workspace = load_json(FIXTURE);
    assert_eq!(workspace.context_project(), Some(ProjectId(1)));
    workspace.set_scope(ProjectScope::Project(ProjectId(2)));
    assert_eq!(workspace.context_project(), Some(ProjectId(2)));
    workspace.open_session(SessionId(7));
    assert_eq!(workspace.context_project(), Some(ProjectId(1)));

    assert_eq!(Workspace::default().context_project(), None);
}

#[test]
fn latest_session_picks_greatest_activity_with_ties_by_id() {
    let temp = TempDir::new();
    let mut workspace = load_json(FIXTURE);
    assert_eq!(
        workspace
            .latest_session(ProjectId(1))
            .map(|session| session.id),
        Some(SessionId(4))
    );
    // The draft's activity is its creation time (30), below session 5's activity (100).
    assert_eq!(
        workspace
            .latest_session(ProjectId(2))
            .map(|session| session.id),
        Some(SessionId(5))
    );
    assert!(workspace.latest_session(ProjectId(99)).is_none());

    let AddedProject::Added(empty) = workspace.add_project(temp.path(), Timestamp(0)).unwrap()
    else {
        panic!("expected a new project");
    };
    assert!(workspace.latest_session(empty).is_none());
}

#[test]
fn a_title_shows_as_one_trimmed_line() {
    let session = |title: &str| workspace::Session {
        id: workspace::SessionId(1),
        project: workspace::ProjectId(0),
        title: title.to_owned(),
        created_at: workspace::Timestamp(0),
        last_activity_at: None,
        choice: workspace::AgentChoice::default(),
        conversation: None,
        archived_at: None,
    };
    assert_eq!(session("Fix the tabs").display_title(), "Fix the tabs");
    assert_eq!(
        session("  First line\nSecond\tline  ").display_title(),
        "First line Second line"
    );
    assert_eq!(session("   ").display_title(), "Untitled session");
    assert_eq!(session("\n\n").display_title(), "Untitled session");
    assert_eq!(
        session("Отчёт oʻzgartirish").display_title(),
        "Отчёт oʻzgartirish"
    );
}
