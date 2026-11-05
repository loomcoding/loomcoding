use std::fs;
use tempfile::TempDir;

use crate::context::pack::{compose, PackOptions, MAX_CHARS_HARD};
use crate::context::SymbolIndex;

fn project() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn alfa(x: u32) -> u32 {\n    beta(x) + 1\n}\n\npub fn beta(x: u32) -> u32 {\n    x * 2\n}\n\npub fn gamma() {\n    alfa(1);\n    alfa(2);\n}\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src/tests")).unwrap();
    fs::write(
        dir.path().join("src/tests/alfa.rs"),
        "fn prueba() {\n    alfa(3);\n}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("docs/CODE-MAP.md"),
        "# Mapa\n\n## Quiero tocar alfa\n\n1. El símbolo `alfa` vive en `src/lib.rs`.\n\n## Quiero otra cosa\n\nnada\n",
    )
    .unwrap();
    dir
}

fn pack(dir: &TempDir, query: &str, opts: &PackOptions) -> String {
    let index = SymbolIndex::build(dir.path());
    compose(&index, dir.path(), query, opts)
}

#[test]
fn an_existing_symbol_gets_signature_location_body_and_uses() {
    let dir = project();
    let text = pack(&dir, "alfa", &PackOptions::default());
    assert!(text.contains("alfa"), "{text}");
    assert!(text.contains("src/lib.rs:1"), "{text}");
    assert!(text.contains("pub fn alfa(x: u32) -> u32"), "{text}");
    assert!(
        text.contains("beta(x) + 1"),
        "el cuerpo debe aparecer: {text}"
    );
    assert!(text.contains("usos"), "{text}");
    assert!(text.contains("src/lib.rs:10"), "{text}");
}

#[test]
fn tests_that_name_the_symbol_are_listed_apart() {
    let dir = project();
    let text = pack(&dir, "alfa", &PackOptions::default());
    assert!(text.contains("pruebas"), "{text}");
    assert!(text.contains("src/tests/alfa.rs:2"), "{text}");
}

#[test]
fn an_unknown_symbol_says_so_and_lists_the_closest_names() {
    let dir = project();
    let text = pack(&dir, "alf", &PackOptions::default());
    assert!(text.contains("alfa"), "debe proponer el parecido: {text}");
    let text = pack(&dir, "zzz_no_existe", &PackOptions::default());
    assert!(text.contains("no symbol"), "{text}");
}

#[test]
fn the_pack_never_exceeds_max_chars_and_declares_a_cut() {
    let dir = project();
    let opts = PackOptions {
        max_chars: 600,
        max_symbols: 12,
        body_lines: 20,
    };
    let text = pack(&dir, "a", &opts);
    assert!(text.chars().count() <= 600, "{}", text.chars().count());
    assert!(text.contains("recortado"), "{text}");
    assert!(
        text.ends_with('\n') || !text.contains('\r'),
        "termina en línea entera"
    );
}

#[test]
fn the_hard_maximum_clamps_a_huge_request() {
    let dir = project();
    // Muchos símbolos con cuerpos largos: sin techo duro el paquete pasaría de MAX_CHARS_HARD.
    let mut big = String::new();
    for n in 0..400 {
        big.push_str(&format!(
            "pub fn grande_{n}() {{
"
        ));
        for k in 0..18 {
            big.push_str(&format!(
                "    let v{k} = {n} + {k}; // relleno del cuerpo
"
            ));
        }
        big.push_str(
            "}
",
        );
    }
    fs::write(dir.path().join("src/grande.rs"), big).unwrap();
    let opts = PackOptions {
        max_chars: usize::MAX,
        max_symbols: 1000,
        body_lines: 20,
    };
    let text = pack(&dir, "grande", &opts);
    assert!(
        text.chars().count() <= MAX_CHARS_HARD,
        "{}",
        text.chars().count()
    );
    assert!(text.contains("recortado"), "debe declarar el recorte");
}

#[test]
fn the_code_map_section_is_attached_when_it_names_the_symbol() {
    let dir = project();
    let text = pack(&dir, "alfa", &PackOptions::default());
    assert!(text.contains("Quiero tocar alfa"), "{text}");
    assert!(!text.contains("Quiero otra cosa"), "{text}");
}

#[test]
fn an_empty_query_is_refused_politely() {
    let dir = project();
    let text = pack(&dir, "   ", &PackOptions::default());
    assert!(text.contains("empty query"), "{text}");
}
