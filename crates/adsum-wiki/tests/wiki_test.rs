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

#[test]
fn read_index_returns_bootstrap_placeholder_then_write_index_overwrites_it() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let store = WikiStore::open(root).expect("open");

    let placeholder = store.read_index().expect("read placeholder");
    assert!(placeholder.contains("Wiki Index"));

    store
        .write_index("# Custom\n\nbody\n")
        .expect("write index");
    let after = store.read_index().expect("read after write");
    assert_eq!(after, "# Custom\n\nbody\n");
}

#[test]
fn append_log_accumulates_and_read_log_returns_full_content() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let store = WikiStore::open(root).expect("open");

    assert_eq!(store.read_log().expect("read empty"), "");

    store
        .append_log("## [2026-05-01] ingest | one")
        .expect("append 1");
    store
        .append_log("## [2026-05-01] ingest | two")
        .expect("append 2");

    let log = store.read_log().expect("read log");
    assert_eq!(
        log,
        "## [2026-05-01] ingest | one\n## [2026-05-01] ingest | two\n"
    );
}

#[test]
fn write_page_then_read_page_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store
        .write_page("font-kit-bug", "# notes\nbody\n")
        .expect("write");
    let content = store.read_page("font-kit-bug").expect("read");
    assert_eq!(content, "# notes\nbody\n");
}

#[test]
fn write_page_overwrites_existing_content() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.write_page("foo", "first").expect("write 1");
    store.write_page("foo", "second").expect("write 2");
    assert_eq!(store.read_page("foo").expect("read"), "second");
}

#[test]
fn write_page_rejects_invalid_slugs() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let bad = [
        "",              // empty
        "Foo",           // uppercase
        "foo bar",       // space
        "foo.md",        // dot
        "foo/bar",       // slash
        "..",            // path traversal
        "-leading-dash", // leading dash
        "foo_bar",       // underscore
    ];
    for slug in bad {
        let result = store.write_page(slug, "x");
        assert!(
            matches!(result, Err(adsum_wiki::WikiError::InvalidSlug(_))),
            "expected InvalidSlug for {slug:?}, got {result:?}"
        );
    }
}

#[test]
fn read_page_returns_page_not_found_for_missing_slug() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let result = store.read_page("does-not-exist");
    assert!(
        matches!(&result, Err(adsum_wiki::WikiError::PageNotFound(s)) if s == "does-not-exist"),
        "expected PageNotFound, got {result:?}"
    );
}

#[test]
fn list_pages_on_fresh_wiki_is_empty() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let pages = store.list_pages().expect("list");
    assert!(pages.is_empty(), "fresh wiki has no pages, got {pages:?}");
}

#[test]
fn list_pages_returns_slugs_sorted_modified_descending() {
    use std::thread::sleep;
    use std::time::Duration;

    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.write_page("first", "1").expect("write 1");
    sleep(Duration::from_millis(20));
    store.write_page("second", "2").expect("write 2");
    sleep(Duration::from_millis(20));
    store.write_page("third", "3").expect("write 3");

    let pages = store.list_pages().expect("list");
    let slugs: Vec<&str> = pages.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, vec!["third", "second", "first"]);
    // Sanity check: timestamps are monotonically non-increasing in the list.
    for window in pages.windows(2) {
        assert!(
            window[0].modified_at >= window[1].modified_at,
            "pages not sorted desc: {:?}",
            pages
        );
    }
}

#[test]
fn list_pages_includes_non_conforming_filenames() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    // Drop a file by hand with a non-conforming name (caps + space).
    std::fs::write(
        dir.path().join("pages").join("Some Entity.md"),
        "hand-edited\n",
    )
    .expect("write file");
    store.write_page("normal", "ok").expect("write normal");

    let slugs: std::collections::HashSet<String> = store
        .list_pages()
        .expect("list")
        .into_iter()
        .map(|p| p.slug)
        .collect();

    assert!(slugs.contains("normal"));
    assert!(slugs.contains("Some Entity"));
}

#[test]
fn create_page_writes_file_with_content_and_lists_it() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store
        .create_page("hello", "# Hello\n\nbody\n")
        .expect("create");

    let body = store.read_page("hello").expect("read");
    assert_eq!(body, "# Hello\n\nbody\n");

    let pages = store.list_pages().expect("list");
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].slug, "hello");
}

