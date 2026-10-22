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
    /// Back to the session shown before.
    Back,
    Check,
    Close,
    /// A folded project in the session list.
    Collapsed,
    /// An unfolded project in the session list.
    Expanded,
    /// Forward to the session Back left.
    Forward,
    Folder,
    FolderAdd,
    FolderUp,
    /// The list grouped by project: off, then on as the same shape filled.
    GroupByProject,
    GroupByProjectOn,
    NewSession,
    /// A new tab in the top row.
    NewTab,
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
            Self::Back => "icons/arrow-left-01.svg",
            Self::Check => "icons/tick-02.svg",
            Self::Close => "icons/cancel-01.svg",
            Self::Collapsed => "icons/arrow-right-01.svg",
            Self::Expanded => "icons/arrow-down-01.svg",
            Self::Forward => "icons/arrow-right-01.svg",
            Self::Folder => "icons/folder-01.svg",
            Self::FolderAdd => "icons/folder-add.svg",
            Self::FolderUp => "icons/folder-upload.svg",
            Self::GroupByProject => "icons/layers-01.svg",
            Self::GroupByProjectOn => "icons/layers-01-filled.svg",
            Self::NewSession => "icons/pencil-edit-02.svg",
            Self::NewTab => "icons/add-01.svg",
            Self::PullRequests => "icons/git-pull-request.svg",
            Self::Return => "icons/arrow-turn-backward.svg",
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
        "icons/arrow-down-01.svg",
        include_bytes!("../assets/icons/arrow-down-01.svg"),
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
        "icons/layers-01.svg",
        include_bytes!("../assets/icons/layers-01.svg"),
    ),
    (
        "icons/layers-01-filled.svg",
        include_bytes!("../assets/icons/layers-01-filled.svg"),
    ),
    (
        "icons/list-filter.svg",
        include_bytes!("../assets/icons/list-filter.svg"),
    ),
    (
        "icons/pencil-edit-02.svg",
        include_bytes!("../assets/icons/pencil-edit-02.svg"),
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
