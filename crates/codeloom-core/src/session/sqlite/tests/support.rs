//! Piezas comunes de los tests del almacén.

use super::*;

/// Descriptor de prueba del subagente `child` con el estado A2A dado.
pub(super) fn descriptor(
    parent: SessionId,
    child: SessionId,
    state: TaskState,
) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent,
        child,
        job_id: format!("subagent-{}", child.0),
        label: "review".into(),
        status: TaskStatus::new(state),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

/// Estado guardado tal cual en la columna `status` de la fila del hijo.
pub(super) async fn raw_status(store: &SqliteStore, child: SessionId) -> String {
    sqlx::query_scalar("SELECT status FROM subagents WHERE child_session_id = ?")
        .bind(child.0)
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

/// Apunte de prueba con precios opcionales `(entrada, salida)` por millón. La sesión es
/// provisional: el almacén devuelve la sesión de la fila con la que se guardó.
pub(super) fn usage(
    model: &str,
    input: u64,
    output: u64,
    prices: Option<(f64, f64)>,
) -> UsageRecord {
    UsageRecord {
        session: SessionId(0),
        model: model.into(),
        input_tokens: input,
        output_tokens: output,
        input_price: prices.map(|(input, _)| input),
        output_price: prices.map(|(_, output)| output),
        reported_cost_usd: None,
        source: crate::message::UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: crate::message::now_ms(),
    }
}

/// Inserta una fila con el esquema de columnas que escribía la versión anterior de `status`.
pub(super) async fn fila_antigua(
    store: &SqliteStore,
    parent: SessionId,
    child: SessionId,
    status: &str,
    status_message: Option<&str>,
    result: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, status_message, status_timestamp_ms, result, created_at_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(parent.0)
    .bind(child.0)
    .bind(format!("subagent-{}", child.0))
    .bind("review")
    .bind(status)
    .bind(status_message)
    .bind(crate::message::now_ms())
    .bind(result)
    .bind(crate::message::now_ms())
    .execute(&store.pool)
    .await
    .unwrap();
}
