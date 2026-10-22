# macOS app identity and a single instance

Research for: the app runs unbundled (no bundle ID, Dock name, icon or own TCC identity),
and several launches write the same `workspace.json`. Checked on macOS 27.0.1 (26A434),
rustc 1.99.1, GPUI at `50304070c283519f2f2338b45671af26374025fe`. **Observed** marks
results run on this Mac (scratch under `/tmp`, `HOME` pointed at a scratch folder so no
test instance touched the real data); everything else is read from the cited source.

## Recommendations

1. **Identity comes only from a bundle path.** Exec'ing `X.app/Contents/MacOS/<exe>`
   directly gives the full identity (bundle ID, menu-bar name, Dock icon), with stdin,
   stdout, env and pid intact. `app_id` and `set_app_identity` do nothing on macOS, and an
   Info.plist embedded in a bare binary does not reach the Dock.
2. **Dev: the debug binary bundles itself and re-execs.** First thing in `main`
   (`cfg(all(target_os = "macos", debug_assertions))`): if `NSBundle.mainBundle` has no
   identifier, refresh `target/debug/Nerda Studio Dev.app` next to the binary (APFS clone
   of `current_exe()` to a temp name, then `rename`; Info.plist; checked-in `.icns`;
   `codesign --force -s -`), then `execv` the bundled executable with the same argv and env.
   This covers `cargo run -p studio`, the dev runner and agents running `target/debug/studio`,
   leaves `cargo test` and Windows alone, and costs about 150 ms only when the binary
   changed. Do not use a `.cargo/config.toml` runner; it also wraps every `cargo test`
   binary and misses direct launches.
3. **Use a separate dev bundle ID** (`studio.nerda.dev`, "Nerda Studio Dev") so LaunchServices,
   Dock pins and TCC never confuse the dev build with `/Applications`. Both share the data
   folder, so they still take over from each other. *Owner decision.*
4. **Install**: build release, assemble `Nerda Studio.app` (`studio.nerda`), sign, copy
   to `/Applications` under a temp name and swap it in, then `open -n` it. A plain `open`
   just brings the still-running old process forward. Do not set
   `LSMultipleInstancesProhibited`: it makes `open -n` reuse the old process.
5. **TCC**: an ad-hoc signature's designated requirement is its cdhash, so every new build
   is a stranger to TCC. Keep grants across updates by signing the installed app with
   one stable self-signed certificate (identifier plus certificate-leaf requirement).
   A directly exec'd dev build is charged to the launching terminal anyway (section D).
6. **Single instance lives in the app, on every platform**: `instance.lock` held with
   `File::try_lock` (std ≥ 1.89) is the right to write `workspace.json`, and
   `instance.sock` (Unix socket; a named pipe on Windows) carries the handoff. Use the
   dev runner's protocol words: `handoff` → `handed-off <screen>` → `ready`, or the old
   instance resumes (section F).
7. **Dev runner shrinks** to building and launching. The app-to-app takeover replaces its
   `handoff`/`resume` and `screen.txt`. It keeps the child's stdin only as "runner alive":
   closed stdin quits, so Ctrl-C still saves and quits (section H).

## A. Info.plist keys

From Apple's Information Property List reference
(<https://developer.apple.com/documentation/bundleresources/information-property-list>, one
page per key):

| Key | Need | Note |
| --- | --- | --- |
| `CFBundleIdentifier` | required | "uniquely identifies a single app throughout the system"; LaunchServices and code signing use it |
| `CFBundleExecutable` | required | Name of the file in `Contents/MacOS`; "macOS uses this key to locate the bundle's executable" |
| `CFBundlePackageType` | set `APPL` | Defaults from the extension, else `BNDL` |
| `CFBundleName` | set | Short name, at most 15 characters; **observed** to be the bold menu-bar name |
| `CFBundleDisplayName` | optional | Used when longer than `CFBundleName` |
| `CFBundleIconFile` | set | `.icns` in `Contents/Resources` (extension optional) |
| `CFBundleIconName` | optional | Needs a compiled asset catalog (`Assets.car`, from `actool`) |
| `CFBundleShortVersionString` / `CFBundleVersion` | set | `0.1.0` (workspace version) and a build number |
| `CFBundleInfoDictionaryVersion` | set `6.0` | |
| `LSMinimumSystemVersion` | set | The lowest macOS GPUI supports |
| `NSHighResolutionCapable` | set `true` | **Observed:** missing or `true` gives backing scale 2.0; `false` gives 1.0 |
| `NSPrincipalClass` | omit | GPUI creates its own `NSApplication` subclass; Xcode's default value is `NSApplication` |
| `LSApplicationCategoryType` | optional | App Store category, e.g. `public.app-category.developer-tools` |

