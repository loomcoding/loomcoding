//! Gauntlets y su bitácora de eventos. Mismo patrón que `workflows.rs`: carga útil JSON,
//! versión optimista y un solo gauntlet activo por sesión.

use super::SqliteStore;
use crate::error::{CoreError, Result};
use crate::gauntlet::{Gauntlet, GauntletEvent, GauntletId, GauntletPhase};
use crate::message;
use crate::session::SessionId;
use sqlx::Row;

fn store_error(error: impl std::fmt::Display) -> CoreError {
    CoreError::Store(error.to_string())
}

fn decode(payload: String) -> Result<Gauntlet> {
    serde_json::from_str(&payload).map_err(store_error)
}

impl SqliteStore {
    pub(super) async fn sql_create_gauntlet(
        &self,
        mut gauntlet: Gauntlet,
        detail: &str,
    ) -> Result<Gauntlet> {
        if gauntlet.id != GauntletId(0)
            || gauntlet.version != 0
            || gauntlet.phase != GauntletPhase::Building
            || !gauntlet.rounds.is_empty()
            || detail.len() > 2_000
        {
            return Err(CoreError::Store("invalid new gauntlet".into()));
        }
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let now = message::now_ms();
        gauntlet.created_at_ms = now;
        gauntlet.updated_at_ms = now;
        gauntlet.version = 1;
        let row = sqlx::query(
            "INSERT INTO gauntlets
             (parent_session_id, phase, version, payload, created_at_ms, updated_at_ms)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(gauntlet.parent.0)
        .bind(gauntlet.phase.as_str())
        .bind(gauntlet.version)
        .bind(serde_json::to_string(&gauntlet).map_err(store_error)?)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        gauntlet.id = GauntletId(row.last_insert_rowid());
        sqlx::query("UPDATE gauntlets SET payload = ? WHERE id = ?")
            .bind(serde_json::to_string(&gauntlet).map_err(store_error)?)
            .bind(gauntlet.id.0)
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        sqlx::query(
            "INSERT INTO gauntlet_events
             (gauntlet_id, from_phase, to_phase, actor, detail, created_at_ms)
             VALUES (?, NULL, ?, 'parent', ?, ?)",
        )
        .bind(gauntlet.id.0)
        .bind(gauntlet.phase.as_str())
        .bind(detail)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(gauntlet)
    }

    pub(super) async fn sql_update_gauntlet(
        &self,
        mut gauntlet: Gauntlet,
        actor: &str,
        detail: &str,
    ) -> Result<Gauntlet> {
        if actor.is_empty() || actor.len() > 128 || detail.len() > 2_000 {
            return Err(CoreError::Store("invalid gauntlet event".into()));
        }
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query(
            "SELECT version, payload FROM gauntlets WHERE id = ? AND parent_session_id = ?",
        )
        .bind(gauntlet.id.0)
        .bind(gauntlet.parent.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?
        .ok_or_else(|| CoreError::Store("gauntlet not found".into()))?;
        let version: i64 = row.get("version");
        if version != gauntlet.version {
            return Err(CoreError::Store("stale gauntlet update".into()));
        }
        let current = decode(row.get::<String, _>("payload"))?;
        if current.id != gauntlet.id || !current.phase.can_transition_to(gauntlet.phase) {
            return Err(CoreError::Store(format!(
                "invalid gauntlet transition: {:?} -> {:?}",
                current.phase, gauntlet.phase
            )));
        }
        let now = message::now_ms();
        gauntlet.version = version + 1;
        gauntlet.created_at_ms = current.created_at_ms;
        gauntlet.updated_at_ms = now;
        let changed = sqlx::query(
            "UPDATE gauntlets SET phase = ?, version = ?, payload = ?, updated_at_ms = ?
             WHERE id = ? AND version = ?",
        )
        .bind(gauntlet.phase.as_str())
        .bind(gauntlet.version)
        .bind(serde_json::to_string(&gauntlet).map_err(store_error)?)
        .bind(now)
        .bind(gauntlet.id.0)
        .bind(version)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        if changed.rows_affected() != 1 {
            return Err(CoreError::Store("stale gauntlet update".into()));
        }
        sqlx::query(
            "INSERT INTO gauntlet_events
             (gauntlet_id, from_phase, to_phase, actor, detail, created_at_ms)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(gauntlet.id.0)
        .bind(current.phase.as_str())
        .bind(gauntlet.phase.as_str())
        .bind(actor)
        .bind(detail)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(gauntlet)
    }

    pub(super) async fn sql_gauntlet(&self, id: GauntletId) -> Result<Option<Gauntlet>> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM gauntlets WHERE id = ?")
                .bind(id.0)
                .fetch_optional(&self.pool)
                .await
                .map_err(store_error)?;
        payload.map(decode).transpose()
    }

    pub(super) async fn sql_active_gauntlet(&self, parent: SessionId) -> Result<Option<Gauntlet>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM gauntlets WHERE parent_session_id = ?
             AND phase NOT IN ('completed', 'stopped', 'inconclusive', 'cancelled', 'failed')
             ORDER BY id DESC LIMIT 1",
        )
        .bind(parent.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        payload.map(decode).transpose()
    }

    pub(super) async fn sql_latest_gauntlet(&self, parent: SessionId) -> Result<Option<Gauntlet>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM gauntlets WHERE parent_session_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(parent.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        payload.map(decode).transpose()
    }

    pub(super) async fn sql_gauntlet_events(&self, id: GauntletId) -> Result<Vec<GauntletEvent>> {
        let rows = sqlx::query(
            "SELECT id, gauntlet_id, from_phase, to_phase, actor, detail, created_at_ms
             FROM gauntlet_events WHERE gauntlet_id = ? ORDER BY id",
        )
        .bind(id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                let parse = |key: &str| -> Result<Option<GauntletPhase>> {
                    row.get::<Option<String>, _>(key)
                        .map(|value| value.parse().map_err(store_error))
                        .transpose()
                };
                Ok(GauntletEvent {
                    sequence: row.get("id"),
                    gauntlet: GauntletId(row.get("gauntlet_id")),
                    from: parse("from_phase")?,
                    to: row
                        .get::<String, _>("to_phase")
                        .parse()
                        .map_err(store_error)?,
                    actor: row.get("actor"),
                    detail: row.get("detail"),
                    created_at_ms: row.get("created_at_ms"),
                })
            })
            .collect()
    }
}
