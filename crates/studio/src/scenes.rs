//! The app's scenes: each opens the real window on no screen, works it as a
//! person would, and checks what it drew. `cargo test -p studio scenes` runs
//! them; each writes its frames to `target/scenes/<scene>/`.

use crate::scene::{Scene, Settings};

/// Relative luminance of an sRGB pixel, 0 to 1.
fn luminance([r, g, b, _]: [u8; 4]) -> f32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

#[test]
fn cmd_b_folds_the_sidebar_and_brings_it_back() {
    let mut scene = Scene::open("sidebar-toggle");
    assert!(scene.read(|shell, _| shell.sidebar_open()));
    let open = scene.shot("open");
    scene.keys("cmd-b");
    scene.wait(800);
    assert!(!scene.read(|shell, _| shell.sidebar_open()));
    let closed = scene.shot("closed");
    assert_ne!(
        open.at(120., 300.),
        closed.at(120., 300.),
        "the sidebar's place changes once it folds"
    );
    scene.keys("cmd-b");
    scene.wait(800);
    assert!(scene.read(|shell, _| shell.sidebar_open()));
    assert_eq!(scene.shot("open-again").at(120., 300.), open.at(120., 300.));
}

#[test]
fn the_sidebar_button_in_the_top_row_keeps_its_click() {
    let mut scene = Scene::open("sidebar-button");
    scene.click(100., 18.);
    scene.wait(800);
    assert!(!scene.read(|shell, _| shell.sidebar_open()));
    scene.shot("closed");
}

#[test]
fn dark_draws_darker_than_light() {
    let light = Scene::open("light").shot("window");
    let mut dark = Scene::open_with(
        "dark",
        Settings {
            dark: true,
            ..Settings::default()
        },
        |_| {},
    );
    let dark = dark.shot("window");
    assert!(luminance(dark.at(600., 400.)) < luminance(light.at(600., 400.)));
}

#[test]
fn a_corrupt_workspace_is_set_aside_not_overwritten() {
    let mut scene = Scene::open_with("corrupt-workspace", Settings::default(), |data| {
        std::fs::write(data.join("workspace.json"), "{ not json").unwrap();
    });
    scene.shot("window");
    let data = scene.data();
    let aside: Vec<_> = std::fs::read_dir(&data)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("corrupt-"))
        .collect();
    assert_eq!(
        aside.len(),
        1,
        "the corrupt file sits aside in {}",
        data.display()
    );
    assert_eq!(
        std::fs::read_to_string(data.join(&aside[0])).unwrap(),
        "{ not json"
    );
}

#[test]
fn the_window_holds_at_its_minimum_size_in_dark() {
    let mut scene = Scene::open("minimum-size-dark");
    let light = scene.shot("light");
    scene.switch(Settings {
        dark: true,
        ..Settings::default()
    });
    let dark = scene.shot("dark");
    assert!(luminance(dark.at(600., 400.)) < luminance(light.at(600., 400.)));
    scene.resize(720., 480.);
    let small = scene.shot("minimum");
    assert_eq!(small.image.width() as f32, 720. * small.scale);
    assert_eq!(
        small.at(500., 300.),
        dark.at(600., 400.),
        "the card fills the smaller window"
    );
}
