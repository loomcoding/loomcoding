use std::fs;
use tempfile::TempDir;

use crate::context::{SymbolIndex, SymbolKind};

#[test]
fn indexes_functions_structs_and_methods() {
    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("lib.rs");
    fs::write(
        &lib_path,
        r#"pub struct Foo;

impl Foo {
    pub fn bar(&self) {}
}

pub fn baz() {}
"#,
    )
    .unwrap();

    let index = SymbolIndex::build(dir.path());

    let foo_defs = index.definitions("Foo");
    assert_eq!(foo_defs.len(), 1);
    assert_eq!(foo_defs[0].kind, SymbolKind::Type);

    let bar_defs = index.definitions("bar");
    assert_eq!(bar_defs.len(), 1);
    assert_eq!(bar_defs[0].kind, SymbolKind::Method);

    let baz_defs = index.definitions("baz");
    assert_eq!(baz_defs.len(), 1);
    assert_eq!(baz_defs[0].kind, SymbolKind::Function);
    assert_eq!(baz_defs[0].line, 7);
}

#[test]
fn ignores_the_target_directory() {
    let dir = TempDir::new().unwrap();
    let target_dir = dir.path().join("target");
    fs::create_dir(&target_dir).unwrap();
    let target_path = target_dir.join("x.rs");
    fs::write(
        &target_path,
        r#"pub fn oculto() {}
"#,
    )
    .unwrap();

    let src_dir = dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    fs::write(
        src_dir.join("y.rs"),
        r#"pub fn visible() {}
"#,
    )
    .unwrap();

    let index = SymbolIndex::build(dir.path());

    let oculto_defs = index.definitions("oculto");
    assert_eq!(oculto_defs.len(), 0);

    let visible_defs = index.definitions("visible");
    assert_eq!(visible_defs.len(), 1);
    assert_eq!(
        visible_defs[0].path,
        std::path::Path::new("src").join("y.rs")
    );
}

#[test]
fn a_file_with_broken_syntax_does_not_panic_nor_hide_its_neighbours() {
    let dir = TempDir::new().unwrap();
    let broken_path = dir.path().join("roto.rs");
    fs::write(
        &broken_path,
        r#"pub fn roto( {{{
"#,
    )
    .unwrap();

    let sano_path = dir.path().join("sano.rs");
    fs::write(
        &sano_path,
        r#"pub fn sano() {}
"#,
    )
    .unwrap();

    let index = SymbolIndex::build(dir.path());

    let sano_defs = index.definitions("sano");
    assert_eq!(sano_defs.len(), 1);
}

#[test]
fn files_are_sorted_relative_and_without_repeats() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("b.rs"),
        "pub fn uno() {}
pub fn dos() {}
",
    )
    .unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "pub fn tres() {}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());
    let files = index.files();
    assert_eq!(
        files,
        vec![
            std::path::PathBuf::from("a.rs"),
            std::path::PathBuf::from("b.rs")
        ]
    );
}

#[test]
fn a_method_is_indexed_once_as_a_method_and_not_also_as_a_function() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("m.rs"),
        "struct S;
impl S {
    fn solo(&self) {}
}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());
    let defs = index.definitions("solo");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].kind, SymbolKind::Method);
    assert_eq!(defs[0].line, 3);
}

#[test]
fn records_call_references_with_file_and_line() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("c.rs"),
        "fn a() { b(); }
fn b() {}
fn c(x: S) { b(); x.metodo(); }
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());
    let lines: Vec<u32> = index.references("b").iter().map(|r| r.line).collect();
    assert_eq!(lines, vec![1, 3]);
    assert_eq!(
        index.references("b")[0].path,
        std::path::PathBuf::from("c.rs")
    );
    assert_eq!(index.references("metodo").len(), 1);
    assert!(index.references("inexistente").is_empty());
}
