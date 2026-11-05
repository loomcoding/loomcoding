use std::fs;

use crate::context::{rank::rank, SymbolIndex};

#[test]
fn a_seed_ranks_above_unrelated_symbols() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "fn a() {}
fn b() {}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());

    let ranked = rank(&index, &["a".to_string()]);
    assert_eq!(ranked[0].0, "a");
}

#[test]
fn callees_of_a_seed_rank_above_strangers() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "fn raiz() { util(); }
fn util() {}
fn ajeno() {}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());

    let ranked = rank(&index, &["raiz".to_string()]);
    let raiz = ranked.iter().find(|(n, _)| *n == "raiz").unwrap();
    let util = ranked.iter().find(|(n, _)| *n == "util").unwrap();
    let ajeno = ranked.iter().find(|(n, _)| *n == "ajeno").unwrap();
    assert!(raiz.1 >= util.1);
    assert!(util.1 >= ajeno.1);
}

#[test]
fn ties_are_broken_by_name_so_the_order_is_stable() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "fn uno() {}
fn dos() {}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());

    let first = rank(&index, &[]);
    let second = rank(&index, &[]);
    assert_eq!(first, second);

    // Los empates salen por nombre ascendente.
    let names: Vec<&str> = first.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["dos", "uno"]);
}

#[test]
fn no_seeds_falls_back_to_global_importance() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "fn a() { c(); }
fn b() { c(); }
fn c() {}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());

    let ranked = rank(&index, &[]);
    assert_eq!(ranked[0].0, "c");
}

#[test]
fn a_seed_pulls_its_neighbourhood_above_a_popular_stranger() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("a.rs"),
        "fn raiz() { hijo(); }
fn hijo() {}
fn x1() { popular(); }
fn x2() { popular(); }
fn x3() { popular(); }
fn popular() {}
",
    )
    .unwrap();
    let index = SymbolIndex::build(dir.path());

    let ranked = rank(&index, &["raiz".to_string()]);
    let score = |name: &str| ranked.iter().find(|(n, _)| n == name).unwrap().1;
    assert!(score("hijo") > score("popular"));
}

#[test]
fn ranking_a_large_repository_stays_fast() {
    let dir = tempfile::tempdir().unwrap();
    for f in 0..60 {
        let mut src = String::new();
        for i in 0..50 {
            let id = f * 50 + i;
            let next = (id * 7 + 1) % 3000;
            let other = (id * 13 + 5) % 3000;
            src.push_str(&format!("fn f{id}() {{ f{next}(); f{other}(); }}\n"));
        }
        fs::write(dir.path().join(format!("m{f}.rs")), src).unwrap();
    }
    let index = SymbolIndex::build(dir.path());

    let started = std::time::Instant::now();
    let ranked = rank(&index, &["f0".to_string()]);
    let took = started.elapsed();
    assert_eq!(ranked.len(), 3000);
    println!("rank de 3000 símbolos: {took:?}");
    assert!(took < std::time::Duration::from_secs(2), "tardó {took:?}");
}
