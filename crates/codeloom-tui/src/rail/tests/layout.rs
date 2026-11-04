//! Tests: layout.

use super::support::*;
use super::*;

#[test]
fn grouping_text_wraps_inside_the_rail_width() {
    let texto = "running (submitted, working, input-required, auth-required)";
    let lineas = partir_por_palabras(texto, 20);
    assert!(lineas.len() > 1, "no parte el texto: {lineas:?}");
    assert!(
        lineas.iter().all(|linea| anchura(linea) <= 20),
        "una línea supera el rail: {lineas:?}"
    );
    assert_eq!(lineas.join(" "), texto, "se pierden palabras: {lineas:?}");
}

#[test]
fn rail_without_subagents_is_unchanged() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));

    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains("SUBAGENTS")),
        "aparece la sección SUBAGENTS sin subagentes: {rows:?}"
    );
}

#[test]
fn rail_shows_return_hint_inside_a_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(9));
    app.cwd = "/proj".into();
    app.parent_stack.push(SessionId(1));

    let rows = rows_of(&app, 40, 24);
    let idx = rows
        .iter()
        .position(|row| row.contains("CODELOOM"))
        .expect("el rail muestra el título CODELOOM");
    assert!(rows[idx + 1].contains("/proj"), "falta el cwd: {rows:?}");
    assert!(
        rows[idx + 2].contains("↩ /parent · Alt+←"),
        "falta la línea de retorno bajo el cwd: {:?}",
        rows[idx + 2]
    );
}

#[test]
fn rail_has_no_return_hint_at_top_level() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.cwd = "/proj".into();

    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains('↩')),
        "línea de retorno en el nivel superior: {rows:?}"
    );
}

#[test]
fn return_hint_is_clipped_to_rail_width() {
    let mut app = App::new();
    app.session_id = Some(SessionId(9));
    app.cwd = "/proj".into();
    app.parent_stack.push(SessionId(1));

    // Con un borde izquierdo, la anchura útil del rail es 11 columnas.
    let rows = rows_of(&app, 12, 24);
    let fila = rows
        .iter()
        .find(|row| row.contains('↩'))
        .expect("la línea de retorno aparece también en un rail estrecho");
    assert!(fila.contains('…'), "la línea no se recorta: {fila:?}");
    assert!(
        !fila.contains("Alt"),
        "la línea desborda la anchura del rail: {fila:?}"
    );
}

#[test]
fn rail_shows_effort_when_set() {
    let mut app = App::new();
    app.set_provider_settings(saved_settings(Some("high")));
    let rows = rows_of(&app, 40, 20);
    assert!(
        rows.iter().any(|row| row.contains("effort high")),
        "el rail muestra el esfuerzo bajo el modelo: {rows:?}"
    );

    app.set_provider_settings(saved_settings(None));
    let rows = rows_of(&app, 40, 20);
    assert!(
        !rows.iter().any(|row| row.contains("effort")),
        "sin esfuerzo no hay línea en el rail: {rows:?}"
    );
}
