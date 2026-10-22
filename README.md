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

## Layout

- `crates/studio`: the app binary: the main window, its shell, the sidebar and the Add
  project palette.
- `crates/theme`: the look every crate shares: materials, colors and sizes. The rules are in
  [docs/design.md](docs/design.md).
- `crates/workspace`: projects and sessions, with no UI: adding folders, browsing them by a
  typed path, search, sort and filter, saved as JSON in the platform's data folder
  (`Nerda Studio/workspace.json`).

GPUI is pinned to one commit of its repository in the root `Cargo.toml`: crates.io has
only an older `gpui` without `gpui_platform`.

