mod support;

use std::{path::Path, process::Command};

use support::TempDir;
use workspace::{Head, Switch, branch_name, branches, switch};

fn git(dir: &Path, args: &[&str]) {
    git_at(dir, args, "2026-01-01T00:00:00");
}

/// Runs git with its commits dated `date`.
fn git_at(dir: &Path, args: &[&str], date: &str) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// A repository on `main` with one commit, and `older` and `newer` branches
/// whose commits are a day apart.
fn repo(temp: &TempDir) -> std::path::PathBuf {
    let repo = temp.dir("repo");
    git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);
    for (branch, date) in [
        ("older", "2026-01-02T00:00:00"),
        ("newer", "2026-01-03T00:00:00"),
    ] {
        git(&repo, &["switch", "--quiet", "-c", branch]);
        git_at(
            &repo,
            &["commit", "--quiet", "--allow-empty", "-m", branch],
            date,
        );
        git(&repo, &["switch", "--quiet", "main"]);
    }
    repo
}

#[test]
fn branches_list_newest_commit_first_with_the_head() {
    let temp = TempDir::new();
    let repo = repo(&temp);
    let found = branches(&repo).unwrap();
    assert_eq!(found.head, Some(Head::Branch("main".into())));
    let names: Vec<&str> = found.local.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["newer", "older", "main"]);
    assert!(found.local.iter().all(|branch| branch.committed.is_some()));
    assert!(found.remote.is_empty());
    assert!(found.worktrees.is_empty());
}

#[test]
fn a_branch_another_worktree_is_on_names_that_worktree() {
    let temp = TempDir::new();
    let repo = repo(&temp);
    let other = temp.path().join("other");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            other.to_str().unwrap(),
            "older",
        ],
    );
    let found = branches(&repo).unwrap();
    let older = found.local.iter().find(|b| b.name == "older").unwrap();
    assert_eq!(older.worktree, Some(other.canonicalize().unwrap()));
    let main = found.local.iter().find(|b| b.name == "main").unwrap();
    assert_eq!(main.worktree, None);
    // A checkout on a branch is reached through it, not listed apart.
    assert!(found.worktrees.is_empty());
    // Seen from the other checkout, `main` is the first one's.
    let there = branches(&other).unwrap();
    assert_eq!(there.head, Some(Head::Branch("older".into())));
    let main = there.local.iter().find(|b| b.name == "main").unwrap();
    assert_eq!(main.worktree, Some(repo.canonicalize().unwrap()));
}

#[test]
fn a_detached_worktree_is_listed_apart() {
    let temp = TempDir::new();
    let repo = repo(&temp);
    let other = temp.path().join("detached");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            other.to_str().unwrap(),
        ],
    );
    let found = branches(&repo).unwrap();
    assert_eq!(found.worktrees.len(), 1);
    assert!(matches!(found.worktrees[0].head, Some(Head::Detached(_))));
}

#[test]
fn remote_branches_without_a_local_one_are_listed_and_tracked() {
    let temp = TempDir::new();
    let origin = repo(&temp);
    let clone = temp.path().join("clone");
    git(
        temp.path(),
        &[
            "clone",
            "--quiet",
            origin.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let found = branches(&clone).unwrap();
    let remote: Vec<String> = found.remote.iter().map(|b| b.full_name()).collect();
    // `main` is checked out locally, and `origin/HEAD` is no branch.
    assert_eq!(remote.len(), 2, "{remote:?}");
    assert!(remote.contains(&"origin/older".to_owned()));
    let older = found.remote.iter().find(|b| b.name == "older").unwrap();
    switch(&clone, &Switch::Track(older.clone())).unwrap();
    let found = branches(&clone).unwrap();
    assert_eq!(found.head, Some(Head::Branch("older".into())));
    assert!(found.remote.iter().all(|branch| branch.name != "older"));
}

#[test]
fn switching_creates_and_refuses_with_a_sentence() {
    let temp = TempDir::new();
    let repo = repo(&temp);
    switch(&repo, &Switch::Branch("newer".into())).unwrap();
    assert_eq!(
        branches(&repo).unwrap().head,
        Some(Head::Branch("newer".into()))
    );
    switch(&repo, &Switch::Create(branch_name(" my  new branch "))).unwrap();
    assert_eq!(
        branches(&repo).unwrap().head,
        Some(Head::Branch("my-new-branch".into()))
    );
    assert_eq!(
        switch(&repo, &Switch::Create("main".into())),
        Err("A branch with that name already exists.".into())
    );
    assert_eq!(
        switch(&repo, &Switch::Create("bad..name".into())),
        Err("That isn't a valid branch name.".into())
    );
    // A change the other branch would overwrite keeps the checkout where it is.
    git(&repo, &["switch", "--quiet", "main"]);
    std::fs::write(repo.join("a.txt"), "changed on main").unwrap();
    git(&repo, &["switch", "--quiet", "-c", "edits"]);
    git(&repo, &["commit", "--quiet", "-am", "edit"]);
    git(&repo, &["switch", "--quiet", "main"]);
    std::fs::write(repo.join("a.txt"), "uncommitted").unwrap();
    assert_eq!(
        switch(&repo, &Switch::Branch("edits".into())),
        Err("Uncommitted changes would be overwritten. Commit or stash them first.".into())
    );
    assert_eq!(
        branches(&repo).unwrap().head,
        Some(Head::Branch("main".into()))
    );
}

#[test]
fn a_folder_outside_git_has_no_branches() {
    let temp = TempDir::new();
    let folder = temp.dir("plain");
    let reason = branches(&folder).unwrap_err();
    assert!(reason.starts_with("Not a git repository"), "{reason}");
}
