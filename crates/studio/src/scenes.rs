//! The app's scenes: each opens the real window on no screen, works it as a
//! person would, and checks what it drew. `cargo test -p studio scenes` runs
//! them; each writes its frames to `target/scenes/<scene>/`.

use crate::scene::{Scene, Settings};

mod effort_menu;
mod project_menu;
mod sidebar_resize;

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

/// The first thing drawn on the card at `x`, going down from under the top
/// row, in points to the pixel.
fn first_mark(shot: &crate::scene::Shot, x: f32) -> Option<f32> {
    let card = shot.at(x, 50.);
    (100..1500)
        .map(|y| y as f32 / 2.)
        .find(|&y| shot.at(x, y) != card)
}

#[test]
fn a_message_sent_stands_at_the_top_as_the_first_did() {
    let mut scene = Scene::conversation(
        "pin-newest",
        Settings::default(),
        &["First reply.", "Second reply."],
    );
    scene.send("first");
    scene.wait_for_agent();
    let first = scene.shot("first");
    scene.send("second");
    scene.wait_for_agent();
    let second = scene.shot("second");
    // Right of the turn's header, where its hairline is the first mark.
    let line = first_mark(&first, 700.).expect("the first turn shows");
    assert!(
        line < 200.,
        "the first turn is at the top, its line at {line}"
    );
    assert_eq!(
        first_mark(&second, 700.),
        Some(line),
        "the second turn stands where the first did"
    );
}

/// How the conversation on screen keeps its rows: whether its newest
/// message holds at the top, and whether it follows the end.
fn holding(scene: &mut Scene) -> (bool, bool) {
    scene.read(|shell, cx| {
        let conversation = shell.conversation(cx).expect("a conversation shows");
        let conversation = conversation.read(cx);
        (conversation.holds_newest(), conversation.follows_end())
    })
}

