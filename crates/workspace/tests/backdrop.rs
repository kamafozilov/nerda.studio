mod support;

use support::{TempDir, load_json};
use workspace::{Backdrop, BackdropStyle, BackdropTint, Workspace};

#[test]
fn a_file_from_before_backdrops_reads_the_default() {
    let workspace = load_json(r#"{"version": 1}"#);
    assert_eq!(workspace.backdrop(), Backdrop::default());
    assert_eq!(workspace.backdrop().style, BackdropStyle::Horizon);
    assert!(!workspace.backdrop().moving);
}

#[test]
fn a_chosen_backdrop_survives_saving_and_loading() {
    let dir = TempDir::new();
    let path = dir.path().join("workspace.json");
    let mut workspace = Workspace::default();
    let chosen = Backdrop {
        style: BackdropStyle::Contours,
        tint: BackdropTint::Warm,
        moving: true,
    };
    workspace.set_backdrop(chosen);
    workspace.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains(r#""style": "contours""#), "{saved}");
    assert_eq!(Workspace::load(&path).unwrap().backdrop(), chosen);
}

#[test]
fn every_style_and_tint_keeps_its_name() {
    for style in BackdropStyle::ALL {
        for tint in BackdropTint::ALL {
            let mut workspace = Workspace::default();
            let chosen = Backdrop {
                style,
                tint,
                moving: false,
            };
            workspace.set_backdrop(chosen);
            let bytes = workspace.encode().unwrap();
            let json = String::from_utf8(bytes).unwrap();
            assert_eq!(load_json(&json).backdrop(), chosen);
        }
    }
}

#[test]
fn names_from_a_newer_build_read_as_the_default_and_the_rest_loads() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 3,
            "projects": [{"id": 1, "title": "one", "path": "/one", "created_at": 0}],
            "backdrop": {"style": "aurora", "tint": "neon", "moving": true, "speed": 2}}"#,
    );
    assert_eq!(workspace.backdrop().style, BackdropStyle::Horizon);
    assert_eq!(workspace.backdrop().tint, BackdropTint::Gray);
    assert!(workspace.backdrop().moving);
    assert_eq!(workspace.projects().len(), 1);
}
