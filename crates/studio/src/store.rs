//! The app's projects and sessions, saved to disk after every change.

use std::path::PathBuf;

use gpui::{Context, SharedString};
use workspace::{LoadError, Timestamp, Workspace};

/// Shared by the sidebar and the main area; both observe it.
pub struct Store {
    workspace: Workspace,
    /// Where changes are saved; `None` when saving would destroy data the app
    /// could not read, or when the platform has no data directory.
    path: Option<PathBuf>,
    /// A problem the user should know about, shown until dismissed.
    notice: Option<SharedString>,
}

impl Store {
    pub fn load() -> Self {
        let Some(path) = workspace::default_state_path() else {
            return Self::unsaved(
                "This system has no folder for app data, so changes won't be saved.",
            );
        };
        match Workspace::load(&path) {
            Ok(workspace) => Self {
                workspace,
                path: Some(path),
                notice: None,
            },
            Err(LoadError::Corrupt(_)) => Self::set_aside(path),
            // Unreadable but intact: leave the file alone and run without saving
            // rather than overwrite it.
            Err(LoadError::UnsupportedVersion(_)) => Self::unsaved(
                "Your projects were saved by a newer Nerda Studio. Update the app to open \
                 them; changes made here won't be saved.",
            ),
            Err(error) => Self::unsaved(&format!(
                "Couldn't read your saved projects ({error}). Check the permissions of {}; \
                 changes made here won't be saved.",
                path.display()
            )),
        }
    }

    fn unsaved(notice: &str) -> Self {
        Self {
            workspace: Workspace::default(),
            path: None,
            notice: Some(notice.to_owned().into()),
        }
    }

    /// A corrupt file is moved aside, never overwritten, so it can be recovered.
    fn set_aside(path: PathBuf) -> Self {
        let backup = path.with_extension(format!("corrupt-{}.json", Timestamp::now().0));
        let backup_name = backup
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::rename(&path, &backup) {
            Ok(()) => Self {
                workspace: Workspace::default(),
                path: Some(path),
                notice: Some(
                    format!(
                        "Your saved projects couldn't be read, so they were set aside as \
                         {backup_name}. Add your projects again."
                    )
                    .into(),
                ),
            },
            Err(error) => Self::unsaved(&format!(
                "Your saved projects couldn't be read or set aside ({error}). Changes made \
                 here won't be saved."
            )),
        }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn notice(&self) -> Option<&SharedString> {
        self.notice.as_ref()
    }

    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.notify();
    }

    /// Changes the workspace, saves it, and tells observers.
    pub fn update<R>(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Workspace) -> R,
    ) -> R {
        let result = change(&mut self.workspace);
        if let Some(path) = &self.path
            && let Err(error) = self.workspace.save(path)
        {
            self.notice = Some(
                format!(
                    "Couldn't save your changes ({error}). Check free disk space and the \
                     permissions of {}.",
                    path.display()
                )
                .into(),
            );
        }
        cx.notify();
        result
    }
}
