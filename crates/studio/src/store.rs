//! The app's projects and sessions, saved to disk after every change.
//!
//! The UI thread only encodes the workspace; the file is written and flushed on
//! the background executor, so a click that changes something never waits on the
//! disk. A change that leaves the file's contents as they were writes nothing.
//! Quitting writes the newest state before the app exits.

use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gpui::{Context, SharedString, Subscription};
use workspace::{Head, LoadError, ProjectId, Timestamp, Workspace};

/// Shared by the sidebar and the main area; both observe it.
pub struct Store {
    workspace: Workspace,
    /// Where changes are saved; `None` when saving would destroy data the app
    /// could not read, or when the platform has no data directory.
    path: Option<PathBuf>,
    /// A problem the user should know about, shown until dismissed.
    notice: Option<SharedString>,
    /// Each project's checkout head, as the sidebar last read it; not saved.
    heads: HashMap<ProjectId, Head>,
    /// The file's contents as last handed to the writer.
    saved: Option<Arc<[u8]>>,
    /// Where saves go once a newer instance has taken the file over; see
    /// [`Store::hand_off`].
    handed_off: Option<PathBuf>,
    /// Numbers the saves, newest highest.
    serial: u64,
    writer: Arc<Writer>,
    _quit: Option<Subscription>,
}

/// Writes saves in order of their numbers: one that finds a newer one already
/// written skips, so a slow write can never put an older state back.
#[derive(Default)]
struct Writer {
    /// The number of the save on disk.
    written: Mutex<u64>,
}

impl Writer {
    fn write(&self, serial: u64, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut written = self
            .written
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if serial <= *written {
            return Ok(());
        }
        workspace::write_state(path, bytes)?;
        *written = serial;
        Ok(())
    }
}

impl Store {
    pub fn load(cx: &mut Context<Self>) -> Self {
        let (workspace, path, notice) = Self::read();
        // Quitting must not lose a save still on its way to the disk.
        let quit = path.is_some().then(|| {
            cx.on_app_quit(|store, _| {
                if let (Some(path), Some(bytes)) = (&store.path, &store.saved) {
                    store.writer.write(store.serial, path, bytes).ok();
                }
                async {}
            })
        });
        Self {
            workspace,
            path,
            notice,
            heads: HashMap::new(),
            saved: None,
            handed_off: None,
            serial: 0,
            writer: Arc::default(),
            _quit: quit,
        }
    }

    fn read() -> (Workspace, Option<PathBuf>, Option<SharedString>) {
        let Some(path) = workspace::default_state_path() else {
            return Self::unsaved(
                "This system has no folder for app data, so changes won\u{2019}t be saved.",
            );
        };
        match Workspace::load(&path) {
            Ok(workspace) => (workspace, Some(path), None),
            Err(LoadError::Corrupt(_)) => Self::set_aside(path),
            // Unreadable but intact: leave the file alone and run without saving
            // rather than overwrite it.
            Err(LoadError::UnsupportedVersion(_)) => Self::unsaved(
                "Your projects were saved by a newer Nerda Studio. Update the app to open \
                 them; changes made here won\u{2019}t be saved.",
            ),
            Err(LoadError::Io(error)) if is_denied(&error) => Self::unsaved(&format!(
                "Nerda Studio isn\u{2019}t allowed to read your saved projects in {}; changes \
                 made here won\u{2019}t be saved.",
                folder_of(&path)
            )),
            Err(_) => Self::unsaved(&format!(
                "Couldn\u{2019}t read your saved projects in {}; changes made here won\u{2019}t \
                 be saved.",
                folder_of(&path)
            )),
        }
    }

    fn unsaved(notice: &str) -> (Workspace, Option<PathBuf>, Option<SharedString>) {
        (Workspace::default(), None, Some(notice.to_owned().into()))
    }

