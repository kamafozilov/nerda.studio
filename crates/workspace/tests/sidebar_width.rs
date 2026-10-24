mod support;

use support::{TempDir, load_json};
use workspace::Workspace;

#[test]
fn a_file_from_before_resizing_keeps_the_default_width() {
    assert_eq!(load_json(r#"{"version": 1}"#).sidebar_width(), None);
}

#[test]
fn a_dragged_width_survives_saving_and_loading() {
    let dir = TempDir::new();
    let path = dir.path().join("workspace.json");
    let mut workspace = Workspace::default();
    workspace.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("sidebar_width"), "{saved}");

    workspace.set_sidebar_width(Some(312.));
    workspace.save(&path).unwrap();
    assert_eq!(Workspace::load(&path).unwrap().sidebar_width(), Some(312.));

    workspace.set_sidebar_width(None);
    workspace.save(&path).unwrap();
    assert_eq!(Workspace::load(&path).unwrap().sidebar_width(), None);
}
