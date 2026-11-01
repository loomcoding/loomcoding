//! Herramienta `artifact_publish`: el hijo deja un avance con nombre mientras trabaja.
//!
//! Solo se ofrece a los subagentes. El artefacto es un `Artifact` de A2A marcado como de progreso
//! (`metadata.codeloom.kind`); publicar con un nombre ya usado lo reemplaza. Las cotas viven en
//! `a2a::publicar_progreso` y el almacén las aplica de forma atómica.

use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::event::LiveEvent;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct ArtifactPublishTool;

#[async_trait]
impl Tool for ArtifactPublishTool {
    fn name(&self) -> &str {
        "artifact_publish"
    }

    fn description(&self) -> &str {
        "Publish a named progress note your parent can read while you work (plan, findings so far). Same name replaces it. Max 8 notes, 8 KiB each."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "maxLength": 64 },
                "content": { "type": "string", "maxLength": 8192 },
                "description": { "type": "string", "maxLength": 200 }
            },
            "required": ["name", "content"]
        })
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }

    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        _: CancellationToken,
    ) -> Result<ToolOutput> {
        let (Some(name), Some(content)) = (input["name"].as_str(), input["content"].as_str())
        else {
            return Ok(ToolOutput::err("name and content are required"));
        };
        let Some(store) = &ctx.store else {
            return Ok(ToolOutput::err("session storage is unavailable"));
        };
        match store
            .publish_subagent_progress(ctx.session, name, input["description"].as_str(), content)
            .await
        {
            Ok(descriptor) => {
                ctx.bus.publish(LiveEvent::SubagentChanged(descriptor));
                Ok(ToolOutput::ok(format!(
                    "Progress note «{}» published ({} bytes).",
                    name.trim(),
                    content.len()
                )))
            }
            Err(CoreError::Tool(message)) => Ok(ToolOutput::err(message)),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionService;
    use crate::session::sqlite::SqliteStore;
    use crate::session::{SessionStore, SubagentDescriptor};
    use crate::task::{TaskState, TaskStatus};
    use std::sync::Arc;

    async fn entorno(
        dir: &std::path::Path,
    ) -> (
        ToolCtx,
        crate::session::SessionId,
        crate::session::SessionId,
    ) {
        let store = Arc::new(
            SqliteStore::open(dir.join("tool.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/proj").await.unwrap();
        let child = store.create_session("/proj").await.unwrap();
        store
            .save_subagent(&SubagentDescriptor {
                parent,
                child,
                job_id: "subagent-1".into(),
                label: "hijo".into(),
                status: TaskStatus::new(TaskState::Working),
                artifacts: Vec::new(),
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            })
            .await
            .unwrap();
        let ctx = ToolCtx {
            cwd: dir.to_path_buf(),
            store: Some(store),
            permission: Arc::new(PermissionService::allow_all()),
            session: child,
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        (ctx, parent, child)
    }

    async fn publicar(ctx: &ToolCtx, entrada: serde_json::Value) -> ToolOutput {
        ArtifactPublishTool
            .run(ctx.clone(), entrada, CancellationToken::new())
            .await
            .unwrap()
    }

    #[test]
    fn the_tool_definition_stays_small_because_every_child_request_carries_it() {
        // Unos 350 bytes (≈ 90 tokens) medidos el 2026-11-01; el tope deja holgura, no crecimiento libre.
        let herramienta = ArtifactPublishTool;
        let bytes = herramienta.name().len()
            + herramienta.description().len()
            + herramienta.schema().to_string().len();
        assert!(bytes <= 450, "la definición pesa {bytes} bytes");
    }

    #[tokio::test]
    async fn publishes_a_progress_note_and_announces_the_task_change() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, parent, child) = entorno(dir.path()).await;
        let mut rx = ctx.bus.subscribe();
        let salida = publicar(
            &ctx,
            serde_json::json!({"name": "plan", "content": "1. leer"}),
        )
        .await;
        assert!(!salida.is_error, "{}", salida.content);
        match rx.try_recv().unwrap() {
            LiveEvent::SubagentChanged(descriptor) => {
                assert_eq!(descriptor.child, child);
                assert_eq!(descriptor.parent, parent);
                assert_eq!(descriptor.status.state, TaskState::Working);
                assert_eq!(descriptor.progress_artifacts().count(), 1);
            }
            otro => panic!("evento inesperado: {otro:?}"),
        }
    }

    #[tokio::test]
    async fn a_second_publication_with_the_same_name_replaces_it() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, parent, _) = entorno(dir.path()).await;
        publicar(&ctx, serde_json::json!({"name": "plan", "content": "a"})).await;
        publicar(&ctx, serde_json::json!({"name": "plan", "content": "b"})).await;
        let hijos = ctx
            .store
            .as_ref()
            .unwrap()
            .list_subagents(parent)
            .await
            .unwrap();
        assert_eq!(hijos[0].artifacts.len(), 1);
        assert_eq!(hijos[0].artifacts[0].text(), "b");
    }

    #[tokio::test]
    async fn limits_are_reported_to_the_model_and_publish_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, parent, _) = entorno(dir.path()).await;
        let mut rx = ctx.bus.subscribe();
        let grande = "x".repeat(9_000);
        for entrada in [
            serde_json::json!({"name": "", "content": "x"}),
            serde_json::json!({"name": "a", "content": grande}),
            serde_json::json!({"name": "a"}),
        ] {
            let salida = publicar(&ctx, entrada).await;
            assert!(salida.is_error, "{}", salida.content);
        }
        assert!(rx.try_recv().is_err());
        let hijos = ctx
            .store
            .as_ref()
            .unwrap()
            .list_subagents(parent)
            .await
            .unwrap();
        assert!(hijos[0].artifacts.is_empty());
    }

    #[tokio::test]
    async fn a_session_that_is_not_a_subagent_gets_a_tool_error() {
        let dir = tempfile::tempdir().unwrap();
        let (mut ctx, parent, _) = entorno(dir.path()).await;
        ctx.session = parent;
        let salida = publicar(&ctx, serde_json::json!({"name": "a", "content": "x"})).await;
        assert!(salida.is_error);
        assert!(
            salida.content.contains("not a subagent"),
            "{}",
            salida.content
        );
    }
}
