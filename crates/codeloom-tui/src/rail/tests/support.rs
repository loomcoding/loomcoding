//! Piezas comunes de los tests.

use super::*;

/// Subagente en marcha de la sesión 1 con el id interno `child`.
pub(super) fn running(child: i64, label: &str) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent: SessionId(1),
        child: SessionId(child),
        job_id: format!("subagent-{child}"),
        label: label.into(),
        status: TaskStatus::new(TaskState::Working),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

/// Filas del búfer de `width` columnas, una cadena por fila.
pub(super) fn rows_of(app: &App, width: u16, height: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| draw_context_rail(f, f.area(), app)).unwrap();
    let buffer = term.backend().buffer().clone();
    buffer
        .content
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

/// Estadísticas de un hijo con paso, herramienta y tokens; `started` fija el arranque.
pub(super) fn stats(
    step: usize,
    max_steps: usize,
    tool: Option<&str>,
    input: u64,
    output: u64,
    started: Option<Instant>,
) -> AgentStats {
    AgentStats {
        step,
        max_steps,
        tool: tool.map(str::to_owned),
        input_tokens: input,
        output_tokens: output,
        started_at: started,
        finished_at: None,
    }
}

/// Columna (en caracteres) donde empieza `texto` dentro de `fila`.
pub(super) fn columna(fila: &str, texto: &str) -> usize {
    let byte = fila.find(texto).expect("el texto debe estar en la fila");
    fila[..byte].chars().count()
}

/// Ajustes guardados con el modelo `claude-think` y el esfuerzo `effort`.
pub(super) fn saved_settings(effort: Option<&str>) -> crate::app::ProviderSettings {
    crate::app::ProviderSettings {
        kind: codeloom_core::config::ProviderKind::Anthropic,
        model: "claude-think".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: effort.map(String::from),
    }
}

/// Apunte con precios `(entrada, salida)` por millón, o sin precio si `prices` es `None`.
pub(super) fn record(input: u64, output: u64, prices: Option<(f64, f64)>) -> UsageRecord {
    UsageRecord {
        session: SessionId(1),
        model: "priced".into(),
        input_tokens: input,
        output_tokens: output,
        input_price: prices.map(|(price_in, _)| price_in),
        output_price: prices.map(|(_, price_out)| price_out),
        reported_cost_usd: None,
        source: codeloom_core::message::UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: 1,
    }
}

/// Ajustes de `acme` con el modelo dado, con el catálogo de precios cargado.
pub(super) fn priced_rail_app(model: &str) -> App {
    let mut app = App::new();
    app.set_catalog(crate::app::priced_sample_catalog());
    app.set_provider_settings(crate::app::ProviderSettings {
        kind: codeloom_core::config::ProviderKind::OpenAiCompat,
        model: model.into(),
        base_url: "https://acme.example/v1".into(),
        api_key_env: "ACME_KEY".into(),
        effort: None,
    });
    app
}
