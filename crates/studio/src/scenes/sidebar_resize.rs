//! The sidebar's edge: dragging it, its bounds, a double-click back to the
//! default, and the width kept from one launch to the next.

use std::path::Path;

use workspace::{AddedProject, Timestamp, Workspace};

use crate::scene::{Scene, Settings};

/// The sidebar's width as the shell holds it, in points.
fn sidebar_width(scene: &mut Scene) -> f32 {
    scene.read(|shell, _| f32::from(shell.sidebar_width()))
}

/// The width `workspace.json` holds, once the save reached the disk.
fn saved_sidebar_width(scene: &mut Scene) -> Option<f64> {
    scene.wait(100);
    let text = std::fs::read_to_string(scene.data().join("workspace.json")).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    file.get("sidebar_width")
        .and_then(serde_json::Value::as_f64)
}

/// A workspace whose sidebar was left `width` wide.
fn left_at(width: &str) -> impl FnOnce(&Path) {
    move |data| {
        std::fs::write(
            data.join("workspace.json"),
            format!(r#"{{"version": 1, "sidebar_width": {width}}}"#),
        )
        .unwrap();
    }
}

#[test]
fn dragging_the_edge_resizes_the_sidebar_within_bounds_and_saves_it() {
    let mut scene = Scene::open("sidebar-resize");
    assert_eq!(sidebar_width(&mut scene), 264.);
    let default = scene.shot("default");
    scene.drag((264., 400.), (340., 400.));
    assert_eq!(sidebar_width(&mut scene), 340.);
    let wider = scene.shot("wider");
    assert_ne!(
        default.at(300., 400.),
        wider.at(300., 400.),
        "the card gave way"
    );
    assert_eq!(
        wider.at(300., 400.),
        default.at(200., 400.),
        "the sidebar took it"
    );
    assert_eq!(saved_sidebar_width(&mut scene), Some(340.));

    // Past either bound it stops there, let go outside the window too.
    scene.drag((340., 400.), (1100., 400.));
    assert_eq!(sidebar_width(&mut scene), 400.);
    scene.shot("widest");
    scene.drag((400., 400.), (-60., 400.));
    assert_eq!(sidebar_width(&mut scene), 216.);
    scene.shot("narrowest");
    assert_eq!(saved_sidebar_width(&mut scene), Some(216.));
    // The drag ended where the button went up: the pointer moves nothing.
    scene.hover(320., 400.);
    assert_eq!(sidebar_width(&mut scene), 216.);

    // Grabbed off center, the edge keeps its place under the pointer.
    scene.drag((219., 300.), (259., 300.));
    assert_eq!(sidebar_width(&mut scene), 256.);
    for to in [300., 250., 288.] {
        let from = sidebar_width(&mut scene);
        scene.drag((from, 500.), (to, 500.));
    }
    assert_eq!(sidebar_width(&mut scene), 288.);

    // A double-click puts the default back, saved as none, and a second
    // keeps it.
    scene.double_click(288., 400.);
    assert_eq!(sidebar_width(&mut scene), 264.);
    assert_eq!(saved_sidebar_width(&mut scene), None);
    scene.double_click(264., 400.);
    assert_eq!(sidebar_width(&mut scene), 264.);
    assert_eq!(
        scene.shot("default-again").at(300., 400.),
        default.at(300., 400.)
    );
}

#[test]
fn the_sidebar_opens_at_the_width_it_was_left() {
    let mut scene = Scene::open_with("sidebar-left-wide", Settings::default(), left_at("312"));
    assert_eq!(sidebar_width(&mut scene), 312.);
    scene.shot("window");
}

#[test]
fn a_saved_width_out_of_bounds_opens_within_them() {
    for (name, saved, width) in [
        ("sidebar-saved-huge", "1e9", 400.),
        ("sidebar-saved-negative", "-20", 216.),
        ("sidebar-saved-fraction", "300.7", 301.),
    ] {
        let mut scene = Scene::open_with(name, Settings::default(), left_at(saved));
        assert_eq!(sidebar_width(&mut scene), width, "{saved}");
    }
}

#[test]
fn the_closed_sidebars_edge_drags_nothing() {
    let mut scene = Scene::open("sidebar-resize-closed");
    scene.keys("cmd-b");
    scene.wait(800);
    scene.drag((264., 400.), (350., 400.));
    assert_eq!(sidebar_width(&mut scene), 264.);
    assert!(!scene.read(|shell, _| shell.sidebar_open()));
    // Mid-slide the edge isn't there either.
    scene.keys("cmd-b");
    scene.drag((264., 400.), (350., 400.));
    assert_eq!(sidebar_width(&mut scene), 264.);
}

/// The edge starts under the top row, whose drag moves the window: the test
/// platform can't move one, so only the row's first point below is driven.
#[test]
fn the_edge_starts_right_under_the_top_row() {
    let mut scene = Scene::open("sidebar-resize-top-row");
    scene.click(264., 22.);
    assert_eq!(sidebar_width(&mut scene), 264.);
    scene.drag((264., 45.), (340., 45.));
    assert_eq!(sidebar_width(&mut scene), 340.);
}

#[test]
fn a_resized_sidebar_folds_and_comes_back_at_its_width() {
    let mut scene = Scene::open("sidebar-resize-toggle");
    scene.drag((264., 400.), (360., 400.));
    let open = scene.shot("open");
    scene.keys("cmd-b");
    scene.wait(800);
    scene.shot("closed");
    scene.keys("cmd-b");
    scene.wait(800);
    assert_eq!(sidebar_width(&mut scene), 360.);
    let again = scene.shot("open-again");
    assert_eq!(again.at(340., 400.), open.at(340., 400.));
    assert_eq!(again.at(380., 400.), open.at(380., 400.));
}

#[test]
fn the_sidebar_resizes_in_dark_and_solid_under_reduce_motion() {
    let settings = Settings {
        dark: true,
        reduce_motion: true,
        reduce_transparency: true,
    };
    let mut scene = Scene::open_with("sidebar-resize-dark", settings, |_| {});
    scene.drag((264., 400.), (380., 400.));
    assert_eq!(sidebar_width(&mut scene), 380.);
    scene.shot("wider");
    // Reduce motion folds it at once, and its edge is back at once.
    scene.keys("cmd-b");
    scene.keys("cmd-b");
    scene.drag((380., 400.), (300., 400.));
    assert_eq!(sidebar_width(&mut scene), 300.);
    scene.double_click(300., 400.);
    assert_eq!(sidebar_width(&mut scene), 264.);
}

#[test]
fn the_widest_sidebar_leaves_the_card_room_in_the_smallest_window() {
    let mut scene = Scene::open_at(
        "sidebar-widest-smallest-window",
        Settings::default(),
        (720., 480.),
        left_at("400"),
    );
    let shot = scene.shot("window");
    assert_ne!(
        shot.at(390., 300.),
        shot.at(500., 300.),
        "the card stands right of it"
    );
    assert_eq!(
        shot.at(410., 300.),
        shot.at(700., 300.),
        "the card spans the rest"
    );
}

#[test]
fn the_narrowest_sidebar_holds_beside_a_session_with_a_long_project_name() {
    let mut scene = Scene::open_with("sidebar-narrowest-session", Settings::default(), |data| {
        let now = Timestamp::now();
        let mut workspace = Workspace::default();
        for name in [
            "Juda uzun loyiha nomi oʻzbekcha yozilgan va u sigʻmaydi",
            "Очень длинное название проекта на кириллице, которое не помещается",
        ] {
            let folder = data.with_file_name(name);
            std::fs::create_dir_all(&folder).unwrap();
            let (AddedProject::Added(id) | AddedProject::Existing(id)) =
                workspace.add_project(&folder, now).unwrap();
            let session = workspace.new_session(id, now).unwrap();
            workspace.open_session(session);
        }
        workspace.set_sidebar_width(Some(216.));
        workspace.save(&data.join("workspace.json")).unwrap();
    });
    assert_eq!(sidebar_width(&mut scene), 216.);
    scene.shot("window");
}
