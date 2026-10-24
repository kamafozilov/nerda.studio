mod support;

use support::TempDir;
use workspace::project_icon;

fn touch(temp: &TempDir, relative: &str, bytes: usize) {
    let path = temp.path().join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![0_u8; bytes]).unwrap();
}

#[test]
fn a_folder_without_an_icon_has_none() {
    let temp = TempDir::new();
    touch(&temp, "README.md", 10);
    assert_eq!(project_icon(temp.path()), None);
}

#[test]
fn a_vector_favicon_comes_before_a_bitmap_and_a_logo() {
    let temp = TempDir::new();
    touch(&temp, "logo.png", 10);
    touch(&temp, "public/favicon.ico", 10);
    touch(&temp, "public/favicon.svg", 10);
    assert_eq!(
        project_icon(temp.path()),
        Some(temp.path().join("public/favicon.svg"))
    );
}

#[test]
fn a_monorepo_lends_the_first_app_s_icon_unless_the_top_has_one() {
    let temp = TempDir::new();
    touch(&temp, "apps/web/public/favicon.png", 10);
    touch(&temp, "apps/admin/src/app/icon.png", 10);
    assert_eq!(
        project_icon(temp.path()),
        Some(temp.path().join("apps/admin/src/app/icon.png"))
    );
    touch(&temp, "favicon.svg", 10);
    assert_eq!(
        project_icon(temp.path()),
        Some(temp.path().join("favicon.svg"))
    );
}

#[test]
fn built_output_and_oversized_files_are_skipped() {
    let temp = TempDir::new();
    touch(&temp, "dist/favicon.ico", 10);
    touch(&temp, "out/icon.png", 10);
    touch(&temp, "favicon.png", 600 * 1024);
    assert_eq!(project_icon(temp.path()), None);
}
