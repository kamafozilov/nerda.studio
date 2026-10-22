mod support;

use support::{TempDir, load_json};
use workspace::{
    AddedProject, LoadError, ProjectId, ProjectScope, SessionId, SessionSort, SessionView,
    Timestamp, Workspace,
};

#[test]
fn a_missing_file_loads_as_empty() {
    let temp = TempDir::new();
    let workspace = Workspace::load(&temp.path().join("absent.json")).unwrap();
    assert!(!workspace.has_projects());
    assert!(workspace.projects().is_empty());
    assert!(workspace.visible_sessions("").is_empty());
}

#[test]
fn corrupt_and_future_files_are_refused() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    for contents in [
        "",
        "{not json",
        r#"{"projects": []}"#,
        r#"{"version": 1, "projects": 5}"#,
    ] {
        std::fs::write(&path, contents).unwrap();
        let error = Workspace::load(&path).unwrap_err();
        assert!(
            matches!(error, LoadError::Corrupt(_)),
            "{contents:?}: {error:?}"
        );
    }
    std::fs::write(&path, r#"{"version": 2, "whatever": true}"#).unwrap();
    assert!(matches!(
        Workspace::load(&path),
        Err(LoadError::UnsupportedVersion(2))
    ));
}

#[test]
fn a_folder_in_place_of_the_file_is_an_io_error() {
    let temp = TempDir::new();
    assert!(matches!(
        Workspace::load(temp.path()),
        Err(LoadError::Io(_))
    ));
}

#[test]
fn round_trip_keeps_state_and_the_id_counter() {
    let temp = TempDir::new();
    let project_dir = temp.dir("project");
    let path = temp.path().join("nested/state/workspace.json");
    let mut workspace = Workspace::default();
    let AddedProject::Added(project) = workspace.add_project(&project_dir, Timestamp(5)).unwrap()
    else {
        panic!("expected a new project");
    };
    let session = workspace.new_session(project, Timestamp(6)).unwrap();
    let AddedProject::Added(second) = workspace
        .add_project(&temp.dir("second"), Timestamp(5))
        .unwrap()
    else {
        panic!("expected a new project");
    };
    let elsewhere = workspace.new_session(second, Timestamp(6)).unwrap();
    workspace.open_session(elsewhere);
    workspace.set_sort(SessionSort::Created);
    workspace.set_scope(ProjectScope::Project(project));
    workspace.open_session(session);
    workspace.set_collapsed(project, true);
    workspace.set_view(SessionView::Sessions);
    workspace.save(&path).unwrap();

    let mut loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.projects(), workspace.projects());
    assert_eq!(loaded.visible_sessions(""), workspace.visible_sessions(""));
    assert_eq!(loaded.sort(), SessionSort::Created);
    assert_eq!(loaded.scope(), ProjectScope::Project(project));
    assert_eq!(loaded.selected_session(), Some(session));
    assert_eq!(loaded.tabs(), [elsewhere, session]);
    assert!(loaded.is_collapsed(project));
    assert_eq!(loaded.view(), SessionView::Sessions);

    let other = temp.dir("other");
    let AddedProject::Added(next) = loaded.add_project(&other, Timestamp(7)).unwrap() else {
        panic!("expected a new project");
    };
    assert!(next.0 > session.0 && next.0 > project.0);

    // Saving again replaces the file and leaves no temp files behind.
    loaded.save(&path).unwrap();
    let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn loading_repairs_dangling_references_and_the_counter() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 2,
            "projects": [
                {"id": 1, "title": "one", "path": "/one", "created_at": 0},
                {"id": 1, "title": "duplicate", "path": "/dup", "created_at": 0}
            ],
            "sessions": [
                {"id": 8, "project": 1, "title": "kept", "created_at": 1, "last_activity_at": 2},
                {"id": 8, "project": 1, "title": "duplicate", "created_at": 1, "last_activity_at": 2},
                {"id": 9, "project": 4, "title": "orphan", "created_at": 1, "last_activity_at": 2}
            ],
            "scope": {"Project": 4},
            "selected_session": 9,
            "tabs": [9, 8, 8, 77],
            "collapsed": [1, 4]}"#,
    );
    let titles: Vec<&str> = workspace
        .projects()
        .iter()
        .map(|project| project.title.as_str())
        .collect();
    assert_eq!(titles, ["one"]);
    let sessions: Vec<&str> = workspace
        .visible_sessions("")
        .iter()
        .map(|session| session.title.as_str())
        .collect();
    assert_eq!(sessions, ["kept"]);
    assert_eq!(workspace.scope(), ProjectScope::All);
    assert_eq!(workspace.selected_session(), None);
    assert_eq!(workspace.tabs(), [SessionId(8)]);
    assert!(workspace.session(SessionId(9)).is_none());
    assert!(workspace.is_collapsed(ProjectId(1)));
    assert!(!workspace.is_collapsed(ProjectId(4)));

    // The counter moves above id 8 even though the file said 2.
    let mut workspace = workspace;
    let draft = workspace.new_session(ProjectId(1), Timestamp(3)).unwrap();
    assert_eq!(draft, SessionId(9));
}

#[test]
fn ids_beyond_the_safe_range_are_corrupt() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let json = format!(r#"{{"version": 1, "next_id": {}}}"#, u64::MAX);
    std::fs::write(&path, json).unwrap();
    assert!(matches!(Workspace::load(&path), Err(LoadError::Corrupt(_))));
}