**Observed:** a bundle with only the "set" rows above launched and showed the full
identity (section B). `plutil -lint` checks the file.

## B. Exec'ing the executable inside a bundle (observed)

Baseline, the bare binary (`/tmp/nerda-id/studio-bin`, a copy of `target/debug/studio`):

```
$ lsappinfo info $(lsappinfo find pid=$P)
"studio-bin" ASN:...: (in front)   bundleID=[ NULL ]   bundle path="/private/tmp/nerda-id/studio-bin"
```

Menu bar shows **studio-bin**. Then, in `/tmp/nerda-id/Nerda Test.app` (ID
`studio.nerda.test`, name "Nerda Test", executable `nerdaexe`), started directly from a
Python driver with pipes and the dev env (`NERDA_DEV_STATE`):

| Executable in `Contents/MacOS` | lsappinfo bundleID | Running app (`NSRunningApplication`) | Menu bar | Dev protocol over pipes |
| --- | --- | --- | --- | --- |
| clone copy (`cp -c`) | `studio.nerda.test` | name "Nerda Test", our icon, policy Regular, active | **Nerda Test** | `ready`, `handed-off`, exit 0 on closed stdin |
| hard link | same | same | same | same |
| symlink to the binary | same | same; via `open` the process path resolves to the target and lsappinfo keeps `originalExecutablePath` | same | same |

- The kernel sees the bundle path, and CFBundle finds `Contents/` from it. **No
  `lsregister` is needed:** the first direct launch also put the bundle into the
  LaunchServices database (`lsregister -dump` lists `Cert1.app`, which was never `open`ed).
- `open -b studio.nerda.test` failed for every bundle under `/tmp`
  (`LSCopyApplicationURLsForBundleIdentifier() failed`), even after `lsregister -f`. Not
  tested under `target/` or `/Applications`. GPUI's URL-scheme registration uses this
  lookup (`crates/gpui_macos/src/platform.rs:820-825`).
