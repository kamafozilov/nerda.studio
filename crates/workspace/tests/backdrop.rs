mod support;

use support::{TempDir, load_json};
use workspace::{Backdrop, BackdropStrength, BackdropStyle, BackdropTint, Workspace};

#[test]
fn a_file_from_before_backdrops_reads_the_default() {
    let workspace = load_json(r#"{"version": 1}"#);
    assert_eq!(*workspace.backdrop(), Backdrop::default());
    assert_eq!(workspace.backdrop().strength, BackdropStrength::Medium);
    assert_eq!(workspace.backdrop().image, None);
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
        tint: BackdropTint::Image,
        strength: BackdropStrength::Bold,
        moving: true,
        image: Some("4f1c.jpg".to_owned()),
    };
    workspace.set_backdrop(chosen.clone());
    workspace.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains(r#""style": "contours""#), "{saved}");
    assert_eq!(*Workspace::load(&path).unwrap().backdrop(), chosen);
}

#[test]
fn every_style_and_tint_keeps_its_name() {
    for style in BackdropStyle::ALL {
        for tint in BackdropTint::ALL {
            let mut workspace = Workspace::default();
            for strength in BackdropStrength::ALL {
                let chosen = Backdrop {
                    style,
                    tint,
                    strength,
                    ..Backdrop::default()
                };
                workspace.set_backdrop(chosen.clone());
                let bytes = workspace.encode().unwrap();
                let json = String::from_utf8(bytes).unwrap();
                assert_eq!(*load_json(&json).backdrop(), chosen);
            }
        }
    }
}

#[test]
fn names_from_a_newer_build_read_as_the_default_and_the_rest_loads() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 3,
            "projects": [{"id": 1, "title": "one", "path": "/one", "created_at": 0}],
            "backdrop": {"style": "aurora", "tint": "neon", "strength": "max",
                         "moving": true, "speed": 2}}"#,
    );
    assert_eq!(workspace.backdrop().style, BackdropStyle::Horizon);
    assert_eq!(workspace.backdrop().tint, BackdropTint::Gray);
    assert_eq!(workspace.backdrop().strength, BackdropStrength::Medium);
    assert!(workspace.backdrop().moving);
    assert_eq!(workspace.projects().len(), 1);
}

#[test]
fn an_image_name_that_would_leave_the_backdrop_folder_reads_as_none() {
    for name in [
        "../secret.png",
        "/etc/hosts",
        "a/b.png",
        ".hidden.png",
        "",
        "c:\\x.png",
    ] {
        let json = format!(
            r#"{{"version": 1, "backdrop": {{"image": {}}}}}"#,
            serde_json::to_string(name).unwrap()
        );
        assert_eq!(load_json(&json).backdrop().image, None, "{name}");
    }
    let kept = load_json(r#"{"version": 1, "backdrop": {"image": "4f1c.png"}}"#);
    assert_eq!(kept.backdrop().image.as_deref(), Some("4f1c.png"));
}

#[test]
fn the_image_colors_need_an_image() {
    let mut backdrop = Backdrop {
        tint: BackdropTint::Image,
        ..Backdrop::default()
    };
    assert_eq!(backdrop.tint_in_use(), BackdropTint::Gray);
    backdrop.image = Some("a.png".to_owned());
    assert_eq!(backdrop.tint_in_use(), BackdropTint::Image);
}
