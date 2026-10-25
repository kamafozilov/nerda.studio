//! A page opened while a new session's draft is the only tab takes that tab,
//! instead of opening a second one beside it. Settings, which opens over the
//! whole window, leaves the tabs alone.

use std::path::Path;

use workspace::{Page, Tab};

use crate::scene::{Scene, Settings};

const NOTES: Tab = Tab::Page(Page::Notes);
const USAGE: Tab = Tab::Page(Page::Usage);

/// The sidebar's Notes row, and Usage's button in its footer, in points.
const NOTES_ROW: (f32, f32) = (56., 91.);
const USAGE_BUTTON: (f32, f32) = (213., 780.);

fn tabs(scene: &mut Scene) -> Vec<Tab> {
    scene.read(|shell, cx| shell.open_tabs(cx))
}

/// A folder project whose new session's draft is the only tab.
fn draft_alone(data: &Path) {
    let web = data.join("web");
    std::fs::create_dir_all(&web).unwrap();
    let web = std::fs::canonicalize(&web).unwrap().display().to_string();
    std::fs::write(
        data.join("workspace.json"),
        format!(
            r#"{{"version": 1, "next_id": 2,
                "projects": [{{"id": 0, "title": "web", "path": {web:?}, "created_at": 1}}],
                "sessions": [{{"id": 1, "project": 0, "title": "New session", "created_at": 1}}],
                "tabs": [1], "selected_session": 1}}"#
        ),
    )
    .unwrap();
}

#[test]
fn a_page_opened_from_the_draft_alone_takes_its_tab() {
    let mut scene = Scene::open_with("draft-tab-page", Settings::default(), draft_alone);
    let before = tabs(&mut scene);
    assert!(
        matches!(before[..], [Tab::Session(_)]),
        "the draft alone: {before:?}"
    );
    scene.shot("draft");
    scene.click(NOTES_ROW.0, NOTES_ROW.1);
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [NOTES]);
    scene.shot("notes");
    // Again, and fast: still the one tab.
    scene.click(NOTES_ROW.0, NOTES_ROW.1);
    scene.click(NOTES_ROW.0, NOTES_ROW.1);
    assert_eq!(tabs(&mut scene), [NOTES]);
    // Closing it brings the same draft back.
    scene.keys("cmd-w");
    scene.wait(300);
    assert_eq!(tabs(&mut scene), before);
    scene.shot("draft-again");
}

#[test]
fn a_page_opened_from_a_used_session_opens_beside_it() {
    let mut scene = Scene::conversation("draft-tab-used", Settings::default(), &["Reply."]);
    scene.send("first");
    scene.wait_for_agent();
    let before = tabs(&mut scene);
    scene.click(NOTES_ROW.0, NOTES_ROW.1);
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [before, vec![NOTES]].concat());
    scene.shot("beside");
}

#[test]
fn the_footers_page_takes_the_drafts_tab_then_a_second_opens_beside() {
    let mut scene = Scene::open_with("draft-tab-click", Settings::default(), draft_alone);
    scene.click(USAGE_BUTTON.0, USAGE_BUTTON.1);
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [USAGE]);
    scene.click(NOTES_ROW.0, NOTES_ROW.1);
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [USAGE, NOTES]);
    scene.shot("two-pages");
}

#[test]
fn settings_over_the_draft_alone_leaves_its_tab() {
    let mut scene = Scene::open_with("draft-tab-settings", Settings::default(), draft_alone);
    let before = tabs(&mut scene);
    scene.keys("cmd-,");
    scene.wait(300);
    assert!(scene.read(|shell, _| shell.settings_open()));
    assert_eq!(tabs(&mut scene), before);
    scene.keys("cmd-,");
    assert!(!scene.read(|shell, _| shell.settings_open()));
    assert_eq!(tabs(&mut scene), before);
}