#[test]
fn create_page_returns_page_already_exists_when_slug_taken() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.create_page("dup", "first").expect("create first");
    let result = store.create_page("dup", "second");
    assert!(
        matches!(&result, Err(adsum_wiki::WikiError::PageAlreadyExists(s)) if s == "dup"),
        "expected PageAlreadyExists, got {result:?}"
    );

    // Existing content must NOT have been clobbered.
    let body = store.read_page("dup").expect("read");
    assert_eq!(body, "first");
}

#[test]
fn create_page_rejects_invalid_slug_without_writing() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let result = store.create_page("Bad Slug", "x");
    assert!(matches!(result, Err(adsum_wiki::WikiError::InvalidSlug(_))));

    let pages = store.list_pages().expect("list");
    assert!(pages.is_empty(), "no file should have been written");
}

#[test]
fn delete_page_removes_file_and_drops_from_list() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.create_page("temp", "x").expect("create");
    store.delete_page("temp").expect("delete");

    let pages = store.list_pages().expect("list");
    assert!(pages.is_empty(), "deleted page should not appear in list");

    let result = store.read_page("temp");
    assert!(matches!(
        result,
        Err(adsum_wiki::WikiError::PageNotFound(_))
    ));
}

#[test]
fn delete_page_returns_page_not_found_for_missing_slug() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let result = store.delete_page("never-existed");
    assert!(
        matches!(&result, Err(adsum_wiki::WikiError::PageNotFound(s)) if s == "never-existed"),
        "expected PageNotFound, got {result:?}"
    );
}

#[test]
fn delete_page_rejects_invalid_slug() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let result = store.delete_page("Bad Slug");
    assert!(matches!(result, Err(adsum_wiki::WikiError::InvalidSlug(_))));
}

#[test]
fn rename_page_moves_file_preserving_content() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.create_page("old", "body\n").expect("create");
    store.rename_page("old", "new").expect("rename");

    assert!(matches!(
        store.read_page("old"),
        Err(adsum_wiki::WikiError::PageNotFound(_))
    ));
    let body = store.read_page("new").expect("read renamed");
    assert_eq!(body, "body\n");
}

#[test]
fn rename_page_same_slug_is_noop_success() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.create_page("same", "body").expect("create");
    store.rename_page("same", "same").expect("rename same");

    let body = store.read_page("same").expect("read");
    assert_eq!(body, "body");
}

#[test]
fn rename_page_returns_page_not_found_when_source_missing() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    let result = store.rename_page("ghost", "target");
    assert!(
        matches!(&result, Err(adsum_wiki::WikiError::PageNotFound(s)) if s == "ghost"),
        "expected PageNotFound for source, got {result:?}"
    );
}

#[test]
fn rename_page_returns_page_already_exists_when_dest_taken() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.create_page("a", "first").expect("create a");
    store.create_page("b", "second").expect("create b");

    let result = store.rename_page("a", "b");
    assert!(
        matches!(&result, Err(adsum_wiki::WikiError::PageAlreadyExists(s)) if s == "b"),
        "expected PageAlreadyExists for dest, got {result:?}"
    );

    // Both files must remain untouched on a rejected rename.
    assert_eq!(store.read_page("a").expect("read a"), "first");
    assert_eq!(store.read_page("b").expect("read b"), "second");
}

#[test]
fn rename_page_rejects_invalid_slugs_in_either_position() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store.create_page("ok", "x").expect("create");

    let bad_old = store.rename_page("Bad Slug", "ok2");
    assert!(matches!(
        bad_old,
        Err(adsum_wiki::WikiError::InvalidSlug(_))
    ));

    let bad_new = store.rename_page("ok", "Bad New");
    assert!(matches!(
        bad_new,
        Err(adsum_wiki::WikiError::InvalidSlug(_))
    ));
}

#[test]
fn write_log_overwrites_existing_content() {
    let dir = tempdir().expect("tempdir");
    let store = WikiStore::open(dir.path().to_path_buf()).expect("open");

    store
        .append_log("first line\n")
        .expect("append");
    store
        .write_log("# Replaced\n\nbody\n")
        .expect("write");

    let after = store.read_log().expect("read");
    assert_eq!(after, "# Replaced\n\nbody\n");
}
