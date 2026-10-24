mod support;

use std::path::Path;

use support::{TempDir, load_json};
use workspace::{AddProjectError, AddedProject, Timestamp, Workspace};

const NOW: Timestamp = Timestamp(1_000);

fn added(result: Result<AddedProject, AddProjectError>) -> workspace::ProjectId {
    match result.unwrap() {
        AddedProject::Added(id) => id,
        AddedProject::Existing(id) => panic!("expected a new project, got existing {id:?}"),
    }
}

#[test]
fn same_folder_via_trailing_slash_or_dot_dot_is_existing() {
    let temp = TempDir::new();
    let folder = temp.dir("app");
    temp.dir("app/sub");
    let mut workspace = Workspace::default();
    let id = added(workspace.add_project(&folder, NOW));

    let slashed = format!("{}/", folder.display());
    assert_eq!(
        workspace.add_project(Path::new(&slashed), NOW).unwrap(),
        AddedProject::Existing(id)
    );
    let dotted = folder.join("sub").join("..");
    assert_eq!(
        workspace.add_project(&dotted, NOW).unwrap(),
        AddedProject::Existing(id)
    );
    assert_eq!(workspace.projects().len(), 1);
}

#[cfg(unix)]
#[test]
fn same_folder_via_symlink_is_existing() {
    let temp = TempDir::new();
    let folder = temp.dir("real");
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&folder, &link).unwrap();
    let mut workspace = Workspace::default();
    let id = added(workspace.add_project(&link, NOW));

    assert_eq!(
        workspace.add_project(&folder, NOW).unwrap(),
        AddedProject::Existing(id)
    );
    let project = workspace.project(id).unwrap();
    assert_eq!(project.path, std::fs::canonicalize(&folder).unwrap());
    assert_eq!(project.title, "real");
}

#[test]
fn a_file_is_not_a_directory() {
    let temp = TempDir::new();
    let file = temp.path().join("notes.txt");
    std::fs::write(&file, "x").unwrap();
    let mut workspace = Workspace::default();

    let error = workspace.add_project(&file, NOW).unwrap_err();
    assert!(
        matches!(error, AddProjectError::NotADirectory(_)),
        "{error:?}"
    );
    assert!(workspace.projects().is_empty());
}

#[test]
fn missing_paths_are_not_found() {
    let temp = TempDir::new();
    let file = temp.path().join("notes.txt");
    std::fs::write(&file, "x").unwrap();
    let mut workspace = Workspace::default();

    for path in [
        temp.path().join("missing"),
        file.join("below-a-file"),
        Default::default(),
    ] {
        let error = workspace.add_project(&path, NOW).unwrap_err();
        assert!(
            matches!(error, AddProjectError::NotFound(_)),
            "{path:?}: {error:?}"
        );
    }
    assert!(workspace.projects().is_empty());
}

#[test]
fn different_folders_with_the_same_name_are_both_added() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let first = added(workspace.add_project(&temp.dir("a/app"), NOW));
    let second = added(workspace.add_project(&temp.dir("b/app"), NOW));

    assert_ne!(first, second);
    assert_eq!(workspace.project(first).unwrap().title, "app");
    assert_eq!(workspace.project(second).unwrap().title, "app");
}

#[test]
fn relative_and_home_paths_resolve_to_absolute_folders() {
    let mut workspace = Workspace::default();
    let current = added(workspace.add_project(Path::new("."), NOW));
    let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    assert_eq!(workspace.project(current).unwrap().path, cwd);
    assert_eq!(
        workspace.add_project(&cwd, NOW).unwrap(),
        AddedProject::Existing(current)
    );

    if let Some(home) = dirs::home_dir().and_then(|home| std::fs::canonicalize(home).ok()) {
        let tilde = match workspace.add_project(Path::new("~"), NOW).unwrap() {
            AddedProject::Added(id) | AddedProject::Existing(id) => id,
        };
        assert_eq!(workspace.project(tilde).unwrap().path, home);
    }
}

#[cfg(unix)]
#[test]
fn root_folder_is_titled_by_its_path() {
    let mut workspace = Workspace::default();
    let id = added(workspace.add_project(Path::new("/"), NOW));
    assert_eq!(workspace.project(id).unwrap().title, "/");
}

#[test]
fn projects_order_by_activity_then_title_then_id() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 10,
            "projects": [
                {"id": 1, "title": "beta", "path": "/b", "created_at": 100},
                {"id": 2, "title": "Alpha", "path": "/a", "created_at": 100},
                {"id": 3, "title": "alpha", "path": "/a2", "created_at": 100},
                {"id": 4, "title": "Old", "path": "/o", "created_at": 50},
                {"id": 5, "title": "Busy", "path": "/busy", "created_at": 10}
            ],
            "sessions": [
                {"id": 6, "project": 5, "title": "t", "created_at": 20, "last_activity_at": 500},
                {"id": 7, "project": 4, "title": "t", "created_at": 60, "last_activity_at": null}
            ]}"#,
    );
    let titles: Vec<&str> = workspace
        .projects()
        .iter()
        .map(|project| project.title.as_str())
        .collect();
    assert_eq!(titles, ["Busy", "Alpha", "alpha", "beta", "Old"]);
}

#[test]
fn projects_match_by_shown_name_folding_case_and_apostrophes() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let web = added(workspace.add_project(&temp.dir("Web"), NOW));
    let uzbek = added(workspace.add_project(&temp.dir("oʻzbek"), NOW));
    let names = |query: &str| -> Vec<workspace::ProjectId> {
        (workspace.matching_projects(query).iter())
            .map(|project| project.id)
            .collect()
    };
    assert_eq!(names("").len(), 2);
    assert_eq!(names("  "), names(""));
    assert_eq!(names("WEB"), [web]);
    assert_eq!(names("o'z"), [uzbek]);
    assert!(names("nothing").is_empty());
}
