use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use crate::AddProjectError;

/// Resolves user input to the canonical absolute folder it names, so equal folders compare equal.
pub(crate) fn resolve_project_dir(path: &Path) -> Result<PathBuf, AddProjectError> {
    let expanded = expand_home(path);
    let absolute = std::path::absolute(&expanded).unwrap_or(expanded);
    let canonical = std::fs::canonicalize(&absolute).map_err(|error| match error.kind() {
        // A file in the middle of the path means the target cannot exist either.
        ErrorKind::NotFound | ErrorKind::NotADirectory => {
            AddProjectError::NotFound(absolute.clone())
        }
        _ => AddProjectError::Io(absolute.clone(), error),
    })?;
    match std::fs::metadata(&canonical) {
        Ok(metadata) if metadata.is_dir() => Ok(canonical),
        Ok(_) => Err(AddProjectError::NotADirectory(canonical)),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Err(AddProjectError::NotFound(canonical))
        }
        Err(error) => Err(AddProjectError::Io(canonical, error)),
    }
}

/// The home folder as a project's path holds it; `None` when the platform
/// names none or it can't be resolved.
pub(crate) fn home_project_dir() -> Option<PathBuf> {
    resolve_project_dir(&dirs::home_dir()?).ok()
}

/// The folder's own name, or the whole path for a root such as `/`.
pub(crate) fn project_title(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

/// Replaces a leading `~` component with the home folder; `~user` is left alone.
pub(crate) fn expand_home(path: &Path) -> PathBuf {
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(first)) if first == "~" => match dirs::home_dir() {
            Some(home) => home.join(components.as_path()),
            None => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

/// The path as a person reads it: inside the home folder it starts with `~`,
/// the counterpart of `expand_home`. The home folder is found once, as given
/// and with its links resolved, so a path written either way is shortened.
pub fn home_relative(path: &Path) -> String {
    static HOMES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    let homes = HOMES.get_or_init(|| {
        let Some(home) = dirs::home_dir() else {
            return Vec::new();
        };
        let canonical = std::fs::canonicalize(&home).unwrap_or_else(|_| home.clone());
        let mut homes = vec![home];
        if canonical != homes[0] {
            homes.push(canonical);
        }
        homes
    });
    match homes.iter().find_map(|home| path.strip_prefix(home).ok()) {
        Some(inside) if inside.as_os_str().is_empty() => "~".to_owned(),
        Some(inside) => Path::new("~").join(inside).display().to_string(),
        None => path.display().to_string(),
    }
}
