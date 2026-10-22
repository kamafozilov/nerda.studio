mod support;

use support::TempDir;
use workspace::browse::{BrowseQuery, ListError, Listing, Target};

#[test]
fn a_query_splits_at_its_last_separator() {
    let query = BrowseQuery::parse("~/code/ap").unwrap();
    assert_eq!((query.directory, query.leaf), ("~/code/", "ap"));
    let query = BrowseQuery::parse("/").unwrap();
    assert_eq!((query.directory, query.leaf), ("/", ""));
    let query = BrowseQuery::parse("../oʻzbek/Проект").unwrap();
    assert_eq!((query.directory, query.leaf), ("../oʻzbek/", "Проект"));
}

#[test]
fn text_that_does_not_start_like_a_path_is_no_query() {
    for text in ["", " ", "code", "~", ".", "..", "~user/x"] {
        assert_eq!(BrowseQuery::parse(text), None, "{text:?}");
    }
}

#[test]
fn a_child_keeps_the_typed_form() {
    let query = BrowseQuery::parse("~/code/ap").unwrap();
    assert_eq!(query.child("app"), "~/code/app/");
}

#[cfg(unix)]
#[test]
fn a_relative_query_resolves_only_against_a_base() {
    use std::path::Path;
    let query = BrowseQuery::parse("./src/").unwrap();
    assert!(query.is_relative());
    assert_eq!(query.resolve_directory(None), None);
    assert_eq!(
        query.resolve_directory(Some(Path::new("/p"))),
        Some(Path::new("/p/./src/").to_path_buf())
    );
    let home = dirs::home_dir().unwrap();
    let query = BrowseQuery::parse("~/x/").unwrap();
    assert_eq!(query.resolve_directory(None), Some(home.join("x/")));
}

fn fixture() -> (TempDir, Listing) {
    let temp = TempDir::new();
    for dir in ["Apps", "api", "beta", ".git", "Проект"] {
        temp.dir(dir);
    }
    std::fs::write(temp.path().join("apple.txt"), "").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(temp.path().join("beta"), temp.path().join("alias")).unwrap();
    let listing = Listing::read(temp.path()).unwrap();
    (temp, listing)
}

fn names<'a>(listing: &'a Listing, leaf: &'a str) -> Vec<&'a str> {
    listing
        .folders(leaf)
        .map(|entry| entry.name.as_str())
        .collect()
}

#[test]
fn folders_match_by_prefix_ignoring_case_and_skip_files_and_hidden() {
    let (_temp, listing) = fixture();
    #[cfg(unix)]
    assert_eq!(
        names(&listing, ""),
        ["alias", "api", "Apps", "beta", "Проект"]
    );
    assert_eq!(names(&listing, "AP"), ["api", "Apps"]);
    assert_eq!(names(&listing, "про"), ["Проект"]);
    assert_eq!(names(&listing, "."), [".git"]);
    assert!(names(&listing, "zzz").is_empty());
}

#[test]
fn a_listing_reports_what_is_missing_or_not_a_folder() {
    let (temp, _) = fixture();
    assert_eq!(
        Listing::read(&temp.path().join("nope")).unwrap_err(),
        ListError::NotFound
    );
    assert_eq!(
        Listing::read(&temp.path().join("apple.txt")).unwrap_err(),
        ListError::NotAFolder
    );
}

#[test]
fn the_target_adds_existing_folders_and_creates_missing_ones() {
    let (temp, listing) = fixture();
    let folder = listing.folder.clone();
    let target = |text: &str| {
        let query = BrowseQuery::parse(text).unwrap();
        Target::of(&query, temp.path(), Ok(&listing))
    };
    let base = format!("{}/", temp.path().display());
    assert_eq!(target(&base), Target::Add(folder.clone()));
    assert_eq!(
        target(&format!("{base}beta")),
        Target::Add(folder.join("beta"))
    );
    assert_eq!(
        target(&format!("{base}new")),
        Target::Create(folder.join("new"))
    );
    assert_eq!(
        target(&format!("{base}apple.txt")),
        Target::File("apple.txt".into())
    );
    if cfg!(any(target_os = "macos", windows)) {
        // The folder's own spelling, so the project matches it later.
        assert_eq!(
            target(&format!("{base}BETA")),
            Target::Add(folder.join("beta"))
        );
    }
}

#[test]
fn the_target_under_a_missing_folder_creates_the_whole_path() {
    let temp = TempDir::new();
    let missing = temp.path().join("a/b");
    let text = format!("{}/c", missing.display());
    let query = BrowseQuery::parse(&text).unwrap();
    assert_eq!(
        Target::of(&query, &missing, Err(ListError::NotFound)),
        Target::Create(missing.join("c"))
    );
}

// Separators and roots below are Unix's.
#[cfg(unix)]
#[test]
fn folder_text_ends_in_a_separator_and_shortens_home() {
    use workspace::browse::folder_text;
    let home = std::fs::canonicalize(dirs::home_dir().unwrap()).unwrap();
    assert_eq!(folder_text(&home, true), "~/");
    assert_eq!(folder_text(&home.join("code"), true), "~/code/");
    assert_eq!(
        folder_text(&home.join("code"), false),
        format!("{}/", home.join("code").display())
    );
    assert_eq!(folder_text(std::path::Path::new("/"), true), "/");
}
