//! Files compiled into the binary and served to GPUI by path.
//!
//! Icons are outline SVGs on a 24 grid at a 1.5 stroke; their licenses are in
//! The search icon's ring spans 15 of the 24 units, so at `ICON_SIZE`
//! it doesn't look oversized beside 13 pt text in the search field.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// An icon the app draws. GPUI paints an SVG in its own `text_color` only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Check,
    Close,
    Folder,
    FolderAdd,
    FolderUp,
    NewSession,
    PullRequests,
    /// The Return key, shown in a key cap.
    Return,
    Search,
    Settings,
    Sort,
    Usage,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Self::Check => "icons/check.svg",
            Self::Close => "icons/x.svg",
            Self::Folder => "icons/folder.svg",
            Self::FolderAdd => "icons/folder-add.svg",
            Self::FolderUp => "icons/folder-up.svg",
            Self::NewSession => "icons/edit.svg",
            Self::PullRequests => "icons/git-pull-request.svg",
            Self::Return => "icons/corner-down-left.svg",
            Self::Search => "icons/search.svg",
            Self::Settings => "icons/gear.svg",
            Self::Sort => "icons/sort.svg",
            Self::Usage => "icons/gauge.svg",
        }
    }
}

const FILES: &[(&str, &[u8])] = &[
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    ("icons/edit.svg", include_bytes!("../assets/icons/edit.svg")),
    (
        "icons/gauge.svg",
        include_bytes!("../assets/icons/gauge.svg"),
    ),
    ("icons/gear.svg", include_bytes!("../assets/icons/gear.svg")),
    (
        "icons/git-pull-request.svg",
        include_bytes!("../assets/icons/git-pull-request.svg"),
    ),
    (
        "icons/corner-down-left.svg",
        include_bytes!("../assets/icons/corner-down-left.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder.svg"),
    ),
    (
        "icons/folder-add.svg",
        include_bytes!("../assets/icons/folder-add.svg"),
    ),
    (
        "icons/folder-up.svg",
        include_bytes!("../assets/icons/folder-up.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    ("icons/sort.svg", include_bytes!("../assets/icons/sort.svg")),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(file, _)| *file == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(file, _)| file.starts_with(path))
            .map(|(file, _)| (*file).into())
            .collect())
    }
}
