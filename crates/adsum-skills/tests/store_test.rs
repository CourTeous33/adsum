use adsum_skills::SkillStore;

#[test]
fn list_is_empty_for_fresh_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    assert!(store.list().is_empty());
}

#[test]
fn loads_a_well_formed_skill() {
    let dir = tempfile::tempdir().unwrap();
    let foo_dir = dir.path().join("foo");
    std::fs::create_dir(&foo_dir).unwrap();
    std::fs::write(
        foo_dir.join("SKILL.md"),
        "---\nname: foo\ndescription: a foo\nwhen-to-use: when foo\n---\nbody\n",
    )
    .unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    let listed = store.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].slug, "foo");
    assert_eq!(listed[0].when_to_use, "when foo");
    assert!(store.find("foo").is_some());
    assert!(store.find("missing").is_none());
}

#[test]
fn skips_skill_with_name_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let foo_dir = dir.path().join("foo");
    std::fs::create_dir(&foo_dir).unwrap();
    std::fs::write(
        foo_dir.join("SKILL.md"),
        "---\nname: BAR\ndescription: x\nwhen-to-use: y\n---\n",
    )
    .unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    assert!(store.list().is_empty(), "name-mismatched skill should be skipped");
}

#[test]
fn skips_directory_without_skill_md() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    assert!(store.list().is_empty());
}

#[test]
fn seed_if_empty_writes_bundled_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    store.seed_if_empty().unwrap();
    let listed = store.list();
    // The placeholder bundled skills land in Task 6; Task 7 swaps real content.
    // Both tasks should see at least 2 entries here.
    assert!(listed.iter().any(|s| s.slug == "query"));
    assert!(listed.iter().any(|s| s.slug == "ingest"));
}

#[test]
fn seed_if_empty_is_noop_when_directory_has_content() {
    let dir = tempfile::tempdir().unwrap();
    let custom = dir.path().join("custom");
    std::fs::create_dir(&custom).unwrap();
    std::fs::write(
        custom.join("SKILL.md"),
        "---\nname: custom\ndescription: x\nwhen-to-use: y\n---\nbody",
    )
    .unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    store.seed_if_empty().unwrap();
    let listed = store.list();
    assert_eq!(listed.len(), 1, "should not have seeded over existing content");
    assert_eq!(listed[0].slug, "custom");
}

#[test]
fn reload_picks_up_new_skill() {
    let dir = tempfile::tempdir().unwrap();
    let store = SkillStore::at(dir.path().to_path_buf()).unwrap();
    assert!(store.list().is_empty());
    let foo = dir.path().join("foo");
    std::fs::create_dir(&foo).unwrap();
    std::fs::write(
        foo.join("SKILL.md"),
        "---\nname: foo\ndescription: x\nwhen-to-use: y\n---\nbody",
    )
    .unwrap();
    store.reload().unwrap();
    assert!(store.find("foo").is_some());
}
