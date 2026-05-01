use adsum_wiki::WikiStore;
use std::fs;
use tempfile::tempdir;

#[test]
fn open_on_missing_root_creates_layout_and_bootstrap_files() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().join("wiki"); // does not exist yet

    let _store = WikiStore::open(root.clone()).expect("open");

    assert!(root.is_dir(), "root dir created");
    assert!(root.join("pages").is_dir(), "pages/ subdir created");
    assert!(root.join("index.md").is_file(), "index.md created");
    assert!(root.join("log.md").is_file(), "log.md created");

    let index = fs::read_to_string(root.join("index.md")).expect("read index");
    assert!(
        index.contains("Wiki Index"),
        "index.md has placeholder content, got: {index:?}"
    );
    let log = fs::read_to_string(root.join("log.md")).expect("read log");
    assert!(log.is_empty(), "log.md created empty, got: {log:?}");
}

#[test]
fn open_on_existing_root_does_not_clobber_index() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("pages")).expect("mkdir pages");
    fs::write(root.join("index.md"), "# My Custom Index\n").expect("write index");
    fs::write(root.join("log.md"), "## [2026-04-01] kept | content\n").expect("write log");

    let _store = WikiStore::open(root.clone()).expect("open");

    let index = fs::read_to_string(root.join("index.md")).expect("read index");
    assert_eq!(index, "# My Custom Index\n");
    let log = fs::read_to_string(root.join("log.md")).expect("read log");
    assert_eq!(log, "## [2026-04-01] kept | content\n");
}
