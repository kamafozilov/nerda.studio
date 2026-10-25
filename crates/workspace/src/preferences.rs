//! How the app works, as set on the Settings page, saved with the workspace.
//! A value this build doesn't know or can't read, such as one a newer build
//! wrote, reads as its default: a preference never makes the file unreadable.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{Timestamp, Workspace};

/// What the window opens to when the app starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnLaunch {
    /// The tabs open when the app last quit, as they were.
    #[default]
    LastTabs,
    /// A new session alone: the tabs open before are closed.
    NewSession,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Preferences {
    pub on_launch: OnLaunch,
    /// Cmd-Q in the window quits only on a second press.
    pub confirm_quit: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            on_launch: OnLaunch::LastTabs,
            confirm_quit: true,
        }
    }
}

impl<'de> Deserialize<'de> for Preferences {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let default = Self::default();
        Ok(Self {
            on_launch: match value.get("on_launch").and_then(Value::as_str) {
                Some("new-session") => OnLaunch::NewSession,
                Some("last-tabs") => OnLaunch::LastTabs,
                _ => default.on_launch,
            },
            confirm_quit: (value.get("confirm_quit").and_then(Value::as_bool))
                .unwrap_or(default.confirm_quit),
        })
    }
}

impl Workspace {
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    pub fn set_on_launch(&mut self, on_launch: OnLaunch) {
        self.preferences.on_launch = on_launch;
    }

    pub fn set_confirm_quit(&mut self, confirm_quit: bool) {
        self.preferences.confirm_quit = confirm_quit;
    }

    /// Opens what the app starts on: with [`OnLaunch::NewSession`], closes
    /// every tab, other projects' too, and opens the draft of the project
    /// last on screen. Returns whether anything changed.
    pub fn open_on_launch(&mut self, now: Timestamp) -> bool {
        if self.preferences.on_launch != OnLaunch::NewSession {
            return false;
        }
        let had_tabs = !self.tabs.is_empty();
        self.tabs.clear();
        self.selected = None;
        self.ensure_open_session(now) || had_tabs
    }
}
