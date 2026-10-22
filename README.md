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
window's place without the window going away: same place and size, same sidebar, in front
only if the old one was. A build that fails leaves the running app as it is. Ctrl-C stops
the runner and the app quits with it, its changes saved.

## Layout

- `crates/studio`: the app binary: the main window, its shell (the top row with its tabs,
  the sidebar and the main area's card) and the Add project palette.
- `crates/theme`: the look every crate shares: materials, colors and sizes. The rules are in
  [docs/design.md](docs/design.md).
- `crates/workspace`: projects, sessions and the tabs open on them, with no UI: adding
  folders, browsing them by a typed path, search, sort and filter, opening and closing
  tabs, and Back and Forward. All but Back and Forward is saved as JSON in the platform's
  data folder (`Nerda Studio/workspace.json`).
- `crates/dev`: the live-reload runner; the app's side is `crates/studio/src/dev_reload`.

GPUI is pinned to one commit of its repository in the root `Cargo.toml`: crates.io has
only an older `gpui` without `gpui_platform`. `vendor/block` is the `block` crate, which
GPUI's macOS backend pulls in, patched so that a future Rust still compiles it; the root
`Cargo.toml` puts it in place of the crates.io copy.