- Each kind of link has a cost. `cargo build` gives `target/debug/studio` a **new inode**
  each time (link count 1, not a hard link to `deps/`), so a hard link keeps the old build.
  A symlink always follows the new build but `codesign` refuses it ("the main executable
  or Info.plist must be a regular file"). A clone copy is 3 ms for the 54 MB binary.
- Replace the executable with a temp file and `rename`, never by overwriting it. macOS
  caches signature info per file in the kernel, and editing a file in place can crash its
  next launch with "Code Signature Invalid" ("Updating Mac Software",
  <https://developer.apple.com/documentation/security/updating-mac-software>).
- An Info.plist embedded in a bare binary (`-sectcreate __TEXT __info_plist`) **does not
  work**: the process sees the ID, lsappinfo shows `bundleID=[ NULL ]`.

## C. `open` and a running instance (observed)

- A directly exec'd instance runs, then `open "Nerda Test.app"`: **no new process**.
  `open` printed "was already running and so the additional environment variables could
  not be set". A probe AppKit app logged `applicationShouldHandleReopen hasVisibleWindows=false`
  in the directly exec'd process. LaunchServices matches running apps by bundle, not by
  how they started.
- A second **direct exec** always starts a second process: LaunchServices never sees it
  coming, so the app's own lock has to stop it.
- `open -n` without `LSMultipleInstancesProhibited` started a new process. **With the key,
  `open -n` reused the running one** (it got a reopen), and two direct execs still both ran.
  Apple documents the key as "whether more than one user can launch the app simultaneously",
  meaning across user sessions.
- Right after a `kill`, `open` still said "already running" and launched nothing for a
  few seconds: another reason for the install to use `open -n`.

## D. Code signing and TCC

**Observed:** the arm64 linker signature (`flags=0x20002(adhoc,linker-signed)`,
`Identifier=studio-5bc26e17aa182160`, "Info.plist=not bound") is enough to launch the
bundle, directly or with `open`, with the Dock and menu identity. Local builds carry no
quarantine (only `com.apple.provenance`), so Gatekeeper never checks them.
`codesign --force -s - X.app` (150 ms) binds the Info.plist and sets
`Identifier=studio.nerda.test`: tidier, not required.

**Designated requirements** (TN3127,
<https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements>):
TCC "record[s] your app's DR" and checks each later version against it. "Ad hoc signed
code ... has a DR but it's tied to that specific version of the code ... macOS can't
reliably track the identity of the code." **Observed:**

```
ad-hoc:        designated => cdhash H"9926f485..."      (changes with every build)
self-signed:   designated => identifier "studio.nerda.test" and certificate leaf = H"015dbaff..."
$ codesign -v -R='<the self-signed DR>' Cert2.app   # a different binary, same cert
Cert2 satisfies Cert1's DR
```

So every ad-hoc rebuild asks for Accessibility and Files and Folders again. One
self-signed code-signing certificate, created once in the login keychain, keeps them. Tested
with a throwaway keychain; it was listed as `CSSMERR_TP_NOT_TRUSTED` and **still signed and
launched**. Not verified: an actual TCC prompt surviving a rebuild, because granting needs a
human click.

**Who TCC charges (observed, private API):**
`responsibility_get_pid_responsible_for_pid` returned the agent host app's pid for a bundle
**exec'd directly** from its shell, and the app's **own pid when launched with `open`**. A
directly exec'd bundle therefore still borrows the terminal's permissions, which is fine for
dev. Only a LaunchServices launch (`open`, Finder, Dock) gets prompts in its own name.
`open -n --stdin FIFO --stdout FIFO --env ...` kept the whole dev protocol (`ready`,
`handed-off`, quit on EOF), but the process is then launchd's child, not the runner's.

## E. Icon

- `.icns` from an `.iconset` with `iconutil -c icns AppIcon.iconset`. File names:
  `icon_{16,32,128,256,512}x{same}.png` and `@2x` of each, 10 files
  (<https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/Optimizing/Optimizing.html>).
  `sips -z N N in.png --out ...` makes each size. **Observed:** a 92 KB `.icns`, shown as
  the running app's icon.
- An asset catalog (`CFBundleIconName`) needs `actool`, which Xcode has (Xcode is already
  required for Metal). Not worth it for a placeholder.
- SVG to PNG on this Mac: there is no `rsvg-convert` or ImageMagick. `qlmanage -t -s 1024`
  renders SVG but **flattens it onto opaque white** (corner pixel `255,255,255,255`). A
  10-line Swift script (`NSImage(contentsOfFile:)` drawn into an `NSBitmapImageRep`)
  keeps alpha (corner `0,0,0,0`), but ignored `font-weight`, so draw the placeholder with
  paths, not text.
- Simplest: commit `AppIcon.icns` (about 90 KB) and its 1024 px master, so no build needs
  a tool. **Observed:** macOS 27 adds its own glass edge to a legacy `.icns`.

## F. Single instance and handoff

**`std::fs::File` locks** (<https://doc.rust-lang.org/std/fs/struct.File.html>): `lock`,
`try_lock`, `unlock` and the shared variants are stable since **1.89.0**. On Unix they are
`flock` (`LOCK_EX|LOCK_NB` for `try_lock`); on Windows, `LockFileEx` with
`LOCKFILE_FAIL_IMMEDIATELY`. A lock is released when the file and every handle duplicated
from it close. `try_lock` returns `Err(TryLockError::WouldBlock)`. "May be advisory or
mandatory": on Windows it is mandatory and blocks other processes' reads and writes of the
range, and the OS frees a dead process's lock after a delay that "depends upon available
system resources" (<https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex>).
So lock a separate `instance.lock`, never `workspace.json`. **Observed:**

```
holder running        -> try_lock: WouldBlock in 2.5µs
after kill -9 holder  -> try_lock: acquired
2nd handle, same proc -> WouldBlock   (flock is per open file, not per process)
holder spawned `sleep`, then kill -9 -> child has neither fd; lock free (std uses CLOEXEC)
```

A pid file can't tell a live owner from a recycled pid; the lock needs no cleanup.

**IPC.** `std::os::unix::net::UnixListener` works on macOS and Linux. **Observed:**
`bind` takes paths up to **103 bytes** (104 gives "path must be shorter than SUN_LEN").
`~/Library/Application Support/Nerda Studio/instance.sock` is **73 bytes** for this user,
which leaves about 41 bytes for a user name. Fall back to `$TMPDIR` (49 bytes, per user)
plus a hash of the data folder when the path is too long. A crashed listener leaves its
file: `connect` → `ConnectionRefused`, and `bind` → `AddrInUse`, so only the lock holder
binds, after `remove_file`. Windows: std has no Unix sockets. Use a named pipe
`\\.\pipe\nerda-studio-<user SID or hash>` through `windows-sys` (`Win32_System_Pipes`,
`CreateNamedPipeW`). The lock code stays shared.

**Protocol.** `O` is the running owner and `N` the new launch. Lines are the dev runner's
words.

1. `N` opens `instance.lock` and calls `try_lock`. On `Ok`, `N` owns the data: it removes
   any stale socket, binds, loads and opens its window. Done.
2. On `WouldBlock`, `N` connects to the socket. On `ConnectionRefused` or `NotFound`, `O`
   is still starting or the socket is stale: retry `try_lock` and `connect` every 50 ms for
   about 2 s.
3. `N` sends `handoff`. `O` writes its pending save (`Store::hand_off`), stops saving,
   **unlocks**, then replies `handed-off <screen line>` (bounds, sidebar, active). `O`
   stops taking edits from here on: anything typed later would be lost.
4. `N` calls `try_lock`, which succeeds now, then removes and rebinds the socket path and
   loads `workspace.json`. It opens at `O`'s bounds, activates only if `O` was active, and
   after its first frame (`announce_ready`) sends `ready`. `O` quits without saving.
5. If `O` sees EOF or no `ready` within 20 s, it `try_lock`s again. On success it
   `resume`s: rebinds, saves what changed and keeps its window. If the lock is held, a
   newer instance owns the data and `O` quits.

Edge cases:

- **Stale socket:** handled in steps 1 and 2.
- **Crash of `O`:** the kernel frees the lock (observed), so `N` takes over at step 1 with
  `O`'s last save, minus its in-flight debounce.
- **Hung `O`:** the lock is held and `connect` succeeds (the kernel backlog), but there is
  no answer within 2 s. Never write without the lock: show "Nerda Studio is already
  running and not responding" and quit, or open read-only. Never kill `O`.
- **Two new launches at once:** `O` serves one `handoff` at a time and answers others
  `busy`. The loser goes back to step 1 and later takes over from the winner, so the
  newest still wins.
- **`N` dies after `handed-off`:** its lock dies with it, and `O` resumes (step 5).
- **Same-process double lock** (observed `WouldBlock`): keep one `File` for the lock.
  Spawned agents don't inherit it (observed CLOEXEC).
- **Different versions:** add a protocol version to `handoff`. An unknown reply counts as
  a hung `O`.

## G. GPUI's macOS backend at the pinned rev

- `WindowOptions::app_id` reaches `PlatformWindow::set_app_id`, which is empty on macOS
  (`crates/gpui_macos/src/window.rs:1836`); it is for Linux desktop grouping
  (`crates/gpui/src/platform.rs:2617`). `App::set_app_identity` (`crates/gpui/src/app.rs:1930`)
  is a default no-op (`crates/gpui/src/platform.rs:523`) that only Windows and Linux
  override. **Identity on macOS is purely the bundle.**
- Activation policy: `Regular`, set in `applicationDidFinishLaunching`
  (`crates/gpui_macos/src/platform.rs:1265-1268`). Setting it earlier "leaves the menu bar of
  an unbundled app launched from a terminal unclickable" (`:536-548`).
  `App::set_activation_policy` exists (`app.rs:2073`).
- GPUI sets no app name (`create_menu_bar`, `platform.rs:269-294`); AppKit shows
  `CFBundleName`, or the process name when unbundled (observed).
- `App::on_reopen` (`app.rs:335`) runs only when `hasVisibleWindows` is false
  (`platform.rs:1326-1342`) and always returns YES, so AppKit brings the app forward and
  restores a minimized window. `main.rs` already reopens the window this way.
- `App::on_open_urls` (`app.rs:325`, `platform.rs:1439-1464`) is for URL schemes and
  needs a registered bundle (`:820-825`).
- `App::activate` calls the deprecated `activateIgnoringOtherApps` (`platform.rs:643-646`);
  it still brought a directly exec'd bundle to the front (observed).
- `App::app_path` is `NSBundle.mainBundle.bundlePath` (`platform.rs:1096-1101`);
  `App::restart` reopens through `open` (`:595-640`).
- System notifications are disabled without a bundle identifier
  (`crates/gpui_macos/src/system_notifications.rs:120-125`), another gain from bundling.

## H. Where bundling and the takeover live

| Option | Covers | Cost |
| --- | --- | --- |
| Dev runner assembles the bundle | `cargo run -p dev` only | Agents and `cargo run -p studio` stay unbundled |
| `.cargo/config.toml` `target.<cfg>.runner` | `cargo run`, but "applies to `cargo run`, `cargo test` and `cargo bench`" (<https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner>) | Must pass test binaries through; is a shell script (Windows); misses the dev runner (`cargo build`) and direct launches |
| **Binary bundles itself and `execv`s** | Every launch of a debug build | About 40 lines of macOS debug-only code; refresh only when size or mtime changed; write to temp and `rename` (two launches at once) |

`execv` (not `open`) keeps the pid, so the runner's `Child`, pipes, process group and
Ctrl-C still work. `open -n` would give dev builds their own TCC identity, but each rebuild
re-prompts unless signed with a certificate, and the runner loses its child. The runner
keeps three jobs: watch, build, and start `target/debug/studio`. The new process re-execs
into the bundle and takes over from whichever instance runs, the runner's or an agent's,
and the runner then follows only the newest child. That is one handoff path, the same for
dev, agents and the installed app.

Not verified: TCC grants surviving a self-signed rebuild; `open -b` outside `/tmp`;
Spotlight; Windows pipes. Scratch lives in `/tmp/nerda-id` and `/tmp/nerda-lock`; the test
bundles were unregistered with `lsregister -u`.
