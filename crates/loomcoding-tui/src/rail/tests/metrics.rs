//! Tests: metrics.

use super::support::*;
use super::*;

#[test]
fn format_tokens_scales() {
    assert_eq!(format_tokens(0), "0");
    assert_eq!(format_tokens(999), "999");
    assert_eq!(format_tokens(1_200), "1.2k");
    assert_eq!(format_tokens(1_999), "1.9k", "el redondeo es hacia abajo");
    assert_eq!(format_tokens(9_999), "9.9k");
    assert_eq!(format_tokens(15_000), "15k");
    assert_eq!(
        format_tokens(999_999),
        "999k",
        "el límite de k no se desborda"
    );
    assert_eq!(format_tokens(1_000_000), "1M");
    assert_eq!(format_tokens(1_500_000), "1.5M");
    assert_eq!(format_tokens(10_000_000), "10M");
    assert_eq!(format_tokens(2_000_000_000), "2G");
}

#[test]
fn format_elapsed_scales() {
    assert_eq!(format_elapsed(Duration::from_secs(0)), "0s");
    assert_eq!(format_elapsed(Duration::from_secs(7)), "7s");
    assert_eq!(format_elapsed(Duration::from_secs(59)), "59s");
    assert_eq!(format_elapsed(Duration::from_secs(65)), "1m05s");
    assert_eq!(format_elapsed(Duration::from_secs(3_599)), "59m59s");
    assert_eq!(format_elapsed(Duration::from_secs(3_600)), "1h00m");
    assert_eq!(format_elapsed(Duration::from_secs(3_700)), "1h01m");
}

#[test]
fn metrics_line_omits_missing_parts() {
    let start = Instant::now();
    let now = start + Duration::from_secs(7);

    let full = stats(2, 5, Some("read_file"), 1_200, 300, Some(start));
    assert_eq!(
        metrics_line(&full, None, 0, now, 80),
        "2/5 read_file · 1.2k/300 · 7s"
    );

    let sin_paso = stats(0, 0, Some("read_file"), 1_200, 300, Some(start));
    assert_eq!(
        metrics_line(&sin_paso, None, 0, now, 80),
        "read_file · 1.2k/300 · 7s"
    );

    let sin_herramienta = stats(2, 5, None, 1_200, 300, Some(start));
    assert_eq!(
        metrics_line(&sin_herramienta, None, 0, now, 80),
        "2/5 · 1.2k/300 · 7s"
    );

    let sin_arranque = stats(2, 5, None, 1_200, 300, None);
    assert_eq!(metrics_line(&sin_arranque, None, 0, now, 80), "2/5");

    let terminado = AgentStats {
        finished_at: Some(start + Duration::from_secs(3)),
        ..stats(2, 5, None, 0, 0, Some(start))
    };
    assert_eq!(metrics_line(&terminado, None, 0, now, 80), "2/5 · 0/0 · 3s");
}

#[test]
fn metrics_line_places_cost_between_tokens_and_elapsed() {
    let start = Instant::now();
    let now = start + Duration::from_secs(7);
    let full = stats(2, 5, Some("read_file"), 1_200, 300, Some(start));
    assert_eq!(
        metrics_line(&full, Some("≈$0.004".into()), 0, now, 80),
        "2/5 read_file · 1.2k/300 · ≈$0.004 · 7s"
    );

    // Sin arranque no salen tokens ni tiempo, así que tampoco el grupo de coste.
    let sin_arranque = stats(2, 5, None, 1_200, 300, None);
    assert_eq!(
        metrics_line(&sin_arranque, Some("≈$0.004".into()), 0, now, 80),
        "2/5"
    );
}

#[test]
fn metrics_line_places_file_count_between_cost_and_elapsed() {
    let start = Instant::now();
    let now = start + Duration::from_secs(7);
    let full = stats(2, 5, Some("read_file"), 1_200, 300, Some(start));
    assert_eq!(
        metrics_line(&full, Some("≈$0.004".into()), 3, now, 80),
        "2/5 read_file · 1.2k/300 · ≈$0.004 · 3f · 7s"
    );
    assert_eq!(
        metrics_line(&full, None, 3, now, 80),
        "2/5 read_file · 1.2k/300 · 3f · 7s"
    );

    // Sin arranque no hay grupo de ficheros, igual que con el coste.
    let sin_arranque = stats(2, 5, None, 1_200, 300, None);
    assert_eq!(metrics_line(&sin_arranque, None, 3, now, 80), "2/5");
}

#[test]
fn rail_shows_file_count_for_a_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.agent_stats
        .insert(9, stats(2, 5, None, 1_200, 300, Some(Instant::now())));
    app.files_by_session.insert(
        9,
        [
            ("src/a.rs".to_string(), (1, 0)),
            ("src/b.rs".to_string(), (2, 2)),
            ("src/c.rs".to_string(), (0, 4)),
        ]
        .into_iter()
        .collect(),
    );

    let rows = rows_of(&app, 40, 24);
    let idx_review = rows
        .iter()
        .position(|row| row.contains("review"))
        .expect("el rail lista el subagente review");
    assert!(
        rows[idx_review + 1].contains("· 3f"),
        "falta el recuento de ficheros bajo review: {:?}",
        rows[idx_review + 1]
    );
}

