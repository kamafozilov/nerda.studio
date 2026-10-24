//! Files compiled into the binary and served to GPUI by path.
//!
//! Icons are outline SVGs on a 24 grid at a 1.5 stroke. The search icon's ring
//! spans 15 of the 24 units, so at `ICON_SIZE` it doesn't look oversized beside
//! 13 pt text in the search field.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// An icon the app draws. GPUI paints an SVG in its own `text_color` only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Archives a session: out of the list, into the archive.
    Archive,
    /// Back to the tab shown before.
    Back,
    /// Opens the choice of pattern behind a new session's composer.
    Backdrop,
    Check,
    /// Claude Code's logo, beside its models; a filled mark, not an outline.
    Claude,
    /// Opens a pill's choices.
    ChevronDown,
    Close,
    /// No project: a session working on the computer itself.
    Computer,
    /// The Design page.
    Design,
    /// An attached file that is neither an image nor a folder.
    File,
    /// Forward to the tab Back left.
    Forward,
    Folder,
    FolderAdd,
    FolderOpen,
    FolderUp,
    /// The branch a session's project is on.
    GitBranch,
    /// An attached image.
    Image,
    /// Opens a menu of more actions.
    More,
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
    /// Sends the composer's message.
    Send,
    Settings,
    Sort,
    /// Brings an archived session back to the list.
    Unarchive,
    Usage,
    /// Shows something set aside, such as the backdrop's hidden image.
    View,
    /// Sets something aside without letting go of it.
    ViewOff,
    /// A task handed to another agent.
    Agent,
    /// Something failed.
    Alert,
    /// The agent's list of steps.
    Checklist,
    /// A file the agent wrote whole.
    FileAdd,
    /// A search for files by name.
    FileSearch,
    /// A file the agent read.
    FileView,
    /// A page fetched or a search of the web.
    Globe,
    /// A tool from an MCP server.
    Plug,
    /// A question the agent asked.
    Question,
    /// A skill the agent used.
    Skill,
    /// Scrolls a list to its end.
    ScrollDown,
    /// A command the agent ran.
    Terminal,
    /// The agent's thinking.
    Thinking,
    /// Any other tool the agent called.
    Tool,
}

impl Icon {
    /// The page's icon, the same in the sidebar and in its tab.
    pub fn page(page: workspace::Page) -> Self {
        match page {
            workspace::Page::Notes => Self::Notes,
            workspace::Page::Design => Self::Design,
            workspace::Page::Routines => Self::Routines,
            workspace::Page::Usage => Self::Usage,
            workspace::Page::PullRequests => Self::PullRequests,
            workspace::Page::Settings => Self::Settings,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Self::Archive => "icons/archive-03.svg",
            Self::Back => "icons/arrow-left-01.svg",
            Self::Backdrop => "icons/paint-board.svg",
            Self::Check => "icons/tick-02.svg",
            Self::Claude => "icons/claude.svg",
            Self::ChevronDown => "icons/arrow-down-01.svg",
            Self::Close => "icons/cancel-01.svg",
            Self::Computer => "icons/computer.svg",
            Self::Design => "icons/pen-tool-03.svg",
            Self::File => "icons/file-02.svg",
            Self::Forward => "icons/arrow-right-01.svg",
            Self::Folder => "icons/folder-01.svg",
            Self::FolderAdd => "icons/folder-add.svg",
            Self::FolderOpen => "icons/folder-open.svg",
            Self::FolderUp => "icons/folder-upload.svg",
            Self::GitBranch => "icons/git-branch.svg",
            Self::Image => "icons/image-01.svg",
            Self::More => "icons/more-horizontal.svg",
            Self::NewSession => "icons/pencil-edit-02.svg",
            Self::NewTab => "icons/add-01.svg",
            Self::Notes => "icons/note-01.svg",
            Self::PullRequests => "icons/git-pull-request.svg",
            Self::Return => "icons/arrow-turn-backward.svg",
            Self::Routines => "icons/repeat.svg",
            Self::Search => "icons/search.svg",
            Self::Send => "icons/arrow-up-02.svg",
            Self::Settings => "icons/gear.svg",
            Self::Sort => "icons/list-filter.svg",
            Self::Unarchive => "icons/unarchive-03.svg",
            Self::Usage => "icons/gauge.svg",
            Self::View => "icons/view.svg",
            Self::ViewOff => "icons/view-off-slash.svg",
            Self::Agent => "icons/robot-01.svg",
            Self::Alert => "icons/alert-circle.svg",
            Self::Checklist => "icons/check-list.svg",
            Self::FileAdd => "icons/file-add.svg",
            Self::FileSearch => "icons/folder-search.svg",
            Self::FileView => "icons/file-view.svg",
            Self::Globe => "icons/globe-02.svg",
            Self::Plug => "icons/plug-socket.svg",
            Self::Question => "icons/bubble-chat-question.svg",
            Self::Skill => "icons/magic-wand-01.svg",
            Self::ScrollDown => "icons/arrow-down-02.svg",
            Self::Terminal => "icons/command-line.svg",
            Self::Thinking => "icons/brain-02.svg",
            Self::Tool => "icons/wrench-01.svg",
        }
    }
}

