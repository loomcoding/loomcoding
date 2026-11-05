//! Tarjeta de agentes del layout `Cards`: ausencia fuera de él, columnas alineadas, tope de
//! filas, plegado con `ctrl+b` y reparto del alto con el transcript.

use super::*;
use crate::app::AgentStats;
use crate::render::agents_card::card_rows;
use loomcoding_cloth::widgets::LayoutStyle;
use loomcoding_core::session::{SessionId, SubagentDescriptor};
use std::time::{Duration, Instant};

/// Subagente de prueba de la sesión 1, con su etiqueta y su estado A2A.
fn agente(hijo: i64, etiqueta: &str, estado: TaskState) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent: SessionId(1),
        child: SessionId(hijo),
        job_id: format!("subagent-{hijo}"),
        label: etiqueta.into(),
        status: TaskStatus::new(estado),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: Some("default".into()),
        agent_origin: Some("built-in".into()),
        reference_task_ids: Vec::new(),
    }
}

/// Aplicación con el layout `Cards` y los subagentes indicados en la sesión 1.
fn app_cards(agentes: &[(i64, &str, TaskState)]) -> App {
    let mut app = App::with_context("/proj", "mistral-nemo");
    app.theme.look.layout = LayoutStyle::Cards;
    app.session_id = Some(SessionId(1));
    for &(hijo, etiqueta, estado) in agentes {
        app.subagents.insert(hijo, agente(hijo, etiqueta, estado));
    }
    app
}

/// Métricas de prueba: tokens de entrada y una duración exacta de `segundos`.
fn metricas(app: &mut App, hijo: i64, tokens: u64, segundos: u64) {
    let inicio = Instant::now();
    app.agent_stats.insert(
        hijo,
        AgentStats {
            input_tokens: tokens,
            started_at: Some(inicio),
            finished_at: Some(inicio + Duration::from_secs(segundos)),
            ..AgentStats::default()
        },
    );
}

