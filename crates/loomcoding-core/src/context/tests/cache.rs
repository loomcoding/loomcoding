use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

use crate::context::cache::index_for;

#[test]
fn an_unchanged_tree_reuses_the_same_index() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.rs"), "pub fn uno() {}\n").unwrap();
    let first = index_for(dir.path());
    let second = index_for(dir.path());
    assert!(Arc::ptr_eq(&first, &second), "no debía reconstruirse");
}

#[test]
fn an_edited_file_rebuilds_the_index_and_never_returns_stale_lines() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("a.rs");
    fs::write(&file, "pub fn uno() {}\n").unwrap();
    let before = index_for(dir.path());
    assert_eq!(before.definitions("uno")[0].line, 1);
    fs::write(
        &file,
        "// nuevo comentario\n\npub fn uno() {}\npub fn dos() {}\n",
    )
    .unwrap();
    let after = index_for(dir.path());
    assert!(!Arc::ptr_eq(&before, &after), "debía reconstruirse");
    assert_eq!(after.definitions("uno")[0].line, 3);
    assert_eq!(after.definitions("dos").len(), 1);
}

#[test]
fn a_new_file_rebuilds_the_index() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.rs"), "pub fn uno() {}\n").unwrap();
    let before = index_for(dir.path());
    fs::write(dir.path().join("b.rs"), "pub fn otro() {}\n").unwrap();
    let after = index_for(dir.path());
    assert_eq!(before.definitions("otro").len(), 0);
    assert_eq!(after.definitions("otro").len(), 1);
}
