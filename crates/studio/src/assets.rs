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
    /// Back to the tab shown before.
    Back,
    Check,
    Close,
    /// The Design page.
    Design,
    /// Forward to the tab Back left.
    Forward,
    Folder,
    FolderAdd,
    FolderUp,
    NewSession,
    /// A new tab in the top row.
    NewTab,
    /// The Notes page.
    Notes,
    PullRequests,
    /// The Return key, shown in a key cap.
    Return,
    /// The Routines page.
    Routines,
    Search,
    Settings,
    Sort,
    Usage,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Self::Back => "icons/arrow-left-01.svg",
            Self::Check => "icons/tick-02.svg",
            Self::Close => "icons/cancel-01.svg",
            Self::Design => "icons/pen-tool-03.svg",
            Self::Forward => "icons/arrow-right-01.svg",
            Self::Folder => "icons/folder-01.svg",
            Self::FolderAdd => "icons/folder-add.svg",
            Self::FolderUp => "icons/folder-upload.svg",
            Self::NewSession => "icons/pencil-edit-02.svg",
            Self::NewTab => "icons/add-01.svg",
            Self::Notes => "icons/note-01.svg",
            Self::PullRequests => "icons/git-pull-request.svg",
            Self::Return => "icons/arrow-turn-backward.svg",
            Self::Routines => "icons/repeat.svg",
            Self::Search => "icons/search.svg",
            Self::Settings => "icons/gear.svg",
            Self::Sort => "icons/list-filter.svg",
            Self::Usage => "icons/gauge.svg",
        }
    }
}

const FILES: &[(&str, &[u8])] = &[
    (
        "icons/add-01.svg",
        include_bytes!("../assets/icons/add-01.svg"),
    ),
    (
        "icons/arrow-left-01.svg",
        include_bytes!("../assets/icons/arrow-left-01.svg"),
    ),
    (
        "icons/arrow-right-01.svg",
        include_bytes!("../assets/icons/arrow-right-01.svg"),
    ),
    (
        "icons/arrow-turn-backward.svg",
        include_bytes!("../assets/icons/arrow-turn-backward.svg"),
    ),
    (
        "icons/cancel-01.svg",
        include_bytes!("../assets/icons/cancel-01.svg"),
    ),
    (
        "icons/folder-01.svg",
        include_bytes!("../assets/icons/folder-01.svg"),
    ),
    (
        "icons/folder-add.svg",
        include_bytes!("../assets/icons/folder-add.svg"),
    ),
    (
        "icons/folder-upload.svg",
        include_bytes!("../assets/icons/folder-upload.svg"),
    ),
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
        "icons/list-filter.svg",
        include_bytes!("../assets/icons/list-filter.svg"),
    ),
    (
        "icons/note-01.svg",
        include_bytes!("../assets/icons/note-01.svg"),
    ),
    (
        "icons/pencil-edit-02.svg",
        include_bytes!("../assets/icons/pencil-edit-02.svg"),
    ),
    (
        "icons/pen-tool-03.svg",
        include_bytes!("../assets/icons/pen-tool-03.svg"),
    ),
    (
        "icons/repeat.svg",
        include_bytes!("../assets/icons/repeat.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/tick-02.svg",
        include_bytes!("../assets/icons/tick-02.svg"),
    ),
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
