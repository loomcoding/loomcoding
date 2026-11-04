//! Tests: cost.

use super::support::*;
use super::*;

#[test]
fn format_cost_ranges() {
    assert_eq!(format_cost(0.00005, false), "≈ < $0.0001");
    assert_eq!(format_cost(0.0001, false), "≈ $0.0001");
    assert_eq!(format_cost(0.0123, false), "≈ $0.0123");
    assert_eq!(format_cost(1.234, false), "≈ $1.23");
    assert_eq!(format_cost(123.4, false), "≈ $123");
}

#[test]
fn reported_cost_is_displayed_without_estimate_marker() {
    assert_eq!(format_actual_cost(0.0123, false), "$0.0123");
}

#[test]
fn format_cost_rounding_edges_and_zero() {
    // El redondeo no debe producir «1.0000» ni «100.00».
    assert_eq!(format_cost(0.99996, false), "≈ $1.00");
    assert_eq!(format_cost(99.996, false), "≈ $100");
    assert_eq!(format_cost(0.0, false), "≈ $0");
}

#[test]
fn format_cost_partial_adds_plus() {
    assert_eq!(format_cost(0.0123, true), "≈ $0.0123+");
    assert_eq!(format_cost(0.0, true), "≈ $0+");
}

#[test]
fn child_cost_label_marks_estimates_but_not_reported_costs() {
    assert_eq!(
        child_cost_label(Some((0.01, false)), true).as_deref(),
        Some("≈$0.0100")
    );
    assert_eq!(
        child_cost_label(Some((0.01, false)), false).as_deref(),
        Some("$0.0100")
    );
}

#[test]
fn format_prices_shapes() {
    assert_eq!(format_prices(3.0, 15.0), "$3/$15 /M");
    assert_eq!(format_prices(0.3, 2.5), "$0.3/$2.5 /M");
    assert_eq!(format_prices(0.25, 1.0), "$0.25/$1 /M");
}

#[test]
fn rail_shows_session_total_and_model_prices() {
    let mut app = priced_rail_app("priced");
    app.usage_records = vec![
        record(1_000_000, 0, Some((3.0, 15.0))),
        record(0, 100_000, Some((3.0, 15.0))),
    ];
    let rows = rows_of(&app, 40, 24);
    // Entrada: 3,00. Salida: 100.000 a 15 $/M = 1,50. Total: 4,50.
    assert!(
        rows.iter().any(|row| row.contains("≈ $4.50")),
        "el rail muestra el total de la sesión: {rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("$3/$15 /M")),
        "el rail muestra el precio por millón del modelo: {rows:?}"
    );
}

#[test]
fn rail_hides_cost_line_when_no_record_has_a_price() {
    use codeloom_core::session::UsageRecord;
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.usage_records.push(UsageRecord {
        session: SessionId(1),
        model: "m".into(),
        input_tokens: 10,
        output_tokens: 5,
        input_price: None,
        output_price: None,
        reported_cost_usd: None,
        source: codeloom_core::message::UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: 0,
    });
    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains('≈')),
        "sale un coste sin precio alguno: {rows:?}"
    );
}

#[test]
fn rail_hides_prices_line_without_catalog() {
    let mut app = App::new();
    app.usage_records = vec![record(1_000, 1_000, None)];
    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains("/M")),
        "sin catálogo no hay línea de precios: {rows:?}"
    );
}

#[test]
fn rail_shows_cost_line_when_priced() {
    let mut app = priced_rail_app("priced");
    app.input_tokens = 1_000_000;
    app.usage_records = vec![record(1_000_000, 0, Some((3.0, 15.0)))];

    let rows = rows_of(&app, 40, 24);
    let idx = rows
        .iter()
        .position(|row| row.contains("↑1000000"))
        .expect("el rail muestra la línea de tokens");
    assert!(
        rows[idx + 1].contains("≈ $3.00"),
        "falta el coste justo bajo los tokens: {:?}",
        rows[idx + 1]
    );
}

#[test]
fn rail_has_no_cost_line_without_prices() {
    let mut app = App::new();
    app.input_tokens = 1_000_000;
    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains('≈')),
        "línea de coste sin precio: {rows:?}"
    );

    // Con catálogo, pero el modelo no tiene precio completo: tampoco.
    let mut app = priced_rail_app("plain");
    app.input_tokens = 1_000_000;
    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains('≈')),
        "línea de coste sin precio del modelo: {rows:?}"
    );
}

#[test]
fn rail_clips_huge_cost_to_width() {
    // Tres mil millones de dólares: la cifra no debe desbordar un rail de 11 columnas.
    let mut app = priced_rail_app("priced");
    app.usage_records = vec![record(1_000_000_000_000_000, 0, Some((3.0, 15.0)))];

    let rows = rows_of(&app, 12, 24);
    let fila = rows
        .iter()
        .find(|row| row.contains('≈'))
        .expect("la línea de coste aparece también en un rail estrecho");
    assert_eq!(fila.chars().count(), 12, "la fila ocupa el ancho del rail");
    assert!(fila.contains('…'), "la línea no se recorta: {fila:?}");
}
