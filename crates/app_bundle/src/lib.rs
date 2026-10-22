//! The macOS app bundle the app runs from. macOS knows an app (its name in the
//! menu bar, its Dock icon, ⌘Tab, the permissions it is granted) only by the
//! bundle its executable sits in, so every build runs from one: a build started
//! outside a bundle puts itself in one next to it (`crates/studio/src/bundle.rs`),
//! and `cargo run -p dev -- install` puts a release build in `/Applications`.
//!
//! Development builds and the installed app are two apps to macOS, with their
//! own names, so opening one never just brings the other forward. They share
//! the data folder, and the newest one launched takes over the window.
//!
//! `assets/AppIcon.icns` is made from `assets/AppIcon.svg`: render it to a
//! 1024 px PNG that keeps its transparency, then `sips -z` the ten sizes of an
//! `.iconset` and run `iconutil -c icns`.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Which app a bundle is to macOS.
pub struct Identity {
    pub id: &'static str,
    /// Shown in the menu bar, the Dock and Finder; the bundle is named after it.
    pub name: &'static str,
}

/// Every build run from the source tree.
pub const DEV: Identity = Identity {
    id: "studio.nerda.dev",
    name: "Nerda Studio Dev",
};

/// The app installed in `/Applications`.
pub const RELEASE: Identity = Identity {
    id: "studio.nerda",
    name: "Nerda Studio",
};

const EXECUTABLE: &str = "studio";
const ICON: &[u8] = include_bytes!("../assets/AppIcon.icns");
/// The oldest macOS Rust builds for on Apple silicon.
const MINIMUM_SYSTEM: &str = "11.0";

/// Whether `executable` already runs from a bundle: `….app/Contents/MacOS/…`.
pub fn is_bundled(executable: &Path) -> bool {
    let mut folders = executable.ancestors().skip(1);
    let names = [folders.next(), folders.next(), folders.next()].map(|folder| {
        folder
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
    });
    matches!(names, [Some("MacOS"), Some("Contents"), Some(app)] if app.ends_with(".app"))
}

/// Where the bundle for `identity` goes beside `executable`.
pub fn beside(executable: &Path, identity: &Identity) -> PathBuf {
    executable.with_file_name(format!("{}.app", identity.name))
}

/// Brings the bundle at `app` up to `executable` and returns the executable
/// inside it. Writes only what changed, so a launch with nothing new costs a
/// few file reads; each file is written aside and renamed into place, so two
/// launches at once, or a running copy, never see one half written.
pub fn assemble(
    executable: &Path,
    app: &Path,
    identity: &Identity,
    version: &str,
) -> io::Result<PathBuf> {
    let contents = app.join("Contents");
    let inside = contents.join("MacOS").join(EXECUTABLE);
    fs::create_dir_all(contents.join("MacOS"))?;
    fs::create_dir_all(contents.join("Resources"))?;
    replace_if_changed(
        &contents.join("Info.plist"),
        info_plist(identity, version).as_bytes(),
    )?;
    replace_if_changed(&contents.join("Resources/AppIcon.icns"), ICON)?;
    let source = fs::metadata(executable)?;
    let current = fs::metadata(&inside).ok();
    let same = current.is_some_and(|current| {
        current.len() == source.len() && current.modified().ok() == source.modified().ok()
    });
    if !same {
        let temp = aside(&inside);
        // A clone on APFS: a few milliseconds for any size.
        fs::copy(executable, &temp)?;
        // The copy keeps the source's time, so the next launch can tell it is current.
        fs::File::options()
            .write(true)
            .open(&temp)?
            .set_modified(source.modified()?)?;
        fs::rename(&temp, &inside)?;
    }
    Ok(inside)
}

fn info_plist(identity: &Identity, version: &str) -> String {
    let Identity { id, name } = identity;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>{id}</string>
	<key>CFBundleName</key>
	<string>{name}</string>
	<key>CFBundleDisplayName</key>
	<string>{name}</string>
	<key>CFBundleExecutable</key>
	<string>{EXECUTABLE}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>LSMinimumSystemVersion</key>
	<string>{MINIMUM_SYSTEM}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    )
}

fn replace_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|current| current == bytes) {
        return Ok(());
    }
    let temp = aside(path);
    let mut file = fs::File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)
}

/// A name beside `path` that no other launch writes at the same time.
fn aside(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_executable_in_an_app_counts_as_bundled() {
        assert!(is_bundled(Path::new(
            "/Applications/Nerda Studio.app/Contents/MacOS/studio"
        )));
        assert!(!is_bundled(Path::new("/repo/target/debug/studio")));
        assert!(!is_bundled(Path::new("/x/Contents/MacOS/studio")));
        assert!(!is_bundled(Path::new("studio")));
    }

    #[test]
    fn a_bundle_follows_its_executable_and_skips_what_is_current() {
        let dir = std::env::temp_dir().join(format!("nerda-bundle-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let executable = dir.join("studio");
        fs::write(&executable, b"first build").unwrap();
        let app = beside(&executable, &DEV);
        let inside = assemble(&executable, &app, &DEV, "1.0").unwrap();
        assert!(is_bundled(&inside));
        assert_eq!(fs::read(&inside).unwrap(), b"first build");
        let plist = fs::read_to_string(app.join("Contents/Info.plist")).unwrap();
        assert!(plist.contains("<string>studio.nerda.dev</string>"));

        // Nothing new: the copy inside is left as it is.
        let before = fs::metadata(&inside).unwrap().modified().unwrap();
        assemble(&executable, &app, &DEV, "1.0").unwrap();
        assert_eq!(fs::metadata(&inside).unwrap().modified().unwrap(), before);

        // A new build replaces it.
        fs::write(&executable, b"second build, longer").unwrap();
        assemble(&executable, &app, &DEV, "1.0").unwrap();
        assert_eq!(fs::read(&inside).unwrap(), b"second build, longer");
        let leftovers = fs::read_dir(app.join("Contents/MacOS")).unwrap().count();
        assert_eq!(leftovers, 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
