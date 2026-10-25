//! Settings over the whole window: its menu in the sidebar's place, General's
//! two settings in the card, saved with the workspace.

use std::path::Path;

use workspace::{OnLaunch, Preferences};

use crate::scene::{Scene, Settings};

/// A folder project with one used session, open, and Notes beside it, with
/// `preferences` as the file holds them.
fn saved(preferences: &'static str) -> impl FnOnce(&Path) {
    move |data: &Path| {
        let web = data.join("web");
        std::fs::create_dir_all(&web).unwrap();
        let web = std::fs::canonicalize(&web).unwrap().display().to_string();
        std::fs::write(
            data.join("workspace.json"),
            format!(
                r#"{{"version": 1, "next_id": 3,
                "projects": [{{"id": 0, "title": "web", "path": {web:?}, "created_at": 1}}],
                "sessions": [{{"id": 1, "project": 0, "title": "Fix tab logic", "created_at": 1,
                    "last_activity_at": 5}}],
                "tabs": [1], "pages": [{{"page": "notes", "at": 1}}],
                "selected_session": 1, "project_on_screen": 0{preferences}}}"#
            ),
        )
        .unwrap();
    }
}

fn preferences(scene: &mut Scene) -> Preferences {
    scene.read(|shell, cx| shell.preferences(cx))
}

/// The preferences as the data folder's file holds them.
fn on_disk(scene: &Scene) -> serde_json::Value {
    let bytes = std::fs::read(scene.data().join("workspace.json")).unwrap();
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["preferences"].clone()
}

fn section(scene: &mut Scene) -> &'static str {
    scene.read(|shell, cx| shell.settings().read(cx).section_title())
}

/// Where things stand at 1200 by 800, in points.
const BACK_ROW: (f32, f32) = (64., 62.);
const SEARCH: (f32, f32) = (82., 91.);
const KEYBOARD_ROW: (f32, f32) = (67., 186.);
const PICKER: (f32, f32) = (1008., 257.);
const SWITCH: (f32, f32) = (1056., 407.);

fn open_settings(name: &str, settings: Settings) -> Scene {
    let mut scene = Scene::open_with(name, settings, saved(""));
    scene.keys("cmd-,");
    scene.wait(300);
    assert!(is_open(&mut scene));
    scene
}

fn is_open(scene: &mut Scene) -> bool {
    scene.read(|shell, _| shell.settings_open())
}

#[test]
fn settings_opens_on_general_and_leaves_the_tabs_as_they_were() {
    let mut scene = Scene::open_with("settings-general", Settings::default(), saved(""));
    let tabs = scene.read(|shell, cx| shell.open_tabs(cx));
    scene.keys("cmd-,");
    scene.wait(300);
    assert!(is_open(&mut scene));
    assert_eq!(section(&mut scene), "General");
    scene.shot("light");
    // Back in the menu leaves it, the tabs as they were.
    scene.click(BACK_ROW.0, BACK_ROW.1);
    scene.wait(300);
    assert!(!is_open(&mut scene));
    assert_eq!(scene.read(|shell, cx| shell.open_tabs(cx)), tabs);
    scene.shot("back");
}

#[test]
fn escape_cmd_w_cmd_comma_and_a_new_session_leave_settings() {
    let mut scene = open_settings("settings-leave", Settings::default());
    scene.keys("escape");
    assert!(!is_open(&mut scene));
    scene.keys("cmd-, cmd-w");
    assert!(!is_open(&mut scene));
    let tabs = scene.read(|shell, cx| shell.open_tabs(cx));
    assert_eq!(tabs.len(), 2, "Cmd-W left Settings, closing no tab");
    scene.keys("cmd-, cmd-,");
    assert!(!is_open(&mut scene));
    scene.keys("cmd-, cmd-n");
    assert!(!is_open(&mut scene));
    scene.keys("cmd-, cmd-1");
    assert!(!is_open(&mut scene));
}

#[test]
fn the_switch_turns_off_asking_before_quitting_and_saves_it() {
    let mut scene = open_settings("settings-switch", Settings::default());
    assert!(preferences(&mut scene).confirm_quit);
    scene.click(SWITCH.0, SWITCH.1);
    scene.wait(30);
    scene.shot("sliding");
    scene.wait(500);
    assert!(!preferences(&mut scene).confirm_quit);
    scene.shot("off");
    scene.wait(300);
    assert_eq!(on_disk(&scene)["confirm_quit"], false);
    // Two quick clicks flip it twice: back off.
    scene.click(SWITCH.0, SWITCH.1);
    scene.click(SWITCH.0, SWITCH.1);
    scene.wait(500);
    assert!(!preferences(&mut scene).confirm_quit);
    scene.click(SWITCH.0, SWITCH.1);
    scene.wait(500);
    assert!(preferences(&mut scene).confirm_quit);
    // On, Cmd-Q asks for a second press.
    scene.keys("cmd-q");
    assert!(scene.read(|shell, _| shell.quit_hint_shown()));
}

#[test]
fn the_launch_picker_sets_what_the_window_opens_to() {
    let mut scene = open_settings("settings-picker", Settings::default());
    scene.click(PICKER.0, PICKER.1);
    scene.wait(100);
    scene.shot("menu");
    // Escape closes the menu alone, and Settings stays.
    scene.keys("escape");
    assert!(is_open(&mut scene));
    scene.click(PICKER.0, PICKER.1);
    scene.keys("down down enter");
    scene.wait(300);
    assert_eq!(preferences(&mut scene).on_launch, OnLaunch::NewSession);
    assert_eq!(on_disk(&scene)["on_launch"], "new-session");
    scene.shot("new-session");
}

