//! A page opened while a new session's draft is the only tab takes that tab,
//! instead of opening a second one beside it.

use std::path::Path;

use workspace::{Page, Tab};

use crate::scene::{Scene, Settings};

const SETTINGS: Tab = Tab::Page(Page::Settings);

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
fn settings_opened_from_the_draft_alone_takes_its_tab() {
    let mut scene = Scene::open_with("draft-tab-settings", Settings::default(), draft_alone);
    let before = tabs(&mut scene);
    assert!(
        matches!(before[..], [Tab::Session(_)]),
        "the draft alone: {before:?}"
    );
    scene.shot("draft");
    scene.keys("cmd-,");
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [SETTINGS]);
    scene.shot("settings");
    // Again, and fast: still the one tab.
    scene.keys("cmd-,");
    scene.keys("cmd-,");
    assert_eq!(tabs(&mut scene), [SETTINGS]);
    // Closing it brings the same draft back.
    scene.keys("cmd-w");
    scene.wait(300);
    assert_eq!(tabs(&mut scene), before);
    scene.shot("draft-again");
}

#[test]
fn settings_opened_from_a_used_session_opens_beside_it() {
    let mut scene = Scene::conversation("draft-tab-used", Settings::default(), &["Reply."]);
    scene.send("first");
    scene.wait_for_agent();
    let before = tabs(&mut scene);
    scene.keys("cmd-,");
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [before, vec![SETTINGS]].concat());
    scene.shot("beside");
}

#[test]
fn a_page_clicked_in_the_sidebar_takes_the_drafts_tab_then_opens_beside() {
    let mut scene = Scene::open_with("draft-tab-click", Settings::default(), draft_alone);
    // The Settings row at the sidebar's foot.
    scene.click(60., 780.);
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [SETTINGS]);
    // Notes, under New session: a second page opens beside the first.
    scene.click(56., 91.);
    scene.wait(300);
    assert_eq!(tabs(&mut scene), [SETTINGS, Tab::Page(Page::Notes)]);
    scene.shot("two-pages");
}