    /// A corrupt file is moved aside, never overwritten, so it can be recovered.
    fn set_aside(path: PathBuf) -> (Workspace, Option<PathBuf>, Option<SharedString>) {
        let backup = path.with_extension(format!("corrupt-{}.json", Timestamp::now().0));
        let backup_name = backup
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::rename(&path, &backup) {
            Ok(()) => (
                Workspace::default(),
                Some(path),
                Some(
                    format!(
                        "Your saved projects couldn\u{2019}t be read, so they were set aside as \
                         {backup_name}. Add your projects again."
                    )
                    .into(),
                ),
            ),
            Err(_) => Self::unsaved(&format!(
                "Your saved projects in {} couldn\u{2019}t be read or set aside; changes made \
                 here won\u{2019}t be saved.",
                folder_of(&path)
            )),
        }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn notice(&self) -> Option<&SharedString> {
        self.notice.as_ref()
    }

    /// What the project's checkout is on; `None` outside git, or before the
    /// sidebar first read it.
    pub fn head(&self, project: ProjectId) -> Option<&Head> {
        self.heads.get(&project)
    }

    /// Takes the heads just read; observers hear of it only if one changed.
    pub fn set_heads(&mut self, heads: HashMap<ProjectId, Head>, cx: &mut Context<Self>) {
        if self.heads != heads {
            self.heads = heads;
            cx.notify();
        }
    }

    /// Takes one project's head, read just after the app changed it.
    pub fn set_head(&mut self, project: ProjectId, head: Option<Head>, cx: &mut Context<Self>) {
        let changed = match head {
            Some(head) => self.heads.insert(project, head.clone()) != Some(head),
            None => self.heads.remove(&project).is_some(),
        };
        if changed {
            cx.notify();
        }
    }

    /// Shows `notice` under the sidebar's header until dismissed.
    pub fn show_notice(&mut self, notice: SharedString, cx: &mut Context<Self>) {
        self.notice = Some(notice);
        cx.notify();
    }

    /// The folder the workspace is saved in, while this instance saves it:
    /// what else the app keeps beside it goes there too, and nowhere when
    /// nothing is saved.
    pub fn data_dir(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
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
        self.save(cx);
        cx.notify();
        result
    }

    /// A newer instance takes over: writes the pending save now and stops
    /// saving, so the new one loads every change and owns the file from then on.
    pub fn hand_off(&mut self) {
        let Some(path) = self.path.take() else {
            return;
        };
        if let Some(bytes) = &self.saved {
            self.writer.write(self.serial, &path, bytes).ok();
        }
        self.handed_off = Some(path);
    }

    /// The newer instance never took over, so this one saves again, starting
    /// with what changed meanwhile.
    pub fn resume(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.handed_off.take() {
            self.path = Some(path);
            self.saved = None;
            self.save(cx);
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let bytes: Arc<[u8]> = match self.workspace.encode() {
            Ok(bytes) => bytes.into(),
            Err(error) => return self.save_failed(&error, &path),
        };
        if self.saved.as_deref() == Some(&*bytes) {
            return;
        }
        self.serial += 1;
        let serial = self.serial;
        self.saved = Some(bytes.clone());
        let writer = self.writer.clone();
        cx.spawn(async move |store, cx| {
            let written = {
                let path = path.clone();
                cx.background_executor()
                    .spawn(async move { writer.write(serial, &path, &bytes) })
                    .await
            };
            if let Err(error) = written {
                store
                    .update(cx, |store, cx| {
                        // The next change tries again, even one back to this state.
                        store.saved = None;
                        store.save_failed(&error, &path);
                        cx.notify();
                    })
                    .ok();
            }
        })
        .detach();
    }

    /// What went wrong, in words the reader can act on, without the system's
    /// error codes.
    fn save_failed(&mut self, error: &std::io::Error, path: &Path) {
        let folder = folder_of(path);
        let notice = if is_denied(error) {
            format!(
                "Couldn\u{2019}t save your changes: Nerda Studio isn\u{2019}t allowed to write to {folder}."
            )
        } else if error.kind() == ErrorKind::StorageFull {
            "Couldn\u{2019}t save your changes: the disk is full. Free some space; the next \
             change saves them all."
                .to_owned()
        } else {
            format!("Couldn\u{2019}t save your changes to {folder}; the next change tries again.")
        };
        self.notice = Some(notice.into());
    }
}

/// Whether the system refused access, as opposed to something going wrong.
fn is_denied(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
    )
}

/// The folder holding `path`, as a person reads it: `~` for home.
fn folder_of(path: &Path) -> String {
    workspace::home_relative(path.parent().unwrap_or(path))
}

#[cfg(test)]
mod tests {
    use super::Writer;

    #[test]
    fn a_slow_older_save_never_replaces_a_newer_one() {
        let dir = std::env::temp_dir().join(format!("nerda-writer-{}", std::process::id()));
        let path = dir.join("workspace.json");
        let writer = Writer::default();
        writer.write(2, &path, b"newer").unwrap();
        writer.write(1, &path, b"older").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"newer");
        writer.write(3, &path, b"newest").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"newest");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