#[test]
fn a_section_chosen_stays_between_visits() {
    let mut scene = open_settings("settings-sections", Settings::default());
    scene.click(KEYBOARD_ROW.0, KEYBOARD_ROW.1);
    scene.wait(100);
    assert_eq!(section(&mut scene), "Keyboard");
    scene.shot("keyboard");
    scene.keys("escape cmd-,");
    assert_eq!(section(&mut scene), "Keyboard");
}

#[test]
fn the_search_finds_settings_and_shows_their_section() {
    let mut scene = open_settings("settings-search", Settings::default());
    scene.click(KEYBOARD_ROW.0, KEYBOARD_ROW.1);
    scene.click(SEARCH.0, SEARCH.1);
    scene.keys("q u i t");
    scene.wait(100);
    scene.shot("found");
    // The one find, under General's title.
    scene.click(KEYBOARD_ROW.0, 165.);
    scene.wait(100);
    assert_eq!(section(&mut scene), "General");
    scene.shot("shown");
    // Nothing found says so; Escape clears the search, then leaves.
    scene.click(SEARCH.0, SEARCH.1);
    scene.keys("z z z");
    scene.wait(100);
    scene.shot("nothing");
    scene.keys("escape");
    assert!(is_open(&mut scene));
    scene.keys("escape");
    assert!(!is_open(&mut scene));
}

#[test]
fn tab_reaches_every_section_and_control() {
    let mut scene = open_settings("settings-keyboard", Settings::default());
    // The sections, then the picker, then the switch; Space flips it.
    for _ in 0..10 {
        scene.keys("tab");
    }
    scene.shot("switch-focused");
    scene.press("space");
    scene.wait(500);
    assert!(!preferences(&mut scene).confirm_quit);
    // Back to Keyboard's row, and Enter shows it.
    for _ in 0..7 {
        scene.keys("shift-tab");
    }
    scene.press("enter");
    assert_eq!(section(&mut scene), "Keyboard");
}

#[test]
fn settings_holds_in_dark_at_the_minimum_size_and_with_the_sidebar_closed() {
    let dark = Settings {
        dark: true,
        ..Settings::default()
    };
    let mut scene = open_settings("settings-dark", dark);
    scene.click(SWITCH.0, SWITCH.1);
    scene.wait(500);
    scene.shot("off");
    scene.click(SWITCH.0, SWITCH.1);
    scene.wait(500);
    scene.shot("on");
    let mut small = Scene::open_at(
        "settings-small",
        Settings::default(),
        (720., 480.),
        saved(""),
    );
    small.keys("cmd-,");
    small.wait(300);
    small.shot("minimum");
    small.keys("cmd-b");
    small.wait(800);
    assert!(small.read(|shell, _| shell.settings_open()));
    small.shot("sidebar-closed");
}

#[test]
fn new_session_on_launch_opens_a_draft_alone() {
    let mut scene = Scene::open_with(
        "settings-launch-new",
        Settings::default(),
        saved(r#", "preferences": {"on_launch": "new-session", "confirm_quit": true}"#),
    );
    let tabs = scene.read(|shell, cx| shell.open_tabs(cx));
    assert_eq!(tabs.len(), 1, "one tab: {tabs:?}");
    let draft = tabs[0].session().expect("a session");
    assert_ne!(draft.0, 1, "a new draft, not the used session");
    scene.shot("draft");
}

#[test]
fn a_preference_from_a_newer_build_reads_as_the_default() {
    let mut scene = Scene::open_with(
        "settings-newer",
        Settings::default(),
        saved(r#", "preferences": {"on_launch": "split-view", "confirm_quit": "ask"}"#),
    );
    assert_eq!(preferences(&mut scene), Preferences::default());
    assert_eq!(scene.read(|shell, cx| shell.open_tabs(cx)).len(), 2);
}

#[test]
fn under_reduce_motion_the_switch_is_off_at_once() {
    let still = Settings {
        reduce_motion: true,
        ..Settings::default()
    };
    let mut scene = open_settings("settings-switch-still", still);
    let on = scene.shot("on");
    scene.click(SWITCH.0, SWITCH.1);
    scene.wait(30);
    let off = scene.shot("off-at-once");
    let mut moving = open_settings("settings-switch-moving", Settings::default());
    moving.click(SWITCH.0, SWITCH.1);
    moving.wait(30);
    let sliding = moving.shot("sliding");
    // Where the knob rests while on: the track's own color once it is off.
    let knob_on = (SWITCH.0 + 6., SWITCH.1);
    assert_ne!(off.at(knob_on.0, knob_on.1), on.at(knob_on.0, knob_on.1));
    assert_ne!(
        off.at(SWITCH.0 - 6., SWITCH.1),
        sliding.at(SWITCH.0 - 6., SWITCH.1),
        "still, the knob is already at the start; moving, it is on its way"
    );
}

#[test]
fn settings_opens_before_the_first_project_and_leaves_to_the_welcome() {
    let mut scene = Scene::open("settings-no-projects");
    scene.keys("cmd-,");
    scene.wait(300);
    assert!(is_open(&mut scene));
    scene.shot("settings");
    scene.keys("escape");
    assert!(!is_open(&mut scene));
    assert!(scene.read(|shell, cx| shell.open_tabs(cx)).is_empty());
    scene.shot("welcome");
}
