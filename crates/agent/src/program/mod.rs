//! Finding the agent's program as a terminal would, and the PATH it runs
//! with. An app opened from Finder or the Dock inherits a bare PATH, so
//! `claude` in `~/.local/bin` isn't on it: the login shell's PATH and the
//! folders installers use are searched as well.

mod login_shell;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Reads the login shell's PATH on a thread of its own, so the first session
/// doesn't wait for the shell to start. Optional: the first lookup reads it
/// otherwise.
pub fn prewarm() {
    let _ = std::thread::Builder::new()
        .name("login shell path".to_owned())
        .spawn(login_shell::path);
}

/// Where a program is looked for, besides a path that names its folder.
struct Search<'a> {
    /// The app's own PATH.
    inherited: Option<&'a OsStr>,
    /// The login shell's PATH.
    login: Option<&'a OsStr>,
    home: Option<&'a Path>,
}

/// The file to run for `program`: a path with a folder in it as given, a bare
/// name looked up on the app's PATH, then the login shell's, then the folders
/// installers put it in. `None` when it is nowhere.
pub(crate) fn find(program: &OsStr) -> Option<PathBuf> {
    if Path::new(program).components().count() > 1 {
        return Some(PathBuf::from(program));
    }
    let inherited = std::env::var_os("PATH");
    let home = dirs::home_dir();
    let search = Search {
        inherited: inherited.as_deref(),
        login: login_shell::path(),
        home: home.as_deref(),
    };
    candidates(program, &search)
        .into_iter()
        .find(|path| is_executable(path))
}

/// The PATH the agent runs with: its program's folder, so a script finds the
/// interpreter installed beside it, then the app's PATH, then the login
/// shell's, each folder once. The tools the agent runs need it as much as the
/// agent does.
pub(crate) fn child_path(program: &Path) -> Option<OsString> {
    let inherited = std::env::var_os("PATH");
    compose(program, inherited.as_deref(), login_shell::path())
}

fn compose(program: &Path, inherited: Option<&OsStr>, login: Option<&OsStr>) -> Option<OsString> {
    let mut folders: Vec<PathBuf> = program.parent().map(Path::to_owned).into_iter().collect();
    for path in [inherited, login].into_iter().flatten() {
        folders.extend(std::env::split_paths(path));
    }
    let mut seen = std::collections::HashSet::new();
    folders.retain(|folder| !folder.as_os_str().is_empty() && seen.insert(folder.clone()));
    std::env::join_paths(folders).ok()
}

/// Every file that could be `name`, in the order they are tried.
fn candidates(name: &OsStr, search: &Search) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    for path in [search.inherited, search.login].into_iter().flatten() {
        folders.extend(std::env::split_paths(path).filter(|folder| folder.is_absolute()));
    }
    let mut files: Vec<PathBuf> = folders
        .iter()
        .flat_map(|folder| named(folder, name))
        .collect();
    if let Some(home) = search.home {
        files.extend(named(&home.join(".local/bin"), name));
        files.extend(named(&home.join(".claude/local"), name));
        files.extend(newest_version(
            &home.join(".local/share").join(name).join("versions"),
        ));
    }
    #[cfg(unix)]
    for folder in ["/opt/homebrew/bin", "/usr/local/bin"] {
        files.extend(named(Path::new(folder), name));
    }
    if let Some(home) = search.home {
        files.extend(named(&home.join(".npm-global/bin"), name));
        #[cfg(windows)]
        files.extend(named(&home.join("AppData/Roaming/npm"), name));
    }
    files
}

/// `name` in `folder`; on Windows as a program or a command script.
fn named(folder: &Path, name: &OsStr) -> Vec<PathBuf> {
    let file = folder.join(name);
    if cfg!(windows) && file.extension().is_none() {
        return ["exe", "cmd"]
            .map(|extension| file.with_extension(extension))
            .into();
    }
    vec![file]
}

/// The newest of the versions installed side by side as files named by their
/// version, such as `2.1.309`.
fn newest_version(folder: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(folder).ok()?;
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            let version: Option<Vec<u64>> = name.split('.').map(|part| part.parse().ok()).collect();
            Some((version?, path))
        })
        .filter(|(_, path)| is_executable(path))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, path)| path)
}

pub(crate) fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn program(path: &Path) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_owned()
    }

    fn found(search: &Search) -> Option<PathBuf> {
        let name = OsStr::new("claude");
        candidates(name, search)
            .into_iter()
            .find(|path| is_executable(path))
    }

    #[test]
    fn the_app_s_path_comes_first_then_the_login_shell_s_then_install_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let home = root.join("home");
        let installed = program(&home.join(".local/bin/claude"));
        let on_login = program(&root.join("login/bin/claude"));
        let on_path = program(&root.join("path/bin/claude"));
        let (path, login) = (
            root.join("path/bin").into_os_string(),
            root.join("login/bin").into_os_string(),
        );
        let mut search = Search {
            inherited: Some(&path),
            login: Some(&login),
            home: Some(&home),
        };
        assert_eq!(found(&search), Some(on_path));
        search.inherited = Some(OsStr::new("/usr/bin:/bin"));
        assert_eq!(found(&search), Some(on_login));
        search.login = None;
        assert_eq!(found(&search), Some(installed));
    }

    #[test]
    fn the_newest_installed_version_is_found_and_a_file_that_cannot_run_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let versions = home.join(".local/share/claude/versions");
        program(&versions.join("2.1.9"));
        let newest = program(&versions.join("2.1.309"));
        std::fs::write(versions.join("2.2.0"), "not executable").unwrap();
        std::fs::write(versions.join("2.1.400.tmp"), "partial download").unwrap();
        let empty = OsStr::new("");
        let search = Search {
            inherited: Some(empty),
            login: None,
            home: Some(&home),
        };
        assert_eq!(found(&search), Some(newest));

        let nowhere = Search {
            inherited: Some(empty),
            login: None,
            home: Some(&dir.path().join("no-home")),
        };
        if !Path::new("/opt/homebrew/bin/claude").exists()
            && !Path::new("/usr/local/bin/claude").exists()
        {
            assert_eq!(found(&nowhere), None);
        }
    }

    #[test]
    fn the_agent_s_path_starts_with_its_own_folder_and_names_each_folder_once() {
        let path = compose(
            Path::new("/home/me/.nvm/bin/claude"),
            Some(OsStr::new("/usr/bin:/bin")),
            Some(OsStr::new("/home/me/.nvm/bin:/usr/bin:/opt/homebrew/bin")),
        );
        assert_eq!(
            path.unwrap(),
            "/home/me/.nvm/bin:/usr/bin:/bin:/opt/homebrew/bin"
        );
    }
}
