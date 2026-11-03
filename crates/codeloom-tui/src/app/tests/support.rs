use super::*;

/// App con el look Crush (herramientas inline y composer sin caja).
pub(super) fn crush_app() -> App {
    let mut app = App::new();
    app.theme_preset = batten::widgets::ThemePreset::Crush;
    app.theme = batten::widgets::Theme::preset(batten::widgets::ThemePreset::Crush);
    app
}

/// Job de prueba con etiqueta `build`, sin progreso ni resultado.
pub(super) fn job(id: &str, status: JobStatus, owner: SessionId) -> JobSnapshot {
    JobSnapshot {
        id: JobId(id.into()),
        kind: "shell".into(),
        label: "build".into(),
        owner,
        status,
        progress: None,
        output: String::new(),
        output_start: 0,
        output_end: 0,
        detail: None,
        result: None,
    }
}

/// Subagente de prueba (hijo `SessionId(9)`).
pub(super) fn sub(label: &str, state: TaskState, parent: SessionId) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent,
        child: SessionId(9),
        job_id: "subagent-1".into(),
        label: label.into(),
        status: TaskStatus::new(state),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

pub(super) fn texts(app: &App) -> Vec<String> {
    app.transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect()
}

/// Cabeceras de actividad con su texto y su estado: las de los jobs (`ActivityHead`) y
/// las de los nodos de subagente (`NodeHead`).
pub(super) fn heads(app: &App) -> Vec<(String, ToolStatus)> {
    app.transcript
        .iter()
        .filter_map(|l| match l.kind {
            batten::state::LineKind::ActivityHead(status)
            | batten::state::LineKind::NodeHead(status) => Some((l.plain().to_string(), status)),
            _ => None,
        })
        .collect()
}

pub(super) fn has_user_line(app: &App) -> bool {
    app.transcript
        .iter()
        .any(|l| l.kind == batten::state::LineKind::User)
}

/// Pregunta de `ask_user` con opciones `labels`.
pub(super) fn ask_question(text: &str, labels: &[&str], multi_select: bool) -> Question {
    Question {
        header: "Stack".into(),
        question: text.into(),
        options: labels
            .iter()
            .map(|label| QuestionOption {
                label: (*label).into(),
                description: String::new(),
            })
            .collect(),
        multi_select,
    }
}

/// App en reposo con la sesión 1 abierta, que es el padre de los hijos de los tests.
pub(super) fn idle_parent_app() -> App {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app
}

/// Lista de textos de las líneas de sistema del transcript.
pub(super) fn system_lines(app: &App) -> Vec<String> {
    app.transcript
        .iter()
        .filter(|l| l.kind == batten::state::LineKind::System)
        .map(|l| l.plain().to_string())
        .collect()
}

/// Evento de un hijo tal como lo reenvía el lanzador al bus del padre.
pub(super) fn child_event(parent: SessionId, child: i64, event: LiveEvent) -> LiveEvent {
    LiveEvent::ChildEvent {
        parent,
        child: SessionId(child),
        event: Box::new(event),
    }
}

/// Descriptor de un hijo con identificador y estado concretos.
pub(super) fn child_descriptor(
    child: i64,
    state: TaskState,
    parent: SessionId,
) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent,
        child: SessionId(child),
        job_id: format!("subagent-{child}"),
        label: format!("hijo-{child}"),
        status: TaskStatus::new(state),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

/// App con el catálogo de precios y unos ajustes que apuntan a `acme` con el modelo dado.
pub(super) fn priced_app(model: &str, base_url: &str) -> App {
    let mut app = App::new();
    app.set_catalog(priced_sample_catalog());
    app.set_provider_settings(ProviderSettings {
        kind: codeloom_core::config::ProviderKind::OpenAiCompat,
        model: model.into(),
        base_url: base_url.into(),
        api_key_env: "ACME_KEY".into(),
        effort: None,
    });
    app
}

pub(super) fn largo(n: usize) -> String {
    (1..=n)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n")
}
