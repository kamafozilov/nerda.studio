# Nerda Studio

One desktop app for my work: coding agents first, then a design canvas, notes and more.
Built in Rust on [GPUI](https://gpui.rs), a GPU-accelerated UI framework.

## Run

Needs stable Rust and Xcode with its Metal Toolchain: GPUI renders with Metal on macOS and
compiles its shaders at build time. Xcode ships the toolchain as a separate download:

```sh
xcodebuild -downloadComponent MetalToolchain
```

Then:

```sh
cargo run -p studio
```

While working on the app, run it with live reload instead:

```sh
cargo run -p dev
```

Each change to `crates/` or the manifests rebuilds it, and the new build takes the old
window's place without the window going away: same display, place and size, same sidebar,
in front only if the old one was, otherwise just where the old one sat among the other
windows. A build that fails leaves the running app as it is. Ctrl-C stops the runner and
the app quits with it, its changes saved. The app keeps where its window was when it quit
and opens there again; when the runner brings the app back on its own, it opens behind
the other windows. Live reload runs only in the main checkout, not in a linked worktree.

To try a build without disturbing the one you work in, as an agent does,
`cargo run -p dev -- try` runs it with its own data, in the background and under the
desktop picture, where no screen shows it, until the command ends; `--front` brings it forward. `AGENTS.md` has the details.

On macOS every build run from the source tree runs as **Nerda Studio Dev**
(`studio.nerda.dev`): it puts itself in `target/<profile>/Nerda Studio Dev.app` and runs
from there, so it has its own name in the menu bar, Dock icon and ⌘Tab entry. To install
the app itself, **Nerda Studio** (`studio.nerda`), in `/Applications` and open it:

```sh
cargo run -p dev -- install
```

It is signed ad hoc, so macOS asks again for any permission after each install; set
`NERDA_SIGN_IDENTITY` to the name of a code-signing certificate in your keychain and the
permissions stay.

Only one instance runs at a time, the newest: launching the app while it runs (another
build, the installed app, an agent's build) takes the window over in place, and the old
instance saves and quits. Opening the installed app while it already runs brings it to
the front.

## Layout

- `crates/studio`: the app binary: the main window, its shell (the top row with its tabs,
  the sidebar and the main area's card), a session's composer, its agent and its
  conversation ([docs/session.md](docs/session.md)), and the Add project palette.
- `crates/theme`: the look every crate shares: materials, colors and sizes. The rules are in
  [docs/design.md](docs/design.md).
- `crates/workspace`: projects (folders, or the computer itself), sessions, the agents a session can run and its choice
  among them, and the sessions and pages (Notes, Design, Routines) open as tabs, with no
  UI: adding folders, browsing them by a typed path,
  search, sort and filter, opening and closing tabs, and Back and Forward. All but Back
  and Forward is saved as JSON in the platform's data folder
  (`Nerda Studio/workspace.json`), beside `instance.lock`, held by the instance that
  saves.
- `crates/agent`: a session's coding agent run as a process, with no UI: what it says
  parsed into typed events and reduced into a transcript of turns, and the session's
  status. [docs/agent.md](docs/agent.md) has the lifecycle and the model, and
  [docs/agent-prompts.md](docs/agent-prompts.md) the prompts and their answers.
- `crates/dev`: the live-reload runner and `install`; one instance taking over from
  another is `crates/studio/src/instance`.
- `crates/app_bundle`: the macOS app bundle, its `Info.plist` and its icon, made from
  `assets/AppIcon.svg`.

GPUI is pinned to one commit of its repository in the root `Cargo.toml`: crates.io has
only an older `gpui` without `gpui_platform`. `vendor/block` is the `block` crate, which
GPUI's macOS backend pulls in, patched so that a future Rust still compiles it; the root
`Cargo.toml` puts it in place of the crates.io copy.
