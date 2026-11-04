//! Tests: index.

use super::support::*;
use super::*;

#[test]
fn a_component_with_several_examples_has_child_rows() {
    let mut show = Showcase::new();
    show.set_theme_column(column_index("Crush"));
    let rows = show.index_rows();
    // ADAPTADO: el símbolo ya no forma parte del texto; la cabecera se identifica por `marked`.
    let header = rows
        .iter()
        .position(|row| row.catalog_index.is_none() && row.text.starts_with("Assistant message"))
        .expect("cabecera de Assistant message");
    assert_eq!(rows[header].text, "Assistant message ─");
    assert!(rows[header].marked && !rows[header].generic);
    let children: Vec<&str> = rows[header + 1..]
        .iter()
        // Las filas hijas van sangradas y sin símbolo; las de componentes con un ejemplo no.
        .take_while(|row| row.catalog_index.is_some() && row.text.starts_with("  "))
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(children, ["  pending", "  refusal", "  summary"]);
    for row in rows[header + 1..].iter().take(children.len()) {
        assert!(!row.marked);
        let index = row.catalog_index.expect("fila hija seleccionable");
        assert_eq!(show.component_at(index), Some("Assistant message"));
    }
}

#[test]
fn a_component_with_one_example_has_no_child_rows() {
    let mut show = Showcase::new();
    show.set_theme_column(column_index("Crush"));
    let rows = show.index_rows();
    let plan = first_demo(&show, "Plan card", "Crush");
    // ADAPTADO: la fila única guarda el nombre sin símbolo y con `marked` para el dibujo.
    assert!(rows
        .iter()
        .any(|row| row.catalog_index == Some(plan) && row.text == "Plan card" && row.marked));
    assert!(rows.iter().all(|row| !row.text.ends_with("Plan card ─")));
}

#[test]
fn text_filter_matches_component_names_in_the_theme() {
    let mut show = Showcase::new();
    show.set_theme_column(column_index("Crush"));
    for c in "PLAN CARD".chars() {
        show.push_query(c);
    }
    assert_eq!(
        show.filtered_indices(),
        vec![first_demo(&show, "Plan card", "Crush")]
    );
    // El filtro es del tema: en Batten no hay «Plan card».
    show.set_theme_column(column_index("Batten"));
    // ADAPTADO: `index_rows` devuelve `IndexRow`, no tuplas.
    let rows = show.index_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].catalog_index, None);
    assert_eq!(rows[0].text, "Sin resultados");
    // El filtro también casa con la etiqueta del ejemplo.
    show.clear_query();
    show.set_theme_column(column_index("Crush"));
    for c in "pending".chars() {
        show.push_query(c);
    }
    assert!(show.filtered_indices().contains(&first_demo(
        &show,
        "Assistant message",
        "Crush · pending"
    )));
}
