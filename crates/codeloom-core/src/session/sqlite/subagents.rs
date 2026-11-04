//! Catálogo de subagentes: estado, resultado, progreso y avisos.

use super::codec::*;
use super::SqliteStore;
use crate::a2a::Artifact;
use crate::error::{CoreError, Result};
use crate::message::{self, Message};
use crate::session::{SessionId, SubagentDescriptor};
use crate::subagent_phase::SubagentPhase;
use crate::task::{TaskState, TaskStatus};
use sqlx::Row;

impl SqliteStore {
    pub(super) async fn sql_save_subagent(&self, descriptor: &SubagentDescriptor) -> Result<()> {
        sqlx::query(
            "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, status_message, status_timestamp_ms, artifacts_json, agent, agent_origin, reference_task_ids, phase, last_activity_at_ms, steps, tool_calls, tokens_in, tokens_out, created_at_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(descriptor.parent.0)
        .bind(descriptor.child.0)
        .bind(&descriptor.job_id)
        .bind(&descriptor.label)
        .bind(descriptor.status.state.proto_name())
        .bind(encode_status_message(descriptor.status.message.as_ref())?)
        .bind(descriptor.status.timestamp_ms)
        .bind(encode_artifacts(&descriptor.artifacts)?)
        .bind(&descriptor.agent)
        .bind(&descriptor.agent_origin)
        .bind(encode_references(&descriptor.reference_task_ids)?)
        .bind(descriptor.phase.as_str())
        .bind(descriptor.last_activity_at_ms as i64)
        .bind(descriptor.steps as i64)
        .bind(descriptor.tool_calls as i64)
        .bind(descriptor.tokens_in as i64)
        .bind(descriptor.tokens_out as i64)
        .bind(crate::message::now_ms())
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn sql_update_subagent_status(
        &self,
        parent: SessionId,
        child: SessionId,
        status: TaskStatus,
    ) -> Result<()> {
        // Leer y escribir en una misma transacción: un terminal no se reabre ni aunque
        // dos escrituras coincidan.
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let actual: Option<String> = sqlx::query_scalar(
            "SELECT status FROM subagents WHERE parent_session_id = ? AND child_session_id = ?",
        )
        .bind(parent.0)
        .bind(child.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        if let Some(actual) = actual {
            let actual = parse_task_state(&actual)?;
            if actual.is_terminal() {
                return Err(CoreError::Store(format!(
                    "subagent already settled as {}",
                    actual.proto_name()
                )));
            }
        }
        sqlx::query(
            "UPDATE subagents SET status = ?, status_message = ?, status_timestamp_ms = ?
             WHERE parent_session_id = ? AND child_session_id = ?",
        )
        .bind(status.state.proto_name())
        .bind(encode_status_message(status.message.as_ref())?)
        .bind(status.timestamp_ms)
        .bind(parent.0)
        .bind(child.0)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn sql_update_subagent_runtime(
        &self,
        parent: SessionId,
        child: SessionId,
        phase: SubagentPhase,
        last_activity_at_ms: u64,
    ) -> Result<()> {
        // Un hijo ya asentado conserva la fase con que terminó: la guarda la propia condición.
        sqlx::query(
            "UPDATE subagents SET phase = ?, last_activity_at_ms = ?
             WHERE parent_session_id = ? AND child_session_id = ?
               AND status NOT IN ('TASK_STATE_COMPLETED', 'TASK_STATE_FAILED',
                                  'TASK_STATE_CANCELED', 'TASK_STATE_REJECTED')",
        )
        .bind(phase.as_str())
        .bind(last_activity_at_ms as i64)
        .bind(parent.0)
        .bind(child.0)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn sql_list_subagents(
        &self,
        parent: SessionId,
    ) -> Result<Vec<SubagentDescriptor>> {
        let rows = sqlx::query(
            "SELECT parent_session_id, child_session_id, job_id, label, status, status_message,
                    status_timestamp_ms, created_at_ms, artifacts_json, result, notice_delivered,
                    agent, agent_origin, reference_task_ids, phase, last_activity_at_ms,
                    steps, tool_calls, tokens_in, tokens_out
             FROM subagents WHERE parent_session_id = ? ORDER BY created_at_ms, child_session_id",
        )
        .bind(parent.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        rows.into_iter()
            .map(|row| {
                let status: String = row.get("status");
                let child: i64 = row.get("child_session_id");
                // Una fila antigua no tiene marca de tiempo propia: se usa la de su creación.
                let timestamp_ms = row
                    .get::<Option<i64>, _>("status_timestamp_ms")
                    .unwrap_or_else(|| row.get("created_at_ms"));
                Ok(SubagentDescriptor {
                    parent: SessionId(row.get("parent_session_id")),
                    child: SessionId(child),
                    job_id: row.get("job_id"),
                    label: row.get("label"),
                    status: TaskStatus {
                        state: parse_task_state(&status)?,
                        message: decode_status_message(row.get("status_message"), child),
                        timestamp_ms,
                    },
                    // Una fila antigua no tiene fase: sale `Queued` sin error. Un texto
                    // de una versión más nueva cae en `Unknown`, tampoco con error.
                    phase: row
                        .get::<Option<String>, _>("phase")
                        .map(|texto| SubagentPhase::from_str(&texto))
                        .unwrap_or(SubagentPhase::Queued),
                    last_activity_at_ms: row
                        .get::<Option<i64>, _>("last_activity_at_ms")
                        .unwrap_or(0)
                        .max(0) as u64,
                    steps: row.get::<Option<i64>, _>("steps").unwrap_or(0).max(0) as u64,
                    tool_calls: row.get::<Option<i64>, _>("tool_calls").unwrap_or(0).max(0) as u64,
                    tokens_in: row.get::<Option<i64>, _>("tokens_in").unwrap_or(0).max(0) as u64,
                    tokens_out: row.get::<Option<i64>, _>("tokens_out").unwrap_or(0).max(0) as u64,
                    artifacts: decode_artifacts(
                        row.get("artifacts_json"),
                        row.get("result"),
                        child,
                    )?,
                    notice_delivered: row.get::<i64, _>("notice_delivered") != 0,
                    agent: row.get("agent"),
                    agent_origin: row.get("agent_origin"),
                    reference_task_ids: decode_references(row.get("reference_task_ids"), child),
                })
            })
            .collect()
    }

    pub(super) async fn sql_complete_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
        artifacts: &[Artifact],
    ) -> Result<()> {
        // Igual que `update_subagent_status`: un terminal no admite otra transición, tampoco esta.
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let fila = sqlx::query(
            "SELECT status, artifacts_json, result FROM subagents WHERE parent_session_id = ? AND child_session_id = ?",
        )
        .bind(parent.0)
        .bind(child.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        // Los artefactos de progreso que el hijo publicó en marcha se conservan; el resultado se añade.
        let mut conservados = Vec::new();
        if let Some(fila) = fila {
            let actual = parse_task_state(&fila.get::<String, _>("status"))?;
            if actual.is_terminal() {
                return Err(CoreError::Store(format!(
                    "subagent already settled as {}",
                    actual.proto_name()
                )));
            }
            conservados =
                decode_artifacts(fila.get("artifacts_json"), fila.get("result"), child.0)?
                    .into_iter()
                    .filter(Artifact::is_progress)
                    .collect();
        }
        conservados.extend(artifacts.iter().cloned());
        let artifacts = &conservados[..];
        sqlx::query(
            "UPDATE subagents SET status = ?, status_message = NULL, status_timestamp_ms = ?, artifacts_json = ?
             WHERE parent_session_id = ? AND child_session_id = ?",
        )
        .bind(TaskState::Completed.proto_name())
        .bind(message::now_ms())
        .bind(encode_artifacts(artifacts)?)
        .bind(parent.0)
        .bind(child.0)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn sql_publish_subagent_progress(
        &self,
        child: SessionId,
        name: &str,
        description: Option<&str>,
        content: &str,
    ) -> Result<SubagentDescriptor> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let fila = sqlx::query(
            "SELECT parent_session_id, status, artifacts_json, result FROM subagents WHERE child_session_id = ?",
        )
        .bind(child.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?
        .ok_or_else(|| CoreError::Tool("this session is not a subagent task".into()))?;
        if parse_task_state(&fila.get::<String, _>("status"))?.is_terminal() {
            return Err(CoreError::Tool("subagent has already finished".into()));
        }
        let parent = SessionId(fila.get("parent_session_id"));
        let actuales = decode_artifacts(fila.get("artifacts_json"), fila.get("result"), child.0)?;
        let nuevos = crate::a2a::publicar_progreso(&actuales, name, description, content)
            .map_err(CoreError::Tool)?;
        sqlx::query("UPDATE subagents SET artifacts_json = ? WHERE child_session_id = ?")
            .bind(encode_artifacts(&nuevos)?)
            .bind(child.0)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        self.sql_list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or_else(|| CoreError::Store("subagent vanished".into()))
    }

    pub(super) async fn sql_append_subagent_notice(
        &self,
        parent: SessionId,
        child: SessionId,
        message: &Message,
    ) -> Result<bool> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let pending = sqlx::query_scalar::<_, i64>(
            "SELECT notice_delivered FROM subagents
             WHERE parent_session_id = ? AND child_session_id = ? AND status IN ('TASK_STATE_COMPLETED', 'completed')",
        )
        .bind(parent.0)
        .bind(child.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        if pending != Some(0) {
            tx.rollback()
                .await
                .map_err(|e| CoreError::Store(e.to_string()))?;
            return Ok(false);
        }
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM messages WHERE session_id = ?",
        )
        .bind(parent.0)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        sqlx::query("INSERT INTO messages (session_id, payload, seq) VALUES (?, ?, ?)")
            .bind(parent.0)
            .bind(message::codec::to_json(message)?)
            .bind(next)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        sqlx::query("UPDATE subagents SET notice_delivered = 1 WHERE parent_session_id = ? AND child_session_id = ?")
            .bind(parent.0)
            .bind(child.0)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(true)
    }
}