/// Filas del buffer de `term`, una a una, como texto.
fn filas(term: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = term.backend().buffer();
    let (ancho, alto) = (buffer.area.width, buffer.area.height);
    (0..alto)
        .map(|y| {
            (0..ancho)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

/// Columna (en caracteres) donde empieza `aguja` dentro de `fila`.
fn columna(fila: &str, aguja: &str) -> Option<usize> {
    fila.find(aguja).map(|byte| fila[..byte].chars().count())
}

#[test]
fn card_is_absent_outside_cards() {
    let mut app = app_cards(&[(11, "constructor", TaskState::Working)]);
    // Positivo: en `Cards` la tarjeta existe, así que la ausencia de abajo significa algo.
    assert!(card_rows(&app, 40) > 0);
    assert!(buffer_of(&mut app, 120, 40).contains("❀ Agentes"));
    for layout in [LayoutStyle::ContextRail, LayoutStyle::Minimal] {
        app.theme.look.layout = layout;
        assert_eq!(card_rows(&app, 40), 0, "{layout:?} dibuja la tarjeta");
        assert!(
            !buffer_of(&mut app, 120, 40).contains('❀'),
            "{layout:?} pinta el glifo de la tarjeta"
        );
    }
}

#[test]
fn card_is_absent_without_subagents() {
    let mut app = app_cards(&[(11, "constructor", TaskState::Working)]);
    assert!(card_rows(&app, 40) > 0);
    assert!(buffer_of(&mut app, 120, 40).contains('❀'));
    app.subagents.clear();
    assert_eq!(card_rows(&app, 40), 0);
    assert!(!buffer_of(&mut app, 120, 40).contains('❀'));
}

#[test]
fn card_columns_are_aligned() {
    let mut app = app_cards(&[
        (11, "constructor", TaskState::Working),
        (12, "explorer", TaskState::Completed),
        (13, "critico", TaskState::Submitted),
    ]);
    metricas(&mut app, 11, 1_200, 192);
    metricas(&mut app, 12, 950, 40);
    metricas(&mut app, 13, 12_000, 65);
    let term = terminal_of(&mut app, 100, 40);
    let filas = filas(&term);
    let buscar = |etiqueta: &str| {
        filas
            .iter()
            .find(|fila| fila.contains(etiqueta))
            .unwrap_or_else(|| panic!("falta la fila de {etiqueta}"))
            .clone()
    };
    let (constructor, explorer, critico) =
        (buscar("constructor"), buscar("explorer"), buscar("critico"));
    let previstas = [
        (&constructor, "1.2k", "3m12s"),
        (&explorer, "950", "40s"),
        (&critico, "12k", "1m05s"),
    ];
    let tokens: Vec<_> = previstas.iter().map(|(f, t, _)| columna(f, t)).collect();
    let tiempos: Vec<_> = previstas.iter().map(|(f, _, s)| columna(f, s)).collect();
    assert!(tokens.iter().all(Option::is_some), "tokens: {tokens:?}");
    assert!(tiempos.iter().all(Option::is_some), "tiempos: {tiempos:?}");
    assert!(
        tokens.windows(2).all(|par| par[0] == par[1]),
        "la columna de tokens no empieza igual: {tokens:?}"
    );
    assert!(
        tiempos.windows(2).all(|par| par[0] == par[1]),
        "la columna de tiempo no empieza igual: {tiempos:?}"
    );
}

#[test]
fn card_caps_rows_and_counts_the_rest() {
    let nombres: Vec<String> = (0..12).map(|i| format!("tarea-{i:02}")).collect();
    let agentes: Vec<(i64, &str, TaskState)> = nombres
        .iter()
        .enumerate()
        .map(|(i, nombre)| (100 + i as i64, nombre.as_str(), TaskState::Working))
        .collect();
    let mut app = app_cards(&agentes);
    // Alto grande: el tope es 8; la cabecera y la fila de resto suman dos más.
    assert_eq!(card_rows(&app, 60), 1 + 8 + 1);
    let term = terminal_of(&mut app, 100, 60);
    let filas = filas(&term);
    assert_eq!(filas.iter().filter(|f| f.contains("tarea-")).count(), 8);
    assert!(filas.iter().any(|f| f.contains("… 4 more")));
    // Alto pequeño: el tope es el 25 % del área, con mínimo de una fila.
    assert_eq!(card_rows(&app, 20), 1 + 5 + 1);
    assert_eq!(card_rows(&app, 4), 1 + 1 + 1);
}

#[test]
fn folded_card_takes_one_row() {
    let mut app = app_cards(&[
        (11, "revisor", TaskState::Working),
        (12, "pruebas", TaskState::Completed),
    ]);
    assert!(buffer_of(&mut app, 100, 40).contains("▾ plegar"));
    app.agents_card_folded = true;
    assert_eq!(card_rows(&app, 40), 1);
    let term = terminal_of(&mut app, 100, 40);
    let filas = filas(&term);
    assert!(filas.iter().any(|f| f.contains("▸ desplegar")));
    assert!(!filas
        .iter()
        .any(|f| f.contains("revisor") || f.contains("pruebas")));
}

#[test]
fn ctrl_b_folds_the_card_in_cards_layout() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut app = app_cards(&[(11, "revisor", TaskState::Working)]);
    let expandido = app.sidebar_expanded;
    crate::run::handle_key(&mut app, KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert!(app.agents_card_folded);
    assert_eq!(app.sidebar_expanded, expandido);
    assert!(!app.rail_hidden);
    crate::run::handle_key(&mut app, KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert!(!app.agents_card_folded);
    assert_eq!(app.sidebar_expanded, expandido);
    // Fuera de `Cards`, ctrl+b sigue alternando el rail.
    app.theme.look.layout = LayoutStyle::ContextRail;
    app.toggle_sidebar();
    assert!(app.rail_hidden);
    assert!(!app.agents_card_folded);
}

#[test]
fn the_transcript_keeps_three_rows_with_the_card() {
    let nombres: Vec<String> = (0..8).map(|i| format!("agente-{i}")).collect();
    let agentes: Vec<(i64, &str, TaskState)> = nombres
        .iter()
        .enumerate()
        .map(|(i, nombre)| (100 + i as i64, nombre.as_str(), TaskState::Working))
        .collect();
    let mut app = app_cards(&agentes);
    let (mut con_tarjeta, mut sin_tarjeta) = (false, false);
    for alto in 1..=24 {
        let term = terminal_of(&mut app, 100, alto);
        if filas(&term).iter().any(|f| f.contains('❀')) {
            con_tarjeta = true;
            let area = app.transcript_area.expect("el transcript fija su área");
            assert!(
                area.height >= 3,
                "alto {alto}: el transcript se queda con {} filas",
                area.height
            );
        } else {
            sin_tarjeta = true;
        }
    }
    assert!(con_tarjeta, "con 24 filas debía caber la tarjeta");
    assert!(sin_tarjeta, "con pocas filas la tarjeta no debía dibujarse");
}
