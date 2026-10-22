# Nerda Studio

Rust on GPUI; `README.md` has the layout and how to run it. Before changing anything on
screen, read [docs/design.md](docs/design.md): materials, colors, motion and window chrome
follow its rules.

Code, comments and docs never name another product or project as a source or model
("after X's", "like X", "ported from X"): describe the behavior itself. License texts

## Done means proven

The owner wants every change to work, edge cases included. A change is done when each of
its edge cases has an observed result on the running app (`cargo run -p studio`), or a
named reason it could not be run.

1. Before writing code, list the change's edge cases: its own, plus every row below that
   applies.
2. Exercise each one on the running app. Measure where a number decides it (pixels,
   timings, contrast); a screenshot proves layout, a trace proves motion.
3. Remove every probe (`SMOKE-PROBE`, then grep for it), then run
   `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check`.
4. Report each case with its result. Name what stayed unverified and why, and give the
   owner exact steps for anything only a real click can show.

### Edge cases to cover

| Area | Cases |
| --- | --- |
| Repeat | Double-click, rapid repeats, a second trigger mid-animation |
| Input | Pointer click, keyboard shortcut, keyboard-activated click |
| Top row | A control there keeps its clicks (`.occlude()`, drawn after the titlebar strip); empty space drags and double-click zooms |
| Appearance | Dark and light; glass and solid (Reduce transparency); Reduce motion |
| Glass | Text contrast over a white and a black desktop |
| Window | Sidebar open and closed, minimum size, zoomed and full screen, inactive |
| Platform | Windows-only code type-checks for `x86_64-pc-windows-msvc` |
| Text | Cyrillic and Uzbek Latin (`oʻ`), whitespace only, very long, paste with newlines |
| Names | Very long names truncate; two folders with the same name stay distinguishable |
| Lists | Empty, one, many enough to scroll; filtered to nothing |
| Saved data | Missing, corrupt (set aside, never overwritten), from a newer version (left untouched), unwritable |

### Launching the app

One instance runs at a time: a build you launch takes the owner's window over in place
(the owner's live-reload runner, or the installed app), with its data. When you are done,
quit what you launched (`kill` its pid); the runner then opens its own build again. To
keep the owner's data out of a test, launch with `HOME` set to a scratch folder: the
instance there is separate, with its own data, lock and window.

## Proving without a real click

The agent's terminal has no Accessibility permission, so macOS drops synthetic OS events.

- Clicks and keys: a temporary probe calls `window.dispatch_event(PlatformInput::…)` or
  `window.dispatch_action(…)` from `cx.spawn_in`, outside any entity update; dispatching
  inside one panics on the entity lease. These reach GPUI, not AppKit.
- Motion: log the animated value per frame with timestamps; slow a spring tenfold to
  screenshot its middle.
- Pixels: `screencapture -x -R x,y,w,h`, sampled in sRGB. For contrast, put a solid
  borderless window behind the app.
- Light appearance without touching the system: launch with
  `-NSRequiresAquaSystemAppearance YES`.
- Windows code: `cargo check --target x86_64-pc-windows-msvc` on the changed file in a
  scratch crate; GPUI's Windows backend builds only on Windows.