/// A reply longer than the window: `lines` numbered paragraphs.
fn long_reply(lines: usize) -> String {
    (1..=lines)
        .map(|at| format!("Paragraph {at} of a reply longer than the view."))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[test]
fn a_message_sent_while_another_runs_ends_at_the_top() {
    let mut scene = Scene::conversation(
        "pin-queued",
        Settings::default(),
        &["First reply.", "Second reply."],
    );
    scene.send("first");
    scene.send("second");
    scene.wait_for_agent();
    scene.shot("both");
    assert_eq!(holding(&mut scene), (true, false));
    let mut fresh = Scene::conversation("pin-queued-fresh", Settings::default(), &["Reply."]);
    fresh.send("first");
    fresh.wait_for_agent();
    let line = first_mark(&fresh.shot("first"), 700.);
    assert_eq!(first_mark(&scene.shot("both-again"), 700.), line);
}

#[test]
fn a_turn_longer_than_the_view_follows_its_end_then_the_next_goes_to_the_top() {
    let long = long_reply(60);
    let mut scene = Scene::conversation(
        "pin-overflow",
        Settings::default(),
        &[long.as_str(), "Short reply."],
    );
    scene.send("first");
    scene.wait_for_agent();
    scene.shot("long");
    assert_eq!(
        holding(&mut scene),
        (false, true),
        "at the end, following it"
    );
    scene.send("second");
    scene.wait_for_agent();
    let short = scene.shot("short");
    assert_eq!(holding(&mut scene), (true, false));
    let mut fresh = Scene::conversation("pin-overflow-fresh", Settings::default(), &["Reply."]);
    fresh.send("first");
    fresh.wait_for_agent();
    assert_eq!(
        first_mark(&short, 700.),
        first_mark(&fresh.shot("first"), 700.),
        "after a long turn the next still stands at the top"
    );
}

#[test]
fn scrolling_lets_the_newest_message_go_and_the_end_takes_it_back() {
    let long = long_reply(30);
    let mut scene = Scene::conversation(
        "pin-scroll",
        Settings::default(),
        &[long.as_str(), "Short reply."],
    );
    scene.send("first");
    scene.wait_for_agent();
    scene.send("second");
    scene.wait_for_agent();
    let pinned = scene.shot("pinned");
    assert_eq!(holding(&mut scene), (true, false));
    scene.scroll(700., 400., 300.);
    scene.wait(50);
    let up = scene.shot("scrolled-up");
    assert_eq!(
        holding(&mut scene),
        (false, false),
        "it stays where it was taken"
    );
    assert_ne!(first_mark(&up, 700.), first_mark(&pinned, 700.));
    scene.scroll(700., 400., -2000.);
    scene.wait(50);
    let back = scene.shot("back-at-the-end");
    assert_eq!(
        holding(&mut scene),
        (true, false),
        "back at the end it holds again"
    );
    assert_eq!(first_mark(&back, 700.), first_mark(&pinned, 700.));
}

#[test]
fn the_newest_message_holds_its_place_in_the_smallest_window() {
    let mut fresh = Scene::conversation("pin-small-fresh", Settings::default(), &["Reply."]);
    fresh.send("first");
    fresh.wait_for_agent();
    let line = first_mark(&fresh.shot("first"), 600.);
    let mut scene = Scene::conversation_at(
        "pin-small",
        Settings::default(),
        (720., 480.),
        &["One.", "Two."],
    );
    scene.send("first");
    scene.wait_for_agent();
    scene.send("second");
    scene.wait_for_agent();
    let small = scene.shot("minimum");
    assert_eq!(holding(&mut scene), (true, false));
    assert_eq!(
        first_mark(&small, 600.),
        line,
        "as high as in a large window"
    );
    scene.keys("cmd-b");
    scene.wait(800);
    let closed = scene.shot("sidebar-closed");
    assert_eq!(holding(&mut scene), (true, false));
    assert_eq!(first_mark(&closed, 600.), line);
}

/// The widest run of the composer's box on any row, in points: in light
/// the box is white, or near it over a pattern, and nothing else is.
fn composer_width(shot: &crate::scene::Shot) -> f32 {
    let (width, height) = (shot.image.width(), shot.image.height());
    let widest = (0..height)
        .step_by(4)
        .map(|y| {
            let mut run = 0;
            let mut widest = 0;
            for x in 0..width {
                run = if shot.image.get_pixel(x, y).0[..3].iter().all(|&c| c >= 246) {
                    run + 1
                } else {
                    0
                };
                widest = widest.max(run);
            }
            widest
        })
        .max()
        .unwrap_or(0);
    widest as f32 / shot.scale
}

#[test]
fn a_drafts_composer_keeps_its_width_and_widens_as_it_docks() {
    let mut scene = Scene::conversation("composer-width", Settings::default(), &["Reply."]);
    let draft = composer_width(&scene.shot("draft"));
    assert!(
        (700. ..=720.).contains(&draft),
        "the draft's box is {draft} wide"
    );
    scene.send("first");
    scene.wait(30);
    let gliding = composer_width(&scene.shot("gliding"));
    scene.wait_for_agent();
    scene.wait(800);
    let docked = composer_width(&scene.shot("docked"));
    assert!(
        (860. ..=880.).contains(&docked),
        "the docked box is {docked} wide"
    );
    assert!(
        draft <= gliding && gliding <= docked,
        "it widens on the way: {gliding}"
    );
}

/// A reply with a link, inline code, a file, and a table whose last column
/// wraps, in Uzbek Latin and Cyrillic.
const RICH_REPLY: &str = "Read [the guide](https://example.com/guide) before `cargo test`, then open `src/main.rs:12`.

| Mahsulot | Kimniki | Oʻzbekistonda borligining dalili |
| --- | --- | --- |
| **LANGAME** | Rossiya | Asosiy raqobatchi. `langame.ru/uzbekistan` katalogida Oʻzbekistondagi 373 ta klub bor. Uzbek klublar uchun alohida dastur sahifasi bor, onlayn toʻlov ATMOS.uz orqali ishlaydi. |
| iCafeCloud | xalqaro | Ҳужжатларида клубни Payme'га улаш йўриқномаси бор. |
| SmartShell | Rossiya | Dalil topilmadi. |";

/// Where the pixels near `color` are: their left, top, right and bottom,
/// in pixels of the shot, and how many there are.
fn pixels_near(shot: &crate::scene::Shot, color: [u8; 3]) -> (u32, u32, u32, u32, usize) {
    let near =
        |pixel: [u8; 4]| (0..3).all(|at| (i32::from(pixel[at]) - i32::from(color[at])).abs() < 16);
    let (mut left, mut top, mut right, mut bottom, mut count) = (u32::MAX, u32::MAX, 0, 0, 0);
    for (x, y, pixel) in shot.image.enumerate_pixels() {
        // Only the conversation: right of the sidebar, above the composer.
        if x < 500 || y > 1500 || !near(pixel.0) {
            continue;
        }
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
        count += 1;
    }
    (left, top, right, bottom, count)
}

fn link_underlines_under_the_pointer(
    name: &str,
    settings: Settings,
    link: [u8; 3],
    mention: [u8; 3],
) {
    let mut scene = Scene::conversation(name, settings, &[RICH_REPLY]);
    scene.send("Compare the competitors in a table");
    scene.wait_for_agent();
    let rest = scene.shot("rest");
    let (left, top, right, bottom, count) = pixels_near(&rest, link);
    assert!(count > 0, "the link shows in its color");
    let (x, y) = (
        (left + right) as f32 / 2. / rest.scale,
        (top + bottom) as f32 / 2. / rest.scale,
    );
    scene.hover(x, y);
    let hovered = scene.shot("link-hovered");
    let (.., under) = pixels_near(&hovered, link);
    let line = ((right - left) as f32 * 0.8) as usize;
    assert!(
        under >= count + line,
        "a line as wide as the link joins it: {count} then {under}"
    );
    scene.hover(x, y + 200.);
    let (.., away) = pixels_near(&scene.shot("link-left"), link);
    assert_eq!(away, count, "the line goes with the pointer");
    let (left, top, right, bottom, icon) = pixels_near(&rest, mention);
    assert!(icon > 0, "the file shows as a chip, its icon in its color");
    // Over the chip's name, right of its icon.
    scene.hover(
        (left as f32 / rest.scale) + 30.,
        (top + bottom) as f32 / 2. / rest.scale,
    );
    let (.., edged) = pixels_near(&scene.shot("chip-hovered"), mention);
    assert!(
        edged > icon + (right - left) as usize,
        "the chip's hairline takes its color: {icon} then {edged}"
    );
}

#[test]
fn a_link_underlines_under_the_pointer_in_light() {
    link_underlines_under_the_pointer(
        "rich-light",
        Settings::default(),
        [0x04, 0x65, 0xaf],
        [0x09, 0x76, 0x89],
    );
}

#[test]
fn a_link_underlines_under_the_pointer_in_dark() {
    let dark = Settings {
        dark: true,
        ..Settings::default()
    };
    link_underlines_under_the_pointer("rich-dark", dark, [0x80, 0xbd, 0xfb], [0x69, 0xce, 0xe6]);
}

/// Tables as an agent may write them: a bold name beside a long cell with
/// code, a commit log with numbers, a link too long to break, missing and
/// empty cells, one column, more columns than fit, and set alignments.
const TABLES: &str = "| Mahsulot | Kimniki | Oʻzbekistonda borligining dalili |
| --- | --- | --- |
| **LANGAME** | Rossiya | Asosiy raqobatchi. `langame.ru/uzbekistan` katalogida Oʻzbekistondagi 373 ta klub bor. Onlayn toʻlov ATMOS.uz orqali ishlaydi, bu bizning `club-verification-payments` tadqiqotimizda ham yozilgan. |
| **iCafeCloud** | xalqaro | Hujjatlarida klubni **Payme**ga ulash yoʻriqnomasi bor. |
| **Startup Base (IT Park)** | Oʻzbekiston | Ҳужжатларида клубни Payme'га улаш йўриқномаси бор. |

| # | Commit | Xabar | Nima qiladi | Fayllar | Oʻzgarish |
| --- | --- | --- | --- | --- | --- |
| 1 | `1662191` | `feat(studio): draw the app in scenes, on no screen` | Ilovani ekransiz, haqiqiy Metal bilan chizadigan ssena harness'i va 5 ta ssena. | 10 | +672 / −44 |
| 2 | `950311d` | `feat(dev): keep the app running when a change only reaches tests` | Ilova binari oʻzgarmasa, live-reload oynani qayta ochmaydi. | 2 | +25 / −5 |
| 3 | `58d833a` | `docs: prove a change in a scene first` | AGENTS.md: avval ssena. | 1 | +43 / −8 |

| Manba | Havola | Izoh |
| --- | --- | --- |
| Fastbase | https://fastbase.uz/companies/computer-clubs-in-uzbekistan-tashkent-samarkand-bukhara-list | |
| OLX | | Eʼlonlar |
| Gizmo |

| Faqat bitta ustun |
| --- |
| Birinchi qator |
| Ikkinchi, ancha uzunroq qator, bitta ustunli jadvalda qanday turishini koʻrish uchun |

| A | B | C | D | E | F | G | H | I | J |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Toshkent shahri | Samarqand viloyati | Buxoro viloyati | Andijon viloyati | Fargʻona viloyati | Namangan viloyati | Xorazm viloyati | Navoiy viloyati | Qashqadaryo viloyati | Surxondaryo viloyati |

| Klub | Kompyuterlar | Narx (soʻm/soat) |
| :--- | :---: | ---: |
| Cyber Arena | 40 | 12 000 |
| GG Club | 120 | 9 500 |";

fn tables_hold_whatever_comes(name: &str, settings: Settings) {
    let tables: Vec<&str> = TABLES.split("\n\n").collect();
    for (label, size) in [("wide", (1200., 800.)), ("narrow", (720., 480.))] {
        let name = format!("{name}-{label}");
        let mut scene = Scene::conversation_at(&name, settings, size, &tables);
        for at in 0..tables.len() {
            scene.send("Jadval");
            scene.wait_for_agent();
            scene.shot(&at.to_string());
        }
    }
}

#[test]
fn tables_hold_whatever_comes_in_light() {
    tables_hold_whatever_comes("tables-light", Settings::default());
}

#[test]
fn tables_hold_whatever_comes_in_dark() {
    let dark = Settings {
        dark: true,
        ..Settings::default()
    };
    tables_hold_whatever_comes("tables-dark", dark);
}

#[test]
fn under_reduce_motion_the_composer_docks_at_its_width_at_once() {
    let still = Settings {
        reduce_motion: true,
        ..Settings::default()
    };
    let mut scene = Scene::conversation("composer-width-still", still, &["Reply."]);
    scene.send("first");
    scene.wait(30);
    let docked = composer_width(&scene.shot("docked"));
    assert!(
        (860. ..=880.).contains(&docked),
        "the docked box is {docked} wide"
    );
}

/// A reply with every kind of block, to read at the conversation's size.
const READING: &str = "## Natija

Bu paragraf bir necha qatorga oʻralishi uchun ataylab uzun yozilgan: agentning javobi ekranda qanday oʻqilishini, qatorlar orasidagi masofa va harflar oʻlchami koʻzni charchatmasligini tekshirish uchun. Ikkinchi gap ham shu maqsadda, matn ustunning eniga yetib, yana pastga tushsin.

### Keyingi qadamlar

1. Birinchi qadam `cargo test` bilan
2. Ikkinchi qadam
   - ichki band
- [x] Bajarilgan ish
- [ ] Qolgan ish

> Iqtibos: muhim eslatma.

```rust
fn main() {
    println!(\"salom\");
}
```";

/// The tops of the runs of rows with ink in them between `x` and `to`,
/// from `from` down to `until`, in points: the lines of text there.
fn line_tops(
    shot: &crate::scene::Shot,
    (x, to): (f32, f32),
    (from, until): (f32, f32),
) -> Vec<f32> {
    let card = shot.at(x, from);
    let inked = |y: f32| {
        (x as u32..to as u32)
            .any(|px| (luminance(shot.at(px as f32, y)) - luminance(card)).abs() > 0.3)
    };
    let mut tops = Vec::new();
    let mut was = false;
    let mut y = from;
    while y < until {
        let now = inked(y);
        if now && !was {
            tops.push(y);
        }
        was = now;
        y += 0.5;
    }
    tops
}

fn a_reply_reads_at_its_size(name: &str, settings: Settings) {
    let mut scene = Scene::conversation(name, settings, &[READING]);
    scene.send("Natijani yoz");
    scene.wait_for_agent();
    let shot = scene.shot("reply");
    // The paragraph's wrapped lines, left of where its shortest line ends.
    let tops = line_tops(&shot, (295., 400.), (170., 240.));
    let pitches: Vec<f32> = tops.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(pitches.len() >= 2, "the paragraph wraps: {tops:?}");
    for pitch in pitches {
        assert!(
            (22. ..=23.).contains(&pitch),
            "lines stand 22.4 apart: {tops:?}"
        );
    }
}

#[test]
fn a_reply_reads_at_its_size_in_light() {
    a_reply_reads_at_its_size("reading-light", Settings::default());
}

#[test]
fn a_reply_reads_at_its_size_in_dark() {
    let dark = Settings {
        dark: true,
        ..Settings::default()
    };
    a_reply_reads_at_its_size("reading-dark", dark);
}
