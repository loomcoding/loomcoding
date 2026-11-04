//! Registro y lectura del uso de tokens.

use super::codec::*;
use super::SqliteStore;
use crate::error::{CoreError, Result};
use crate::session::{SessionId, UsageRecord};
use sqlx::Row;

impl SqliteStore {
    pub(super) async fn sql_record_usage(
        &self,
        session: SessionId,
        record: &UsageRecord,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO usage (session_id, model, input_tokens, output_tokens, input_price, output_price, reported_cost_usd, source, is_final, components_json, cache_read_tokens, cache_write_tokens, created_at_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(session.0)
        .bind(&record.model)
        .bind(to_sql_int(record.input_tokens)?)
        .bind(to_sql_int(record.output_tokens)?)
        .bind(record.input_price)
        .bind(record.output_price)
        .bind(record.reported_cost_usd)
        .bind(match record.source {
            crate::message::UsageSource::Provider => "provider",
            crate::message::UsageSource::Estimate => "estimate",
        })
        .bind(record.is_final)
        .bind(serde_json::to_string(&record.components).map_err(|e| CoreError::Store(e.to_string()))?)
        .bind(to_sql_int(record.cache_read_tokens)?)
        .bind(to_sql_int(record.cache_write_tokens)?)
        .bind(record.created_at_ms)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn sql_session_usage(&self, session: SessionId) -> Result<Vec<UsageRecord>> {
        let rows = sqlx::query(
            "SELECT session_id, model, input_tokens, output_tokens, input_price, output_price, reported_cost_usd, source, is_final, components_json, cache_read_tokens, cache_write_tokens, created_at_ms
             FROM usage
             WHERE session_id IN (
                 WITH RECURSIVE arbol(id) AS (
                     SELECT ?
                     UNION
                     SELECT s.child_session_id FROM subagents s JOIN arbol a ON s.parent_session_id = a.id
                 )
                 SELECT id FROM arbol
             )
             ORDER BY id",
        )
        .bind(session.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        rows.into_iter()
            .map(|row| {
                Ok(UsageRecord {
                    session: SessionId(row.get("session_id")),
                    model: row.get("model"),
                    input_tokens: from_sql_int(row.get("input_tokens"))?,
                    output_tokens: from_sql_int(row.get("output_tokens"))?,
                    input_price: row.get("input_price"),
                    output_price: row.get("output_price"),
                    reported_cost_usd: row.get("reported_cost_usd"),
                    source: match row.get::<String, _>("source").as_str() {
                        "estimate" => crate::message::UsageSource::Estimate,
                        _ => crate::message::UsageSource::Provider,
                    },
                    is_final: row.get::<i64, _>("is_final") != 0,
                    components: serde_json::from_str(
                        row.get::<String, _>("components_json").as_str(),
                    )
                    .unwrap_or_default(),
                    cache_read_tokens: from_sql_int(row.get("cache_read_tokens"))?,
                    cache_write_tokens: from_sql_int(row.get("cache_write_tokens"))?,
                    created_at_ms: row.get("created_at_ms"),
                })
            })
            .collect()
    }
}
