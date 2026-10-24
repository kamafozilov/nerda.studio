//! A project's own icon, as its sessions' rows show it: the favicon or logo
//! a web project keeps in one of the places frameworks put it, at the
//! folder's top or one app deep in a monorepo's `apps/`. Built output
//! (`dist`, `out`, `build`) is never read: it may be stale or absent.

use std::path::{Path, PathBuf};

/// Where a project keeps its icon, best first: a vector before a bitmap,
/// a favicon before a logo.
const PLACES: &[&str] = &[
    "favicon.svg",
    "favicon.png",
    "favicon.ico",
    "public/favicon.svg",
    "public/favicon.png",
    "public/favicon.ico",
    "static/favicon.svg",
    "static/favicon.png",
    "static/favicon.ico",
    "app/icon.svg",
    "app/icon.png",
    "app/favicon.ico",
    "src/app/icon.svg",
    "src/app/icon.png",
    "src/app/favicon.ico",
    "icon.svg",
    "icon.png",
    "logo.svg",
    "logo.png",
    "public/logo.svg",
    "public/logo.png",
    "assets/icon.svg",
    "assets/icon.png",
    "assets/logo.svg",
    "assets/logo.png",
];

/// Biggest file read as an icon: a favicon is a few kilobytes, and a larger
/// file is more likely a picture than a mark.
const MAX_BYTES: u64 = 512 * 1024;

/// The project's icon in `folder`, if it keeps one: in the folder itself,
/// else in the first of its `apps/` that does, by name.
pub fn project_icon(folder: &Path) -> Option<PathBuf> {
    icon_in(folder).or_else(|| {
        let mut apps: Vec<PathBuf> = std::fs::read_dir(folder.join("apps"))
            .ok()?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_dir())
            .collect();
        apps.sort();
        apps.iter().find_map(|app| icon_in(app))
    })
}

fn icon_in(folder: &Path) -> Option<PathBuf> {
    PLACES.iter().map(|place| folder.join(place)).find(|path| {
        std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_BYTES)
    })
}