#[test]
fn rail_omits_file_count_without_files() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.agent_stats
        .insert(9, stats(2, 5, None, 1_200, 300, Some(Instant::now())));

    let rows = rows_of(&app, 40, 24);
    let idx_review = rows
        .iter()
        .position(|row| row.contains("review"))
        .expect("el rail lista el subagente review");
    assert!(
        !rows[idx_review + 1].contains('f'),
        "recuento de ficheros sin ficheros: {:?}",
        rows[idx_review + 1]
    );
}

#[test]
fn rail_child_metrics_show_cost_group_between_tokens_and_time() {
    let mut app = priced_rail_app("priced");
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.agent_stats.insert(
        9,
        stats(
            2,
            5,
            None,
            1_200,
            300,
            Some(Instant::now() - Duration::from_secs(7)),
        ),
    );
    app.usage_records = vec![UsageRecord {
        session: SessionId(9),
        ..record(1_000_000, 0, Some((3.0, 15.0)))
    }];

    let rows = rows_of(&app, 40, 24);
    let fila = rows
        .iter()
        .find(|row| row.contains("1.2k/300 ·"))
        .expect("el rail muestra las métricas del hijo");
    let tokens = fila.find("1.2k/300").expect("tokens");
    let coste = fila.find("≈$3.00").expect("el grupo de coste del hijo");
    let tiempo = fila.find("· 7s").expect("tiempo transcurrido");
    assert!(
        tokens < coste && coste < tiempo,
        "el coste va entre tokens y tiempo: {fila:?}"
    );
}

#[test]
fn rail_child_metrics_omit_cost_group_without_price() {
    let mut app = priced_rail_app("priced");
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.agent_stats.insert(
        9,
        stats(
            2,
            5,
            None,
            1_200,
            300,
            Some(Instant::now() - Duration::from_secs(7)),
        ),
    );
    app.usage_records = vec![UsageRecord {
        session: SessionId(9),
        ..record(1_000, 1_000, None)
    }];

    let rows = rows_of(&app, 40, 24);
    let fila = rows
        .iter()
        .find(|row| row.contains("1.2k/300 ·"))
        .expect("el rail muestra las métricas del hijo");
    assert!(
        !fila.contains('≈'),
        "sin precio no hay grupo de coste: {fila:?}"
    );
}

#[test]
fn metrics_line_truncates_to_width() {
    let start = Instant::now();
    let now = start + Duration::from_secs(7);
    let herramienta = "t".repeat(50);
    let largo = stats(2, 5, Some(&herramienta), 1_200, 300, Some(start));

    let linea = metrics_line(&largo, None, 0, now, 30);
    assert!(linea.chars().count() <= 30, "supera la anchura: {linea:?}");
    assert!(linea.ends_with('…'), "no acaba en elipsis: {linea:?}");
}

#[test]
fn rail_shows_metrics_second_line_for_a_subagent_with_stats() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    app.agent_stats.insert(
        9,
        stats(2, 5, Some("read_file"), 1_200, 300, Some(Instant::now())),
    );

    let rows = rows_of(&app, 40, 24);
    let idx_review = rows
        .iter()
        .position(|row| row.contains("review"))
        .expect("el rail lista el subagente review");
    let idx_tests = rows
        .iter()
        .position(|row| row.contains("tests"))
        .expect("el rail lista el subagente tests");
    assert!(
        rows[idx_review + 1].contains("2/5 read_file"),
        "falta la línea de métricas bajo review: {:?}",
        rows[idx_review + 1]
    );
    assert!(
        !rows[idx_tests + 1].contains('·'),
        "métricas bajo un subagente sin stats: {:?}",
        rows[idx_tests + 1]
    );
}

#[test]
fn finished_agent_takes_one_line_with_elapsed() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(
        9,
        SubagentDescriptor {
            status: TaskStatus::new(TaskState::Completed),
            ..running(9, "review")
        },
    );
    // El tiempo sale de `finished_at`, no de «ahora»: el hijo acabó 7 s después de arrancar.
    let inicio = Instant::now() - Duration::from_secs(60);
    app.agent_stats.insert(
        9,
        AgentStats {
            finished_at: Some(inicio + Duration::from_secs(7)),
            ..stats(2, 5, Some("read_file"), 1_200, 300, Some(inicio))
        },
    );

    let rows = rows_of(&app, 40, 24);
    let idx = rows
        .iter()
        .position(|row| row.contains("review"))
        .expect("el rail lista el subagente review");
    assert!(
        rows[idx].contains("7s"),
        "falta el tiempo transcurrido del hijo terminado: {:?}",
        rows[idx]
    );
    // La herramienta solo aparece en la línea de métricas; el título de Agents no la nombra.
    assert!(
        !rows.iter().any(|row| row.contains("read_file")),
        "un hijo terminado conserva su línea de métricas: {rows:?}"
    );
}

#[test]
fn running_agent_keeps_its_metrics_line() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.agent_stats.insert(
        9,
        stats(2, 5, Some("read_file"), 1_200, 300, Some(Instant::now())),
    );

    let rows = rows_of(&app, 40, 24);
    let idx = rows
        .iter()
        .position(|row| row.contains("review"))
        .expect("el rail lista el subagente review");
    assert!(
        rows[idx].contains("0s"),
        "falta el tiempo transcurrido del hijo en marcha: {:?}",
        rows[idx]
    );
    assert!(
        rows[idx + 1].contains("2/5 read_file"),
        "un hijo en marcha pierde su línea de métricas: {:?}",
        rows[idx + 1]
    );
}
