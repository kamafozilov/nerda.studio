mod support;

use std::path::PathBuf;

use support::{TempDir, load_json};
use workspace::{AddedProject, ProjectId, ProjectKind, Timestamp, Workspace};

const NOW: Timestamp = Timestamp(1_000);

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
}

fn add(workspace: &mut Workspace, path: &std::path::Path) -> ProjectId {
    match workspace.add_project(path, NOW).unwrap() {
        AddedProject::Added(id) | AddedProject::Existing(id) => id,
    }
}

#[test]
fn the_computer_is_one_project_however_often_it_is_asked_for() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    add(&mut workspace, &temp.dir("web"));

    let computer = workspace.computer_project(NOW).unwrap();
    assert_eq!(workspace.computer_project(Timestamp(2_000)), Some(computer));
    assert_eq!(workspace.projects().len(), 2);
    assert_eq!(
        workspace.computer().map(|project| project.id),
        Some(computer)
    );

    let session = workspace.new_session(computer, NOW).unwrap();
    let project = workspace.session(session).unwrap().project;
    assert_eq!(project, computer);
    assert_eq!(
        workspace.project(project).unwrap().path,
        std::fs::canonicalize(home()).unwrap()
    );
}

#[test]
fn no_computer_until_it_is_asked_for() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    add(&mut workspace, &temp.dir("web"));
    assert!(workspace.computer().is_none());
}

#[test]
fn adding_the_home_folder_finds_the_computer() {
    let mut workspace = Workspace::default();
    let computer = workspace.computer_project(NOW).unwrap();
    assert_eq!(
        workspace.add_project(&home(), NOW).unwrap(),
        AddedProject::Existing(computer)
    );
}

#[test]
fn the_computer_is_called_no_project() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    // A folder of the same name keeps it: the pill's icon tells them apart.
    let folder = add(&mut workspace, &temp.dir("No project"));
    let computer = workspace.computer_project(NOW).unwrap();
    assert_eq!(
        workspace.display_name(computer).as_deref(),
        Some("No project")
    );
    assert_eq!(
        workspace.display_name(folder).as_deref(),
        Some("No project")
    );
}

#[test]
fn project_search_never_finds_the_computer() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let web = add(&mut workspace, &temp.dir("web"));
    workspace.computer_project(NOW).unwrap();

    let ids = |query: &str| -> Vec<ProjectId> {
        (workspace.matching_projects(query).iter())
            .map(|project| project.id)
            .collect()
    };
    assert_eq!(ids(""), vec![web]);
    assert_eq!(ids("project"), Vec::<ProjectId>::new());
}

#[test]
fn projects_saved_before_the_computer_load_as_folders() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 2,
            "projects": [
                {"id": 0, "title": "web", "path": "/tmp/web", "created_at": 1},
                {"id": 1, "title": "api", "path": "/tmp/api", "created_at": 1}
            ]}"#,
    );
    let kinds: Vec<ProjectKind> = (workspace.projects().iter())
        .map(|project| project.kind)
        .collect();
    assert_eq!(kinds, vec![ProjectKind::Folder, ProjectKind::Folder]);
    assert!(workspace.computer().is_none());
}

#[test]
fn the_computer_survives_saving_and_folders_are_written_as_before() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = Workspace::default();
    let web = add(&mut workspace, &temp.dir("web"));
    let computer = workspace.computer_project(NOW).unwrap();
    workspace.save(&path).unwrap();

    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.computer().map(|project| project.id), Some(computer));
    assert_eq!(loaded.project(web).unwrap().kind, ProjectKind::Folder);
    assert_eq!(loaded.project(computer), workspace.project(computer));
    // Only the computer's entry names a kind, so an older build reads every
    // folder's entry unchanged.
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("\"kind\"").count(), 1, "{text}");
}

/// The home folder as a JSON string, as a project's path holds it.
fn home_json() -> String {
    format!(
        "{:?}",
        std::fs::canonicalize(home()).unwrap().display().to_string()
    )
}

#[test]
fn adding_the_home_folder_first_makes_the_computer() {
    let mut workspace = Workspace::default();
    let AddedProject::Added(computer) = workspace.add_project(&home(), NOW).unwrap() else {
        panic!("the home folder should add the computer");
    };
    assert_eq!(
        workspace.computer().map(|project| project.id),
        Some(computer)
    );
    assert_eq!(workspace.computer_project(NOW), Some(computer));
    assert_eq!(workspace.projects().len(), 1);
}

#[test]
fn a_computer_an_older_build_saved_as_a_folder_loads_as_the_computer() {
    // An older build drops the kind it doesn't know, leaving a folder at home.
    let workspace = load_json(&format!(
        r#"{{"version": 1, "next_id": 4,
            "projects": [
                {{"id": 0, "title": "web", "path": "/tmp/web", "created_at": 1}},
                {{"id": 2, "title": "No project", "path": {home}, "created_at": 1}}
            ],
            "sessions": [{{"id": 3, "project": 2, "title": "New session", "created_at": 1}}]}}"#,
        home = home_json()
    ));
    let computer = workspace.computer().expect("the computer");
    assert_eq!(computer.id, ProjectId(2));
    assert_eq!(workspace.projects().len(), 2);
    assert_eq!(
        workspace.project(ProjectId(0)).unwrap().kind,
        ProjectKind::Folder
    );
    assert_eq!(
        workspace.session(workspace::SessionId(3)).unwrap().project,
        ProjectId(2)
    );
    assert!(
        workspace
            .matching_projects("")
            .iter()
            .all(|p| p.id == ProjectId(0))
    );
}

#[test]
fn a_folder_at_home_beside_the_computer_folds_into_it() {
    let workspace = load_json(&format!(
        r#"{{"version": 1, "next_id": 6,
            "projects": [
                {{"id": 1, "title": "kamron", "path": {home}, "created_at": 1}},
                {{"id": 2, "title": "No project", "path": {home}, "created_at": 1, "kind": "computer"}}
            ],
            "sessions": [
                {{"id": 3, "project": 1, "title": "old", "created_at": 1}},
                {{"id": 4, "project": 2, "title": "new", "created_at": 1}}
            ],
            "scope": {{"Project": 1}}}}"#,
        home = home_json()
    ));
    assert_eq!(workspace.projects().len(), 1);
    assert_eq!(workspace.computer().map(|p| p.id), Some(ProjectId(2)));
    for session in [3, 4] {
        assert_eq!(
            workspace
                .session(workspace::SessionId(session))
                .unwrap()
                .project,
            ProjectId(2)
        );
    }
    assert_eq!(
        workspace.scope(),
        workspace::ProjectScope::Project(ProjectId(2))
    );
}

#[test]
fn a_folder_named_no_project_elsewhere_stays_a_folder() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 1,
            "projects": [{"id": 0, "title": "No project", "path": "/tmp/No project", "created_at": 1}]}"#,
    );
    assert!(workspace.computer().is_none());
    assert_eq!(workspace.projects()[0].kind, ProjectKind::Folder);
}
