//! text up to the last separator names the folder listed, the rest filters its
//! subfolders by prefix or names a folder to create.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::paths::expand_home;

/// Typed text that names a path, split at its last separator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrowseQuery<'a> {
    /// The folder to list, as typed, ending in a separator: `~/code/`.
    pub directory: &'a str,
    /// What follows the last separator: a filter, or the name of a folder to add.
    pub leaf: &'a str,
}

impl<'a> BrowseQuery<'a> {
    /// `None` unless the text starts like a path: `/`, `~/`, `./` or `../`, and on
    /// Windows also with `\` in place of `/`, a drive (`C:\`) or a share (`\\`).
    pub fn parse(text: &'a str) -> Option<Self> {
        if !starts_like_path(text) {
            return None;
        }
        let split = text.rfind(is_separator)? + 1;
        Some(Self {
            directory: &text[..split],
            leaf: &text[split..],
        })
    }

    /// Starts from the current project's folder rather than a root or home.
    pub fn is_relative(&self) -> bool {
        self.directory.starts_with('.')
    }

    /// The listed folder as a path: home expanded, a relative one joined to `base`.
    /// `None` for a relative path without a base.
    pub fn resolve_directory(&self, base: Option<&Path>) -> Option<PathBuf> {
        let expanded = expand_home(Path::new(self.directory));
        if expanded.is_absolute() || !self.is_relative() {
            Some(expanded)
        } else {
            base.map(|base| base.join(expanded))
        }
    }

    /// The text that lists the subfolder `name`: `~/code/` and `app` give `~/code/app/`.
    pub fn child(&self, name: &str) -> String {
        let separator = self.directory.chars().next_back().unwrap_or('/');
        format!("{}{name}{separator}", self.directory)
    }
}

fn is_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

fn starts_like_path(text: &str) -> bool {
    let unix = ["/", "~/", "./", "../"];
    if unix.iter().any(|prefix| text.starts_with(prefix)) {
        return true;
    }
    if !cfg!(windows) {
        return false;
    }
    let windows = ["\\", "~\\", ".\\", "..\\"];
    let bytes = text.as_bytes();
    let drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && is_separator(bytes[2] as char);
    drive || windows.iter().any(|prefix| text.starts_with(prefix))
}

/// One entry of a listed folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// A folder, or a link to one.
    pub is_folder: bool,
}

/// Why a folder could not be listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListError {
    NotFound,
    NotAFolder,
    Unreadable(ErrorKind),
}

/// A folder's entries, sorted by name ignoring case.
#[derive(Clone, Debug)]
pub struct Listing {
    /// The folder listed, canonical.
    pub folder: PathBuf,
    entries: Vec<Entry>,
}

impl Listing {
    /// Reads `folder`; blocking, so callers run it off the UI thread.
    pub fn read(folder: &Path) -> Result<Self, ListError> {
        let folder = std::fs::canonicalize(folder).map_err(|error| match error.kind() {
            ErrorKind::NotFound => ListError::NotFound,
            ErrorKind::NotADirectory => ListError::NotAFolder,
            kind => ListError::Unreadable(kind),
        })?;
        if !folder.is_dir() {
            return Err(ListError::NotAFolder);
        }
        let read = |error: io::Error| ListError::Unreadable(error.kind());
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&folder).map_err(read)? {
            let entry = entry.map_err(read)?;
            // A name that isn't Unicode can't be typed back into the field.
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            // `metadata` follows links, so a link to a folder lists as one.
            let is_folder = std::fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir());
            entries.push(Entry { name, is_folder });
        }
        entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.name.clone()));
        Ok(Self { folder, entries })
    }

    /// Subfolders whose names start with `leaf`, ignoring case; hidden ones only
    /// when `leaf` starts with a dot.
    pub fn folders<'a>(&'a self, leaf: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        let prefix = leaf.to_lowercase();
        let show_hidden = leaf.starts_with('.');
        self.entries.iter().filter(move |entry| {
            entry.is_folder
                && (show_hidden || !entry.name.starts_with('.'))
                && entry.name.to_lowercase().starts_with(&prefix)
        })
    }

    /// The entry named `leaf`: exactly, else ignoring case where file names do.
    pub fn find(&self, leaf: &str) -> Option<&Entry> {
        let exact = self.entries.iter().find(|entry| entry.name == leaf);
        if exact.is_some() || !cfg!(any(target_os = "macos", windows)) {
            return exact;
        }
        let leaf = leaf.to_lowercase();
        self.entries
            .iter()
            .find(|entry| entry.name.to_lowercase() == leaf)
    }
}

/// What submitting a query does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Adds this folder; adding reports it if it turns out unusable.
    Add(PathBuf),
    /// Creates this folder, then adds it.
    Create(PathBuf),
    /// A file has this name: nothing can be added.
    File(String),
}

impl Target {
    /// What `query` names, given how its folder (at `directory`) listed.
    pub fn of(
        query: &BrowseQuery<'_>,
        directory: &Path,
        listing: Result<&Listing, ListError>,
    ) -> Self {
        let leaf = query.leaf;
        match listing {
            Ok(listing) if leaf.is_empty() => Self::Add(listing.folder.clone()),
            Ok(listing) if leaf == "." || leaf == ".." => Self::Add(listing.folder.join(leaf)),
            Ok(listing) => match listing.find(leaf) {
                Some(entry) if entry.is_folder => Self::Add(listing.folder.join(&entry.name)),
                Some(entry) => Self::File(entry.name.clone()),
                None => Self::Create(listing.folder.join(leaf)),
            },
            Err(ListError::NotFound) => Self::Create(directory.join(leaf)),
            Err(ListError::NotAFolder) => Self::File(file_name(directory)),
            Err(ListError::Unreadable(_)) => Self::Add(directory.join(leaf)),
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// `folder` as text to type, ending in a separator, under `~` when it is inside
/// the home folder and `tilde` asks for it.
pub fn folder_text(folder: &Path, tilde: bool) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let home = dirs::home_dir().map(|home| std::fs::canonicalize(&home).unwrap_or(home));
    let mut text = match home.as_deref().map(|home| folder.strip_prefix(home)) {
        Some(Ok(rest)) if tilde => {
            let mut text = format!("~{separator}");
            text.push_str(&rest.to_string_lossy());
            text
        }
        _ => folder.to_string_lossy().into_owned(),
    };
    // Windows canonical paths are verbatim (`\\?\C:\…`); a drive path reads the same without it.
    if let Some(rest) = text.strip_prefix(r"\\?\")
        && rest.as_bytes().get(1) == Some(&b':')
    {
        text = rest.to_owned();
    }
    if !text.ends_with(is_separator) {
        text.push(separator);
    }
    text
}
