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

Several sessions work on the project at once. A session works where the owner started
it, and never makes a worktree or a branch to move itself elsewhere:

- **Started in the owner's checkout** (the first path `git worktree list` prints): the
  work stays there, on its branch. The owner's live-reload runner builds every save, so
  each save keeps the app building, and work is never split by swapping files in it.
- **Started in a worktree** (one the owner made for the session): the work stays there. Its
  `target/` starts as an APFS clone of the owner's; one without gets
  `cp -c -R <owner's checkout>/target target`. It reaches `main` when the owner asks.
  Live reload (`cargo run -p dev`) refuses to run in a worktree: its builds would take
  the owner's window over.

Several `try` builds can run at once, from one session or many: each has its own data
folder and window.

### Launching the app

The owner works at this Mac while agents do: nothing an agent runs may take their
window, keyboard or mouse. Launch with `cargo run -p dev -- try`, which stays in the
background; `cargo run -p studio` and `try --front` refuse to start unless a person
types them at a terminal.

1. Start it as a background task: `cargo run -p dev -- try --for 600`.
2. Read `window <number>` from its output; `screencapture -x -o -l <number> shot.png`
   captures the window where it is, under the desktop picture.
3. When done, kill the `try` process; the app saves and quits with it. A forgotten one
   quits on its own after `--for` seconds (900 by default).

What it gives the app:

- A data folder of its own under `target/try/`, removed afterwards: it never takes over
  the owner's window or reads and saves the owner's data. `--data <folder>` starts from
  a prepared folder (a missing, corrupt or newer `workspace.json`) and keeps it.
- The background: no Dock icon, no ⌘Tab entry, no menu bar, never activated, its window
  inactive and under the desktop picture, on no screen. The owner keeps the screens and
  the keyboard. The app draws its own frames there at 60 Hz, so motion runs, a plain
  `screencapture` shows the current frame, and CPU can be measured.
- `--active` draws the window as the active one (the cursor shows and blinks);
  `--desktop <rrggbb>` gives its glass that desktop, such as `ffffff` or `000000`, for
  contrast over the worst one. Measured: the light sidebar over `000000` reads 196,
  198, 201, as on a real black desktop.
- Nothing opens outside the window: a system picker or the Finder says on stdout that it
  doesn't open from a background build.
- Arguments after `--` go to the app.

What the background can't show is the macOS menu bar and the system's own windows (a
picker, the Finder): check a menu command by dispatching its action from a probe, and
leave a picker's look to the owner, with exact steps. Never use computer use or
synthetic OS events: they drive the owner's own mouse and keyboard.

## Proving without a real click

The agent's terminal has no Accessibility permission, so macOS drops synthetic OS events.

- Clicks and keys: a temporary probe calls `window.dispatch_event(PlatformInput::…)` or
  `window.dispatch_action(…)` from `cx.spawn_in`, outside any entity update; dispatching
  inside one panics on the entity lease. These reach GPUI, not AppKit.
- Motion: log the animated value per frame with timestamps; slow a spring tenfold to
  screenshot its middle.
- Pixels: `screencapture -x -o -l <window>` for a background build, sampled in sRGB.
  Give a change a frame or two (50 ms) before capturing it. For contrast on glass, run
  with `--desktop ffffff` and `--desktop 000000`.
- Light appearance without touching the system:
  `cargo run -p dev -- try -- -NSRequiresAquaSystemAppearance YES`.
- Agents: `NERDA_AGENT_PROGRAM=<repo>/target/debug/fake-agent` (`cargo build -p agent
  --bin fake-agent`) runs the fake agent in place of claude, playing `fake-agent.jsonl`
  from the project folder (`crates/agent/tests/support/fake_agent.rs` has the format).
- Windows code: `cargo check --target x86_64-pc-windows-msvc` on the changed file in a
  scratch crate; GPUI's Windows backend builds only on Windows.
