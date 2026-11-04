//! Sesiones y mensajes.

use super::SqliteStore;
use crate::error::{CoreError, Result};
use crate::message::{self, Message};
use crate::session::{SessionId, SessionInfo};
use sqlx::Row;

impl SqliteStore {
    pub(super) async fn sql_create_session(&self, project: &str) -> Result<SessionId> {
        let now = crate::message::now_ms();
        let r = sqlx::query("INSERT INTO sessions (project, created_at_ms) VALUES (?, ?)")
            .bind(project)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(SessionId(r.last_insert_rowid()))
    }

    pub(super) async fn sql_list_sessions(&self, project: &str) -> Result<Vec<SessionInfo>> {
        let rows = sqlx::query(
            "SELECT s.id, COUNT(m.id) AS message_count, MAX(m.id) AS last_message_id
             FROM sessions s
             JOIN messages m ON m.session_id = s.id
WHERE s.project = ?
  OR substr(s.project, 1, length(?) + 1) = ? || ':'
             GROUP BY s.id
             ORDER BY last_message_id DESC",
        )
        .bind(project)
        .bind(project)
        .bind(project)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;

        let mut sessions = Vec::with_capacity(rows.len());
        for row in rows {
            let id = SessionId(row.get("id"));
            let payload: Option<String> = sqlx::query_scalar(
                "SELECT payload FROM messages WHERE session_id = ? ORDER BY seq ASC LIMIT 1",
            )
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
            let preview = payload
                .map(|payload| message::codec::from_json(&payload))
                .transpose()?
                .map(|message| message.text().replace('\n', " "))
                .unwrap_or_default();
            sessions.push(SessionInfo {
                id,
                preview,
                message_count: usize::try_from(row.get::<i64, _>("message_count")).unwrap_or(0),
            });
        }
        Ok(sessions)
    }

    pub(super) async fn sql_session_belongs_to_project(
        &self,
        session: SessionId,
        project: &str,
    ) -> Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ? AND (project = ? OR substr(project, 1, length(?) + 1) = ? || ':'))",
        )
            .bind(session.0)
            .bind(project)
            .bind(project)
            .bind(project)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))
    }

    pub(super) async fn sql_append_message(&self, sid: SessionId, msg: &Message) -> Result<()> {
        let next: i64 = sqlx::query(
            "SELECT COALESCE(MAX(seq), -1) + 1 AS n FROM messages WHERE session_id = ?",
        )
        .bind(sid.0)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?
        .get("n");
        sqlx::query("INSERT INTO messages (session_id, payload, seq) VALUES (?, ?, ?)")
            .bind(sid.0)
            .bind(message::codec::to_json(msg)?)
            .bind(next)
            .execute(&self.pool)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn sql_messages(&self, sid: SessionId) -> Result<Vec<Message>> {
        let rows =
            sqlx::query("SELECT payload FROM messages WHERE session_id = ? ORDER BY seq ASC")
                .bind(sid.0)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| CoreError::Store(e.to_string()))?;
        rows.into_iter()
            .map(|r| message::codec::from_json(r.get::<String, _>("payload").as_str()))
            .collect()
    }

    /// Los últimos `limit` mensajes, ya ordenados de antiguo a nuevo.
    ///
    /// Cabe señalar que la consulta pide `DESC` y luego se da la vuelta: con `ASC LIMIT`
    /// se traerían los más **antiguos**, que es justo lo contrario de lo que hace falta.
    pub(super) async fn sql_recent_messages(
        &self,
        sid: SessionId,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let rows = sqlx::query(
            "SELECT payload FROM messages WHERE session_id = ? ORDER BY seq DESC LIMIT ?",
        )
        .bind(sid.0)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Store(e.to_string()))?;
        let mut mensajes: Vec<Message> = rows
            .into_iter()
            .map(|r| message::codec::from_json(r.get::<String, _>("payload").as_str()))
            .collect::<Result<Vec<_>>>()?;
        mensajes.reverse();
        Ok(mensajes)
    }
}
