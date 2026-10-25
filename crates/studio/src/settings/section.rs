//! The Settings page's sections, in the order its menu lists them, in groups
//! under headings, and the settings each holds, which the search looks through.

use crate::assets::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
    General,
    Appearance,
    Keyboard,
    Notifications,
    AgentsAndModels,
    Projects,
    Git,
    Advanced,
}

/// One setting, as its row names it.
pub(super) struct Setting {
    pub label: &'static str,
    pub detail: &'static str,
}

impl Section {
    pub const ALL: [Section; 8] = [
        Section::General,
        Section::Appearance,
        Section::Keyboard,
        Section::Notifications,
        Section::AgentsAndModels,
        Section::Projects,
        Section::Git,
        Section::Advanced,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Appearance => "Appearance",
            Section::Keyboard => "Keyboard",
            Section::Notifications => "Notifications",
            Section::AgentsAndModels => "Agents and models",
            Section::Projects => "Projects",
            Section::Git => "Git",
            Section::Advanced => "Advanced",
        }
    }

    /// The heading the menu lists the section under; the first group has none.
    pub fn group(self) -> Option<&'static str> {
        match self {
            Section::General | Section::Appearance | Section::Keyboard | Section::Notifications => {
                None
            }
            Section::AgentsAndModels => Some("Agents"),
            Section::Projects | Section::Git => Some("Code"),
            Section::Advanced => Some("App"),
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Section::General => Icon::Sliders,
            Section::Appearance => Icon::Appearance,
            Section::Keyboard => Icon::Keyboard,
            Section::Notifications => Icon::Notification,
            Section::AgentsAndModels => Icon::Agent,
            Section::Projects => Icon::Folder,
            Section::Git => Icon::GitBranch,
            Section::Advanced => Icon::Tool,
        }
    }

    /// What a section without settings yet will hold, in one line.
    pub fn purpose(self) -> &'static str {
        match self {
            Section::General => "How the app starts and quits.",
            Section::Appearance => "Colors, the glass and motion will be set here.",
            Section::Keyboard => "Shortcuts will be listed and changed here.",
            Section::Notifications => "When the app tells you an agent is done will be set here.",
            Section::AgentsAndModels => {
                "Which agent and model new sessions start with will be set here."
            }
            Section::Projects => "Where projects live and how they open will be set here.",
            Section::Git => "Branches, worktrees and commits will be set here.",
            Section::Advanced => "Where the app keeps its data will be shown here.",
        }
    }

    pub fn settings(self) -> &'static [Setting] {
        match self {
            Section::General => &super::general::SETTINGS,
            _ => &[],
        }
    }

    /// Its place in the menu, which names its row and its fill.
    pub fn index(self) -> usize {
        Section::ALL
            .iter()
            .position(|&each| each == self)
            .expect("every section is in Section::ALL")
    }
}
