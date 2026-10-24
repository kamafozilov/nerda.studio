//! Everything a project's repository holds that its checkout could be on:
//! its branches, newest commit first, the remote branches it has no branch
//! for yet, and the other checkouts (worktrees) of the same repository. Read
//! and changed by running git, since packed refs, remotes and worktrees are
//! more than a file read can follow; each call runs one or two git commands
//! and is meant for a background thread.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{Head, Timestamp, head};

/// What a repository holds, as seen from one of its checkouts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Branches {
    /// What this checkout is on.
    pub head: Option<Head>,
    /// The branches, newest commit first.
    pub local: Vec<Branch>,
    /// Remote branches without a local branch of the same name, newest first.
    pub remote: Vec<RemoteBranch>,
    /// The repository's other checkouts on no branch, which no branch
    /// leads to, in the order git lists them.
    pub worktrees: Vec<Worktree>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    /// When its newest commit was made.
    pub committed: Option<Timestamp>,
    /// The other checkout that is on it: git keeps a branch in one
    /// checkout, so going to it means opening that one.
    pub worktree: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteBranch {
    /// The remote's name: `origin`.
    pub remote: String,
    /// The branch's name on the remote: `feature/sidebar`.
    pub name: String,
    pub committed: Option<Timestamp>,
}

impl RemoteBranch {
    /// As git names it: `origin/feature/sidebar`.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.remote, self.name)
    }
}

/// Another checkout of the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// What it is on; `None` for a bare repository's entry.
    pub head: Option<Head>,
}

/// What the checkout should go on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Switch {
    /// A local branch.
    Branch(String),
    /// A new local branch tracking a remote one, named as it is there.
    Track(RemoteBranch),
    /// A new branch at the current commit.
    Create(String),
}

/// What the repository holding `dir` holds; `Err` with git's reason when
/// `dir` is in no repository or git can't be run.
pub fn branches(dir: &Path) -> Result<Branches, String> {
    let top = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?.trim());
    let top = top.canonicalize().unwrap_or(top);
    let worktrees = list_worktrees(dir)?;
    let elsewhere = |name: &str| {
        (worktrees.iter())
            .find(|worktree| {
                worktree.path != top && worktree.head == Some(Head::Branch(name.to_owned()))
            })
            .map(|worktree| worktree.path.clone())
    };
    let refs = git(
        dir,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname)%00%(committerdate:unix)%00%(symref)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for line in refs.lines() {
        let mut fields = line.split('\0');
        let (Some(name), Some(date), Some(symref)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let committed = date
            .parse::<u64>()
            .ok()
            .map(|seconds| Timestamp(seconds * 1000));
        if let Some(name) = name.strip_prefix("refs/heads/") {
            local.push(Branch {
                name: name.to_owned(),
                committed,
                worktree: elsewhere(name),
            });
        } else if let Some(full) = name.strip_prefix("refs/remotes/") {
            // `origin/HEAD` points at another remote branch, listed itself.
            let Some((remote_name, branch)) = full.split_once('/').filter(|_| symref.is_empty())
            else {
                continue;
            };
            remote.push(RemoteBranch {
                remote: remote_name.to_owned(),
                name: branch.to_owned(),
                committed,
            });
        }
    }
    // A remote branch with a local one of its name is that branch's upstream,
    // or will be: picking it would only switch to the local one.
    remote.retain(|branch| local.iter().all(|other| other.name != branch.name));
    Ok(Branches {
        head: head(dir),
        local,
        remote,
        worktrees: worktrees
            .into_iter()
            .filter(|worktree| {
                worktree.path != top && !matches!(worktree.head, Some(Head::Branch(_)))
            })
            .collect(),
    })
}

/// Puts the checkout holding `dir` on `to`; `Err` with why, in a sentence,
/// when git refuses, as when uncommitted changes would be lost.
pub fn switch(dir: &Path, to: &Switch) -> Result<(), String> {
    let full;
    let args: Vec<&str> = match to {
        Switch::Branch(name) => vec!["switch", "--no-guess", name],
        Switch::Track(branch) => {
            full = branch.full_name();
            vec!["switch", "--create", &branch.name, "--track", &full]
        }
        Switch::Create(name) => vec!["switch", "--create", name],
    };
    git(dir, &args).map(drop)
}

/// A branch name made from what was typed: spaces become hyphens, as a
/// branch name can't hold them.
pub fn branch_name(typed: &str) -> String {
    typed.split_whitespace().collect::<Vec<_>>().join("-")
}

fn list_worktrees(dir: &Path) -> Result<Vec<Worktree>, String> {
    let porcelain = git(dir, &["worktree", "list", "--porcelain"])?;
    let mut worktrees = Vec::new();
    for block in porcelain.split("\n\n") {
        let mut path = None;
        let mut head = None;
        for line in block.lines() {
            if let Some(found) = line.strip_prefix("worktree ") {
                let found = PathBuf::from(found);
                path = Some(found.canonicalize().unwrap_or(found));
            } else if let Some(reference) = line.strip_prefix("branch ") {
                let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
                head = Some(Head::Branch(name.to_owned()));
            } else if let Some(id) = line.strip_prefix("HEAD ") {
                head = head.or_else(|| id.get(..7).map(|id| Head::Detached(id.to_owned())));
            } else if line == "bare" {
                path = None;
            }
        }
        if let Some(path) = path {
            worktrees.push(Worktree { path, head });
        }
    }
    Ok(worktrees)
}

/// Runs git in `dir` and gives its output; `Err` with the reason it gave.
/// It never asks for anything: no prompt, no editor, no pager.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .output()
        .map_err(|_| "Git couldn't be run.".to_owned())?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(reason(&String::from_utf8_lossy(&output.stderr)))
}

/// Git's complaint as one sentence.
fn reason(stderr: &str) -> String {
    if stderr.contains("would be overwritten") {
        return "Uncommitted changes would be overwritten. Commit or stash them first.".to_owned();
    }
    if stderr.contains("already exists") {
        return "A branch with that name already exists.".to_owned();
    }
    if stderr.contains("not a valid branch name") {
        return "That isn't a valid branch name.".to_owned();
    }
    let line = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("hint:"))
        .unwrap_or("Git failed.");
    let line = ["fatal: ", "error: "].iter().fold(line, |line, prefix| {
        line.strip_prefix(prefix).unwrap_or(line)
    });
    let mut chars = line.chars();
    let mut sentence: String = chars
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    sentence.push_str(chars.as_str());
    if !sentence.ends_with('.') {
        sentence.push('.');
    }
    sentence
}
