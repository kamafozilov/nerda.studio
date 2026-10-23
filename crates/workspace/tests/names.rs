mod support;

use support::TempDir;
use workspace::{AddedProject, ProjectId, Timestamp, Workspace};

const NOW: Timestamp = Timestamp(1_000);

fn add(workspace: &mut Workspace, path: &std::path::Path) -> ProjectId {
    match workspace.add_project(path, NOW).unwrap() {
        AddedProject::Added(id) | AddedProject::Existing(id) => id,
    }
}

#[test]
fn a_unique_title_is_the_name() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let web = add(&mut workspace, &temp.dir("web"));
    let api = add(&mut workspace, &temp.dir("api"));
    assert_eq!(workspace.display_name(web).as_deref(), Some("web"));
    assert_eq!(workspace.display_name(api).as_deref(), Some("api"));
}

#[test]
fn same_titles_take_the_shortest_distinct_end_of_their_paths() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let site = add(&mut workspace, &temp.dir("site/web"));
    let app = add(&mut workspace, &temp.dir("app/web"));
    // Two levels up, the parents match too: only the third component differs.
    let left = add(&mut workspace, &temp.dir("left/shared/web"));
    let right = add(&mut workspace, &temp.dir("right/shared/web"));

    assert_eq!(workspace.display_name(site).as_deref(), Some("site/web"));
    assert_eq!(workspace.display_name(app).as_deref(), Some("app/web"));
    assert_eq!(
        workspace.display_name(left).as_deref(),
        Some("left/shared/web")
    );
    assert_eq!(
        workspace.display_name(right).as_deref(),
        Some("right/shared/web")
    );
}

#[test]
fn an_unknown_project_has_no_name() {
    assert_eq!(Workspace::default().display_name(ProjectId(7)), None);
}

#[test]
fn a_path_in_the_home_folder_starts_with_a_tilde() {
    let home = std::fs::canonicalize(dirs_home()).unwrap();
    let inside = home.join("Developer").join("web");
    assert_eq!(
        workspace::home_relative(&inside),
        std::path::Path::new("~")
            .join("Developer")
            .join("web")
            .display()
            .to_string()
    );
    assert_eq!(workspace::home_relative(&home), "~");
    let outside = std::path::Path::new("/opt/web");
    assert_eq!(
        workspace::home_relative(outside),
        outside.display().to_string()
    );
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(Into::into)
        .expect("HOME is set")
}
