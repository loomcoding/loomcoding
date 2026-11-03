//! Herramienta `artifact_read`: el padre lee un avance que publicó uno de sus hijos.
//!
//! Solo lee artefactos de progreso de hijos de la sesión que llama (los mismos que lista `subagent_list`),
//! con el contenido acotado.

use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::session::SessionId;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct ArtifactReadTool;

#[async_trait]
impl Tool for ArtifactReadTool {
    fn name(&self) -> &str {
        "artifact_read"
    }

    fn description(&self) -> &str {
        "Read a progress note published by one of your child agents (names come from subagent_list). Params: child_id, name."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "child_id": { "type": "integer", "minimum": 1 },
                "name": { "type": "string", "maxLength": 64 }
            },
            "required": ["child_id", "name"]
        })
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Parallel
    }

    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        _: CancellationToken,
    ) -> Result<ToolOutput> {
        let child = input["child_id"]
            .as_i64()
            .filter(|id| *id > 0)
            .ok_or_else(|| CoreError::Tool("child_id must be a positive integer".into()))?;
        let name = input["name"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("name is required".into()))?;
        let launcher = ctx
            .subagents
            .as_ref()
            .ok_or_else(|| CoreError::Tool("subagents are unavailable in this context".into()))?;
        let hijos = launcher.list(ctx.session).await?;
        let Some(hijo) = hijos.iter().find(|hijo| hijo.child == SessionId(child)) else {
            return Ok(ToolOutput::err(format!("unknown subagent {child}")));
        };
        let salida = match hijo
            .progress_artifacts()
            .find(|artefacto| artefacto.name.as_deref() == Some(name))
        {
            Some(artefacto) => ToolOutput::ok(
                super::limit_bytes(&artefacto.text(), crate::a2a::PROGRESS_MAX_CONTENT)
                    .into_owned(),
            ),
            None => ToolOutput::err(format!(
                "subagent {child} has no progress note named «{name}»"
            )),
        };
        Ok(salida)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::publicar_progreso;
    use crate::job::JobId;
    use crate::message::Message;
    use crate::permission::PermissionService;
    use crate::session::SubagentDescriptor;
    use crate::subagent::SubagentLauncher;
    use crate::subagent_phase::SubagentPhase;
    use crate::task::{TaskState, TaskStatus};
    use std::sync::Arc;

    /// Lanzador de prueba: `list` devuelve los hijos del padre indicado.
    struct ConHijos {
        padre: SessionId,
        hijos: Vec<SubagentDescriptor>,
    }

    #[async_trait]
    impl SubagentLauncher for ConHijos {
        async fn start(
            &self,
            _: SessionId,
            _: String,
            _: String,
            _: Option<String>,
            _: CancellationToken,
        ) -> Result<(SessionId, JobId)> {
            Err(CoreError::Tool("not used".into()))
        }
        async fn control(
            &self,
            _: SessionId,
            _: SessionId,
            _: &str,
            _: Option<String>,
        ) -> Result<()> {
            Err(CoreError::Tool("not used".into()))
        }
        async fn cancel(&self, _: SessionId, child: SessionId) -> Result<SubagentDescriptor> {
            Err(CoreError::TaskNotFound(child.0))
        }
        async fn list(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
            Ok(if parent == self.padre {
                self.hijos.clone()
            } else {
                Vec::new()
            })
        }
        async fn deliver_notices(&self, _: SessionId) -> Result<Vec<Message>> {
            Ok(Vec::new())
        }
        async fn answer(&self, _: SessionId, _: SessionId, _: Vec<String>) -> Result<()> {
            Ok(())
        }
    }

    fn ctx(sesion: SessionId, lanzador: Arc<ConHijos>) -> ToolCtx {
        ToolCtx {
            cwd: std::path::PathBuf::from("."),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: sesion,
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: Some(lanzador),
        }
    }

    fn lanzador() -> Arc<ConHijos> {
        let mut artefactos = publicar_progreso(&[], "plan", None, "1. leer\n2. editar").unwrap();
        artefactos.push(crate::a2a::Artifact::text_artifact("resultado"));
        Arc::new(ConHijos {
            padre: SessionId(1),
            hijos: vec![SubagentDescriptor {
                phase: SubagentPhase::Queued,
                last_activity_at_ms: 0,
                steps: 0,
                tool_calls: 0,
                tokens_in: 0,
                tokens_out: 0,
                parent: SessionId(1),
                child: SessionId(2),
                job_id: "subagent-1".into(),
                label: "hijo".into(),
                status: TaskStatus::new(TaskState::Working),
                artifacts: artefactos,
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            }],
        })
    }

    async fn leer(sesion: SessionId, entrada: serde_json::Value) -> Result<ToolOutput> {
        ArtifactReadTool
            .run(ctx(sesion, lanzador()), entrada, CancellationToken::new())
            .await
    }

    #[tokio::test]
    async fn reads_a_progress_note_of_a_child_of_the_caller() {
        let salida = leer(
            SessionId(1),
            serde_json::json!({"child_id": 2, "name": "plan"}),
        )
        .await
        .unwrap();
        assert!(!salida.is_error);
        assert_eq!(salida.content, "1. leer\n2. editar");
    }

    #[tokio::test]
    async fn another_session_cannot_read_a_foreign_childs_notes() {
        let salida = leer(
            SessionId(9),
            serde_json::json!({"child_id": 2, "name": "plan"}),
        )
        .await
        .unwrap();
        assert!(salida.is_error);
        assert!(
            salida.content.contains("unknown subagent"),
            "{}",
            salida.content
        );
    }

    #[tokio::test]
    async fn the_final_result_is_not_a_progress_note_and_unknown_names_fail() {
        for nombre in ["resultado", "nada"] {
            let salida = leer(
                SessionId(1),
                serde_json::json!({"child_id": 2, "name": nombre}),
            )
            .await
            .unwrap();
            assert!(salida.is_error, "{nombre}");
            assert!(
                salida.content.contains("no progress note"),
                "{}",
                salida.content
            );
        }
    }

    #[tokio::test]
    async fn bad_input_is_a_tool_error() {
        let error = leer(
            SessionId(1),
            serde_json::json!({"child_id": 0, "name": "plan"}),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("child_id"), "{error}");
    }
}
