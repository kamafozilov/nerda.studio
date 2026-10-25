//! The composer's project menu: No project once, at its top, however the
//! workspace file holds the computer.

use std::path::Path;

use crate::scene::{Scene, Settings};

/// A workspace an older build saved: the computer lost its kind and reads
/// as a folder at home, titled No project, beside a folder project whose
/// draft is on screen.
fn saved_by_an_older_build(data: &Path) {
    let web = data.join("web");
    std::fs::create_dir_all(&web).unwrap();
    let canonical = |path: &Path| std::fs::canonicalize(path).unwrap().display().to_string();
    let home = canonical(&dirs::home_dir().unwrap());
    std::fs::write(
        data.join("workspace.json"),
        format!(
            r#"{{"version": 1, "next_id": 4,
                "projects": [
                    {{"id": 0, "title": "web", "path": {web:?}, "created_at": 1}},
                    {{"id": 2, "title": "No project", "path": {home:?}, "created_at": 1}}
                ],
                "sessions": [
                    {{"id": 1, "project": 0, "title": "New session", "created_at": 1}},
                    {{"id": 3, "project": 2, "title": "New session", "created_at": 1}}
                ],
                "tabs": [1], "selected_session": 1}}"#,
            web = canonical(&web),
        ),
    )
    .unwrap();
}

#[test]
fn the_project_menu_offers_no_project_once() {
    let mut scene = Scene::open_with("project-menu", Settings::default(), saved_by_an_older_build);
    scene.shot("draft");
    scene.click(432., 450.);
    scene.wait(400);
    let menu = scene.shot("menu");
    // No project, web, New project: a second No project row would carry the
    // menu's surface on past its last row, down to 645.
    let surface = menu.at(560., 600.);
    assert_ne!(
        menu.at(560., 630.),
        surface,
        "the menu ends after three rows"
    );
    assert_ne!(
        menu.at(560., 645.),
        surface,
        "the menu ends after three rows"
    );
}
