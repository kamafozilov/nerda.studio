//! What a project folder's git checkout is on, read from its files without
//! running git, so it costs two small reads.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Where a checkout's HEAD points.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Head {
    /// On a branch, by its short name: `main`, `feature/sidebar`.
    Branch(String),
    /// On no branch: the first 7 hex digits of the commit checked out.
    Detached(String),
}

/// The head of the checkout holding `dir`, found in `dir` or the nearest folder
/// above it, as git finds it; `None` outside a checkout or when it can't be
/// read. A `.git` file, as in a linked worktree or a submodule, points to the
/// folder that holds the head.
pub fn head(dir: &Path) -> Option<Head> {
    let top = dir
        .ancestors()
        .find(|folder| folder.join(".git").symlink_metadata().is_ok())?;
    let git_dir = git_dir(top)?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref:") {
        let reference = reference.trim();
        let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
        // A checkout keeping its refs in a reftable leaves this stub in HEAD;
        // its real head isn't in a file read this way, so none is shown.
        let unknown = name.is_empty() || name == ".invalid";
        return (!unknown).then(|| Head::Branch(name.to_owned()));
    }
    let id = head.get(..7)?;
    id.chars()
        .all(|digit| digit.is_ascii_hexdigit())
        .then(|| Head::Detached(id.to_owned()))
}

/// The git folder of the checkout whose top is `dir`; `None` when its `.git`
/// is neither a folder nor a file naming one.
fn git_dir(dir: &Path) -> Option<PathBuf> {
    let dot_git = dir.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let text = fs::read_to_string(&dot_git).ok()?;
    let target = Path::new(text.strip_prefix("gitdir:")?.trim());
    Some(dir.join(target))
}
