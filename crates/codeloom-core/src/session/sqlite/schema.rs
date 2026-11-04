//! Esquema de la base y sus migraciones: se aplican, idempotentes, al abrir el almacén.

use super::codec::*;
use crate::a2a::{self};
use crate::error::{CoreError, Result};
use crate::message::{self};
use crate::task::TaskState;
use sqlx::SqlitePool;

/// Mensaje con el que queda una tarea que seguía en marcha cuando la aplicación se cerró.
const INTERRUPTED_AT_SHUTDOWN: &str = "interrupted by application shutdown";

pub(super) async fn migrate(pool: &SqlitePool) -> Result<()> {
    let pool = pool.clone();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                payload TEXT NOT NULL,
                seq INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS subagents (
                parent_session_id INTEGER NOT NULL,
                child_session_id INTEGER NOT NULL PRIMARY KEY,
                job_id TEXT NOT NULL,
                label TEXT NOT NULL,
                status TEXT NOT NULL,
                status_message TEXT,
                status_timestamp_ms INTEGER,
                result TEXT,
                notice_delivered INTEGER NOT NULL DEFAULT 0,
                created_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    // Una base anterior a A2A no tiene las columnas de estado: se añaden solo si faltan,
    // así que abrirla dos veces seguidas no falla.
    let columnas: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('subagents')")
            .fetch_all(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
    for (columna, ddl) in [
        (
            "status_message",
            "ALTER TABLE subagents ADD COLUMN status_message TEXT",
        ),
        (
            "status_timestamp_ms",
            "ALTER TABLE subagents ADD COLUMN status_timestamp_ms INTEGER",
        ),
        (
            "artifacts_json",
            "ALTER TABLE subagents ADD COLUMN artifacts_json TEXT",
        ),
        ("agent", "ALTER TABLE subagents ADD COLUMN agent TEXT"),
        (
            "agent_origin",
            "ALTER TABLE subagents ADD COLUMN agent_origin TEXT",
        ),
        (
            "reference_task_ids",
            "ALTER TABLE subagents ADD COLUMN reference_task_ids TEXT",
        ),
        (
            "phase",
            "ALTER TABLE subagents ADD COLUMN phase TEXT NOT NULL DEFAULT 'queued'",
        ),
        (
            "last_activity_at_ms",
            "ALTER TABLE subagents ADD COLUMN last_activity_at_ms INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "steps",
            "ALTER TABLE subagents ADD COLUMN steps INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "tool_calls",
            "ALTER TABLE subagents ADD COLUMN tool_calls INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "tokens_in",
            "ALTER TABLE subagents ADD COLUMN tokens_in INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "tokens_out",
            "ALTER TABLE subagents ADD COLUMN tokens_out INTEGER NOT NULL DEFAULT 0",
        ),
    ] {
        if !columnas.iter().any(|existente| existente == columna) {
            sqlx::query(ddl)
                .execute(&pool)
                .await
                .map_err(|e| CoreError::Store(e.to_string()))?;
        }
    }
    // Libro de consumo. Una base anterior no la tiene: `IF NOT EXISTS` la añade sin tocar el resto.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                input_price REAL,
                output_price REAL,
                created_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    let usage_columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('usage')")
            .fetch_all(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
    if !usage_columns
        .iter()
        .any(|column| column == "reported_cost_usd")
    {
        sqlx::query("ALTER TABLE usage ADD COLUMN reported_cost_usd REAL")
            .execute(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
    }
    if !usage_columns.iter().any(|column| column == "source") {
        sqlx::query("ALTER TABLE usage ADD COLUMN source TEXT NOT NULL DEFAULT 'provider'")
            .execute(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
    }
    if !usage_columns.iter().any(|column| column == "is_final") {
        sqlx::query("ALTER TABLE usage ADD COLUMN is_final INTEGER NOT NULL DEFAULT 1")
            .execute(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
    }
    if !usage_columns
        .iter()
        .any(|column| column == "components_json")
    {
        sqlx::query("ALTER TABLE usage ADD COLUMN components_json TEXT NOT NULL DEFAULT '[]'")
            .execute(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
    }
    // Tokens de caché del proveedor; cero significa «no informado o sin caché».
    for column in ["cache_read_tokens", "cache_write_tokens"] {
        if !usage_columns.iter().any(|existing| existing == column) {
            sqlx::query(&format!(
                "ALTER TABLE usage ADD COLUMN {column} INTEGER NOT NULL DEFAULT 0"
            ))
            .execute(&pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        }
    }
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS development_workflows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                parent_session_id INTEGER NOT NULL,
                phase TEXT NOT NULL,
                version INTEGER NOT NULL,
                payload TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS one_active_development_workflow_per_session
             ON development_workflows(parent_session_id)
             WHERE phase NOT IN ('completed', 'cancelled', 'failed')",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS development_workflow_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workflow_id INTEGER NOT NULL,
                from_phase TEXT,
                to_phase TEXT NOT NULL,
                actor TEXT NOT NULL,
                detail TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    // shortcut: one process per DB; use leases if concurrent Loom processes share it.
    // Lo no terminado al reabrir (y la cadena antigua «running») pasa a fallido, con un
    // mensaje que explica por qué: A2A no define qué le corresponde a un proceso que desapareció.
    sqlx::query(
        "UPDATE subagents SET status = ?, status_message = ?, status_timestamp_ms = ?
             WHERE status IN (?, ?, ?, ?, 'running')",
    )
    .bind(TaskState::Failed.proto_name())
    .bind(encode_status_message(Some(&a2a::Message::agent_text(
        INTERRUPTED_AT_SHUTDOWN,
    )))?)
    .bind(message::now_ms())
    .bind(TaskState::Submitted.proto_name())
    .bind(TaskState::Working.proto_name())
    .bind(TaskState::InputRequired.proto_name())
    .bind(TaskState::AuthRequired.proto_name())
    .execute(&pool)
    .await
    .map_err(|e| CoreError::Store(e.to_string()))?;
    Ok(())
}