const FILES: &[(&str, &[u8])] = &[
    (
        "icons/arrow-down-02.svg",
        include_bytes!("../assets/icons/arrow-down-02.svg"),
    ),
    (
        "icons/robot-01.svg",
        include_bytes!("../assets/icons/robot-01.svg"),
    ),
    (
        "icons/alert-circle.svg",
        include_bytes!("../assets/icons/alert-circle.svg"),
    ),
    (
        "icons/check-list.svg",
        include_bytes!("../assets/icons/check-list.svg"),
    ),
    (
        "icons/file-add.svg",
        include_bytes!("../assets/icons/file-add.svg"),
    ),
    (
        "icons/folder-search.svg",
        include_bytes!("../assets/icons/folder-search.svg"),
    ),
    (
        "icons/file-view.svg",
        include_bytes!("../assets/icons/file-view.svg"),
    ),
    (
        "icons/globe-02.svg",
        include_bytes!("../assets/icons/globe-02.svg"),
    ),
    (
        "icons/plug-socket.svg",
        include_bytes!("../assets/icons/plug-socket.svg"),
    ),
    (
        "icons/bubble-chat-question.svg",
        include_bytes!("../assets/icons/bubble-chat-question.svg"),
    ),
    (
        "icons/magic-wand-01.svg",
        include_bytes!("../assets/icons/magic-wand-01.svg"),
    ),
    (
        "icons/command-line.svg",
        include_bytes!("../assets/icons/command-line.svg"),
    ),
    (
        "icons/brain-02.svg",
        include_bytes!("../assets/icons/brain-02.svg"),
    ),
    (
        "icons/wrench-01.svg",
        include_bytes!("../assets/icons/wrench-01.svg"),
    ),
    (
        "icons/archive-03.svg",
        include_bytes!("../assets/icons/archive-03.svg"),
    ),
    (
        "icons/folder-open.svg",
        include_bytes!("../assets/icons/folder-open.svg"),
    ),
    (
        "icons/unarchive-03.svg",
        include_bytes!("../assets/icons/unarchive-03.svg"),
    ),
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
        "icons/arrow-up-02.svg",
        include_bytes!("../assets/icons/arrow-up-02.svg"),
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
        "icons/claude.svg",
        include_bytes!("../assets/icons/claude.svg"),
    ),
    (
        "icons/computer.svg",
        include_bytes!("../assets/icons/computer.svg"),
    ),
    (
        "icons/file-02.svg",
        include_bytes!("../assets/icons/file-02.svg"),
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
        "icons/git-branch.svg",
        include_bytes!("../assets/icons/git-branch.svg"),
    ),
    (
        "icons/git-pull-request.svg",
        include_bytes!("../assets/icons/git-pull-request.svg"),
    ),
    (
        "icons/image-01.svg",
        include_bytes!("../assets/icons/image-01.svg"),
    ),
    (
        "icons/list-filter.svg",
        include_bytes!("../assets/icons/list-filter.svg"),
    ),
    (
        "icons/more-horizontal.svg",
        include_bytes!("../assets/icons/more-horizontal.svg"),
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
        "icons/paint-board.svg",
        include_bytes!("../assets/icons/paint-board.svg"),
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
    ("icons/view.svg", include_bytes!("../assets/icons/view.svg")),
    (
        "icons/view-off-slash.svg",
        include_bytes!("../assets/icons/view-off-slash.svg"),
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
