# Nerda Studio

Rust on GPUI; `README.md` has the layout and how to run it. Before changing anything on
screen, read [docs/design.md](docs/design.md): materials, colors, motion and window chrome
follow its rules.

Code, comments and docs never name another product or project as a source or model
("after X's", "like X", "ported from X"): describe the behavior itself. License texts

## Done means proven

The owner wants every change to work, edge cases included. A change is done when each of
its edge cases has an observed result on the running app (`cargo run -p dev -- try`,
below), or a named reason it could not be run.

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

### Parallel sessions

Several sessions work on the project at once. Each works in its own git worktree on its
own branch, never in the owner's checkout (the first path `git worktree list` prints):
the owner's live-reload runner builds whatever that checkout holds, half-done edits
included.
owner's, so the first build takes seconds; a worktree without `target/` gets one with
`cp -c -R <owner's checkout>/target target`. Commit on the branch; it reaches `main` when
the owner asks for it. A session that does run in the owner's checkout keeps every save
building, since the runner shows each one. Live reload (`cargo run -p dev`) refuses to
run in a worktree: its builds would take the owner's window over.

### Launching the app

Launch with `cargo run -p dev -- try`, never `cargo run -p studio`:

1. Start it as a background task: `cargo run -p dev -- try --for 600`.
2. Read `window <number>` from its output; `screencapture -x -o -l <number> shot.png`
   captures the window while it stays behind the others.
3. When done, kill the `try` process; the app saves and quits with it. A forgotten one
   quits on its own after `--for` seconds (900 by default).

What it gives the app:

- A data folder of its own under `target/try/`, removed afterwards: it never takes over
  the owner's window or reads and saves the owner's data. `--data <folder>` starts from
  a prepared folder (a missing, corrupt or newer `workspace.json`) and keeps it.
- The background: no Dock icon, no ⌘Tab entry, no menu bar, never activated, its window
  behind every other and inactive. The owner keeps the screen and the keyboard.
- Arguments after `--` go to the app.

`--front` runs it as a regular app in front, for what only that shows: the active
window's look, the menu bar, an on-screen region (`screencapture -R`), contrast over a
window put behind it. It takes the owner's focus, so keep that run short. Taking the
owner's window over is the one case for `cargo run -p studio`, and only when the change
is that takeover.

## Proving without a real click

The agent's terminal has no Accessibility permission, so macOS drops synthetic OS events.

- Clicks and keys: a temporary probe calls `window.dispatch_event(PlatformInput::…)` or
  `window.dispatch_action(…)` from `cx.spawn_in`, outside any entity update; dispatching
  inside one panics on the entity lease. These reach GPUI, not AppKit.
- Motion: log the animated value per frame with timestamps; slow a spring tenfold to
  screenshot its middle.
- Pixels: `screencapture -x -o -l <window>` for a background build, sampled in sRGB.
  For contrast, run with `--front` and put a solid borderless window behind the app.
- Light appearance without touching the system:
  `cargo run -p dev -- try -- -NSRequiresAquaSystemAppearance YES`.
- Windows code: `cargo check --target x86_64-pc-windows-msvc` on the changed file in a
  scratch crate; GPUI's Windows backend builds only on Windows.
