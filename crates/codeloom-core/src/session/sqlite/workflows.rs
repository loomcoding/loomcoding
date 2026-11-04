//! Flujos de desarrollo y su bitácora de eventos.

use super::SqliteStore;
use crate::error::{CoreError, Result};
use crate::message::{self};
use crate::session::SessionId;
use crate::workflow::{DevelopmentWorkflow, DevelopmentWorkflowEvent, WorkflowId, WorkflowPhase};
use sqlx::Row;

impl SqliteStore {
    pub(super) async fn sql_create_development_workflow(
        &self,
        mut workflow: DevelopmentWorkflow,
        detail: &str,
    ) -> Result<DevelopmentWorkflow> {
        if workflow.id != WorkflowId(0)
            || workflow.version != 0
            || workflow.phase != WorkflowPhase::Specifying
            || detail.len() > 2_000
        {
            return Err(CoreError::Store("invalid new development workflow".into()));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let now = message::now_ms();
        workflow.created_at_ms = now;
        workflow.updated_at_ms = now;
        workflow.version = 1;
        let row = sqlx::query(
            "INSERT INTO development_workflows
             (parent_session_id, phase, version, payload, created_at_ms, updated_at_ms)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(workflow.parent.0)
        .bind(workflow.phase.as_str())
        .bind(workflow.version)
        .bind(serde_json::to_string(&workflow).map_err(|e| CoreError::Store(e.to_string()))?)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        workflow.id = WorkflowId(row.last_insert_rowid());
        sqlx::query("UPDATE development_workflows SET payload = ? WHERE id = ?")
            .bind(serde_json::to_string(&workflow).map_err(|e| CoreError::Store(e.to_string()))?)
            .bind(workflow.id.0)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        sqlx::query(
            "INSERT INTO development_workflow_events
             (workflow_id, from_phase, to_phase, actor, detail, created_at_ms)
             VALUES (?, NULL, ?, 'parent', ?, ?)",
        )
        .bind(workflow.id.0)
        .bind(workflow.phase.as_str())
        .bind(detail)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(workflow)
    }

    pub(super) async fn sql_update_development_workflow(
        &self,
        mut workflow: DevelopmentWorkflow,
        actor: &str,
        detail: &str,
    ) -> Result<DevelopmentWorkflow> {
        if actor.is_empty() || actor.len() > 128 || detail.len() > 2_000 {
            return Err(CoreError::Store("invalid workflow event".into()));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let row = sqlx::query(
            "SELECT version, payload FROM development_workflows WHERE id = ? AND parent_session_id = ?",
        )
        .bind(workflow.id.0)
        .bind(workflow.parent.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?
        .ok_or_else(|| CoreError::Store("development workflow not found".into()))?;
        let version: i64 = row.get("version");
        if version != workflow.version {
            return Err(CoreError::Store("stale development workflow update".into()));
        }
        let current: DevelopmentWorkflow = serde_json::from_str(&row.get::<String, _>("payload"))
            .map_err(|e| CoreError::Store(e.to_string()))?;
        if current.id != workflow.id || !current.phase.can_transition_to(workflow.phase) {
            return Err(CoreError::Store(format!(
                "invalid development workflow transition: {:?} -> {:?}",
                current.phase, workflow.phase
            )));
        }
        let now = message::now_ms();
        workflow.version = version + 1;
        workflow.created_at_ms = current.created_at_ms;
        workflow.updated_at_ms = now;
        let changed = sqlx::query(
            "UPDATE development_workflows SET phase = ?, version = ?, payload = ?, updated_at_ms = ?
             WHERE id = ? AND version = ?",
        )
        .bind(workflow.phase.as_str())
        .bind(workflow.version)
        .bind(serde_json::to_string(&workflow).map_err(|e| CoreError::Store(e.to_string()))?)
        .bind(now)
        .bind(workflow.id.0)
        .bind(version)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        if changed.rows_affected() != 1 {
            return Err(CoreError::Store("stale development workflow update".into()));
        }
        sqlx::query(
            "INSERT INTO development_workflow_events
             (workflow_id, from_phase, to_phase, actor, detail, created_at_ms)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(workflow.id.0)
        .bind(current.phase.as_str())
        .bind(workflow.phase.as_str())
        .bind(actor)
        .bind(detail)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(workflow)
    }

    pub(super) async fn sql_development_workflow(
        &self,
        id: WorkflowId,
    ) -> Result<Option<DevelopmentWorkflow>> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM development_workflows WHERE id = ?")
                .bind(id.0)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| CoreError::Store(e.to_string()))?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).map_err(|e| CoreError::Store(e.to_string()))
            })
            .transpose()
    }

    pub(super) async fn sql_active_development_workflow(
        &self,
        parent: SessionId,
    ) -> Result<Option<DevelopmentWorkflow>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM development_workflows WHERE parent_session_id = ?
             AND phase NOT IN ('completed', 'cancelled', 'failed') ORDER BY id DESC LIMIT 1",
        )
        .bind(parent.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).map_err(|e| CoreError::Store(e.to_string()))
            })
            .transpose()
    }

    pub(super) async fn sql_latest_development_workflow(
        &self,
        parent: SessionId,
    ) -> Result<Option<DevelopmentWorkflow>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM development_workflows WHERE parent_session_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(parent.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).map_err(|e| CoreError::Store(e.to_string()))
            })
            .transpose()
    }

    pub(super) async fn sql_development_workflow_events(
        &self,
        id: WorkflowId,
    ) -> Result<Vec<DevelopmentWorkflowEvent>> {
        let rows = sqlx::query(
            "SELECT id, workflow_id, from_phase, to_phase, actor, detail, created_at_ms
             FROM development_workflow_events WHERE workflow_id = ? ORDER BY id",
        )
        .bind(id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        rows.into_iter()
            .map(|row| {
                let parse = |key: &str| -> Result<Option<WorkflowPhase>> {
                    row.get::<Option<String>, _>(key)
                        .map(|value| {
                            value
                                .parse()
                                .map_err(|e: serde_json::Error| CoreError::Store(e.to_string()))
                        })
                        .transpose()
                };
                Ok(DevelopmentWorkflowEvent {
                    sequence: row.get("id"),
                    workflow: WorkflowId(row.get("workflow_id")),
                    from: parse("from_phase")?,
                    to: row
                        .get::<String, _>("to_phase")
                        .parse()
                        .map_err(|e: serde_json::Error| CoreError::Store(e.to_string()))?,
                    actor: row.get("actor"),
                    detail: row.get("detail"),
                    created_at_ms: row.get("created_at_ms"),
                })
            })
            .collect()
    }
}
