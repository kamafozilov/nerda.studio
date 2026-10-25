//! The effort pill and its menu: a gauge whose needle and color follow the
//! level, Low to Max, in dark and light.

use std::path::Path;

use crate::scene::{Scene, Settings};

/// A folder project whose draft is on screen, its effort at `effort`.
fn draft_at(effort: &'static str) -> impl FnOnce(&Path) {
    move |data| {
        let web = data.join("web");
        std::fs::create_dir_all(&web).unwrap();
        let web = std::fs::canonicalize(&web).unwrap().display().to_string();
        std::fs::write(
            data.join("workspace.json"),
            format!(
                r#"{{"version": 1, "next_id": 2,
                    "projects": [{{"id": 0, "title": "web", "path": {web:?}, "created_at": 1}}],
                    "sessions": [{{"id": 1, "project": 0, "title": "New session", "created_at": 1,
                        "choice": {{"agent": "claude", "model": "opus", "effort": "{effort}", "access": "full-access"}}}}],
                    "tabs": [1], "selected_session": 1}}"#
            ),
        )
        .unwrap();
    }
}

/// Whether the pill's gauge shows `color`: some pixel of it within a few
/// steps, its antialiased edge aside.
fn pill_shows(shot: &crate::scene::Shot, color: u32) -> bool {
    let [r, g, b] = [color >> 16, color >> 8, color].map(|c| (c & 0xff) as i32);
    (0..16).any(|dy| {
        (0..16).any(|dx| {
            let [pr, pg, pb, _] = shot.at(538. + dx as f32, 401. + dy as f32);
            (i32::from(pr) - r).abs() + (i32::from(pg) - g).abs() + (i32::from(pb) - b).abs() < 24
        })
    })
}

/// The effort `workspace.json` holds for the draft, once saved.
fn saved_effort(scene: &mut Scene) -> String {
    scene.wait(100);
    let text = std::fs::read_to_string(scene.data().join("workspace.json")).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    file["sessions"][0]["choice"]["effort"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Opens the menu, picks Max with the pointer, then Low from the keyboard;
/// `colors` are the gauge's High, Max and Low.
fn effort_menu(name: &str, settings: Settings, [high, max, low]: [u32; 3]) {
    let mut scene = Scene::open_with(name, settings, draft_at("high"));
    assert!(pill_shows(&scene.shot("high"), high), "High is blue");
    scene.click(570., 408.);
    scene.wait(400);
    let menu = scene.shot("menu");
    // Medium's row alone carries the tag, at its end.
    assert_ne!(menu.at(700., 289.), menu.at(700., 261.), "Medium is tagged");
    assert_eq!(menu.at(700., 317.), menu.at(700., 261.), "High is not");
    scene.hover(600., 289.);
    scene.wait(150);
    scene.shot("menu-medium-highlighted");
    scene.click(600., 373.);
    scene.wait(200);
    assert!(pill_shows(&scene.shot("max"), max), "Max is coral");
    assert_eq!(saved_effort(&mut scene), "max");

    // From the keyboard: the menu opens on the pick, so up goes to Low.
    scene.click(570., 408.);
    scene.wait(400);
    scene.keys("up");
    scene.keys("up");
    scene.keys("up");
    scene.keys("up");
    scene.keys("up");
    scene.keys("enter");
    scene.wait(200);
    assert!(pill_shows(&scene.shot("low"), low), "Low is gray");
    assert_eq!(saved_effort(&mut scene), "low");
}

#[test]
fn the_effort_gauge_follows_the_level_in_dark() {
    effort_menu(
        "effort-menu-dark",
        Settings {
            dark: true,
            ..Settings::default()
        },
        [0x80c5ff, 0xffa383, 0xa0a0a1],
    );
}

#[test]
fn the_effort_gauge_follows_the_level_in_light() {
    effort_menu(
        "effort-menu-light",
        Settings::default(),
        [0x007ac1, 0xba4e25, 0x505358],
    );
}
