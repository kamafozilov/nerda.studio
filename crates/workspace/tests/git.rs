mod support;

use std::fs;

use support::TempDir;
use workspace::{Head, head};

#[test]
fn a_branch_is_named_without_its_ref_prefix() {
    let temp = TempDir::new();
    let git = temp.dir("repo/.git");
    fs::write(git.join("HEAD"), "ref: refs/heads/feature/sidebar\n").unwrap();
    assert_eq!(
        head(&temp.path().join("repo")),
        Some(Head::Branch("feature/sidebar".into()))
    );
}

#[test]
fn a_detached_head_is_its_short_commit() {
    let temp = TempDir::new();
    let git = temp.dir("repo/.git");
    fs::write(
        git.join("HEAD"),
        "9c894a9f0e1d2c3b4a5968778695a4b3c2d1e0f9\n",
    )
    .unwrap();
    assert_eq!(
        head(&temp.path().join("repo")),
        Some(Head::Detached("9c894a9".into()))
    );
}

#[test]
fn a_folder_inside_a_checkout_reads_the_checkout() {
    let temp = TempDir::new();
    let git = temp.dir("repo/.git");
    fs::write(git.join("HEAD"), "ref: refs/heads/main").unwrap();
    let inner = temp.dir("repo/crates/studio");
    assert_eq!(head(&inner), Some(Head::Branch("main".into())));
}

#[test]
fn a_git_file_points_to_the_worktree_folder() {
    let temp = TempDir::new();
    let worktree_git = temp.dir("main/.git/worktrees/side");
    fs::write(worktree_git.join("HEAD"), "ref: refs/heads/side\n").unwrap();
    let side = temp.dir("side");
    fs::write(
        side.join(".git"),
        format!("gitdir: {}\n", worktree_git.display()),
    )
    .unwrap();
    assert_eq!(head(&side), Some(Head::Branch("side".into())));
    // A relative gitdir is relative to the folder holding the file.
    fs::write(side.join(".git"), "gitdir: ../main/.git/worktrees/side").unwrap();
    assert_eq!(head(&side), Some(Head::Branch("side".into())));
}

#[test]
fn no_checkout_or_an_unreadable_head_is_none() {
    let temp = TempDir::new();
    let plain = temp.dir("plain");
    // A broken `.git` ends the search, as in git: the temp folder may sit
    // inside a checkout on some machines, and that is not this folder's.
    fs::write(plain.join(".git"), "not a gitdir line").unwrap();
    assert_eq!(head(&plain), None);
    let empty = temp.dir("empty/.git");
    fs::write(empty.join("HEAD"), "").unwrap();
    assert_eq!(head(&temp.path().join("empty")), None);
    fs::write(empty.join("HEAD"), "ref: refs/heads/").unwrap();
    assert_eq!(head(&temp.path().join("empty")), None);
}

#[test]
fn a_reftable_stub_head_is_none() {
    let temp = TempDir::new();
    let git = temp.dir("repo/.git");
    std::fs::create_dir_all(git.join("reftable")).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/.invalid\n").unwrap();
    assert_eq!(head(&temp.path().join("repo")), None);
}
