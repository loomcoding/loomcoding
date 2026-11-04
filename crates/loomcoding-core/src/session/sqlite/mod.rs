//! Almacén SQLite de sesiones, subagentes, uso y flujos. El `impl SessionStore` de aquí solo
//! reenvía: la lógica de cada tema vive en su módulo, como método de `SqliteStore`.

use super::{SessionId, SessionInfo, SessionStore, SubagentDescriptor, UsageRecord};
use crate::a2a::Artifact;
use crate::error::{CoreError, Result};
use crate::gauntlet::{Gauntlet, GauntletEvent, GauntletId};
use crate::message::Message;
use crate::subagent_phase::SubagentPhase;
use crate::task::TaskStatus;
use crate::workflow::{DevelopmentWorkflow, DevelopmentWorkflowEvent, WorkflowId};
use async_trait::async_trait;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

mod codec;
mod gauntlets;
mod schema;
mod sessions;
mod subagents;
mod usage;
mod workflows;

#[cfg(test)]
mod tests;

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn open(path: &str) -> Result<Self> {
        let url = format!("sqlite://{path}?mode=rwc");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .map_err(|e| CoreError::Store(e.to_string()))?;
        schema::migrate(&pool).await?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn create_session(&self, project: &str) -> Result<SessionId> {
        self.sql_create_session(project).await
    }

    async fn list_sessions(&self, project: &str) -> Result<Vec<SessionInfo>> {
        self.sql_list_sessions(project).await
    }

    async fn session_belongs_to_project(&self, session: SessionId, project: &str) -> Result<bool> {
        self.sql_session_belongs_to_project(session, project).await
    }

    async fn append_message(&self, sid: SessionId, msg: &Message) -> Result<()> {
        self.sql_append_message(sid, msg).await
    }

    async fn messages(&self, sid: SessionId) -> Result<Vec<Message>> {
        self.sql_messages(sid).await
    }

    async fn recent_messages(&self, sid: SessionId, limit: usize) -> Result<Vec<Message>> {
        self.sql_recent_messages(sid, limit).await
    }

    async fn save_subagent(&self, descriptor: &SubagentDescriptor) -> Result<()> {
        self.sql_save_subagent(descriptor).await
    }

    async fn update_subagent_status(
        &self,
        parent: SessionId,
        child: SessionId,
        status: TaskStatus,
    ) -> Result<()> {
        self.sql_update_subagent_status(parent, child, status).await
    }

    async fn update_subagent_runtime(
        &self,
        parent: SessionId,
        child: SessionId,
        phase: SubagentPhase,
        last_activity_at_ms: u64,
    ) -> Result<()> {
        self.sql_update_subagent_runtime(parent, child, phase, last_activity_at_ms)
            .await
    }

    async fn list_subagents(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
        self.sql_list_subagents(parent).await
    }

    async fn complete_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
        artifacts: &[Artifact],
    ) -> Result<()> {
        self.sql_complete_subagent(parent, child, artifacts).await
    }

    async fn publish_subagent_progress(
        &self,
        child: SessionId,
        name: &str,
        description: Option<&str>,
        content: &str,
    ) -> Result<SubagentDescriptor> {
        self.sql_publish_subagent_progress(child, name, description, content)
            .await
    }

    async fn append_subagent_notice(
        &self,
        parent: SessionId,
        child: SessionId,
        message: &Message,
    ) -> Result<bool> {
        self.sql_append_subagent_notice(parent, child, message)
            .await
    }

    async fn record_usage(&self, session: SessionId, record: &UsageRecord) -> Result<()> {
        self.sql_record_usage(session, record).await
    }

    async fn session_usage(&self, session: SessionId) -> Result<Vec<UsageRecord>> {
        self.sql_session_usage(session).await
    }

    async fn create_development_workflow(
        &self,
        workflow: DevelopmentWorkflow,
        detail: &str,
    ) -> Result<DevelopmentWorkflow> {
        self.sql_create_development_workflow(workflow, detail).await
    }

    async fn update_development_workflow(
        &self,
        workflow: DevelopmentWorkflow,
        actor: &str,
        detail: &str,
    ) -> Result<DevelopmentWorkflow> {
        self.sql_update_development_workflow(workflow, actor, detail)
            .await
    }

    async fn development_workflow(&self, id: WorkflowId) -> Result<Option<DevelopmentWorkflow>> {
        self.sql_development_workflow(id).await
    }

    async fn active_development_workflow(
        &self,
        parent: SessionId,
    ) -> Result<Option<DevelopmentWorkflow>> {
        self.sql_active_development_workflow(parent).await
    }

    async fn latest_development_workflow(
        &self,
        parent: SessionId,
    ) -> Result<Option<DevelopmentWorkflow>> {
        self.sql_latest_development_workflow(parent).await
    }

    async fn development_workflow_events(
        &self,
        id: WorkflowId,
    ) -> Result<Vec<DevelopmentWorkflowEvent>> {
        self.sql_development_workflow_events(id).await
    }

    async fn create_gauntlet(&self, gauntlet: Gauntlet, detail: &str) -> Result<Gauntlet> {
        self.sql_create_gauntlet(gauntlet, detail).await
    }

    async fn update_gauntlet(
        &self,
        gauntlet: Gauntlet,
        actor: &str,
        detail: &str,
    ) -> Result<Gauntlet> {
        self.sql_update_gauntlet(gauntlet, actor, detail).await
    }

    async fn gauntlet(&self, id: GauntletId) -> Result<Option<Gauntlet>> {
        self.sql_gauntlet(id).await
    }

    async fn active_gauntlet(&self, parent: SessionId) -> Result<Option<Gauntlet>> {
        self.sql_active_gauntlet(parent).await
    }

    async fn latest_gauntlet(&self, parent: SessionId) -> Result<Option<Gauntlet>> {
        self.sql_latest_gauntlet(parent).await
    }

    async fn gauntlet_events(&self, id: GauntletId) -> Result<Vec<GauntletEvent>> {
        self.sql_gauntlet_events(id).await
    }
}
