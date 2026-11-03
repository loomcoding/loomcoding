use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::Result;
use crate::session::SubagentDescriptor;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct SubagentListTool;

fn descriptor_json(agent: SubagentDescriptor) -> serde_json::Value {
    let avances: Vec<_> = agent
        .progress_artifacts()
        .map(|artefacto| {
            serde_json::json!({
                "name": artefacto.name,
                "bytes": artefacto.text().len(),
            })
        })
        .collect();
    serde_json::json!({
        "artifacts": avances,
        "child_id": agent.child.0,
        "job_id": agent.job_id,
        "label": agent.label,
        "status": format!("{:?}", agent.status.state).to_lowercase(),
        "message": agent.status.message_text().map(|message| {
            crate::tools::limit_bytes(&message, crate::tools::MAX_NOTICE_BYTES).into_owned()
        }),
    })
}

#[async_trait]
impl Tool for SubagentListTool {
    fn name(&self) -> &str {
        "subagent_list"
    }

    fn description(&self) -> &str {
        "List durable child-agent descriptors for the current session, including jobs interrupted by restart."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Parallel
    }

    async fn run(
        &self,
        ctx: ToolCtx,
        _: serde_json::Value,
        _: CancellationToken,
    ) -> Result<ToolOutput> {
        let agents = match &ctx.subagents {
            Some(runtime) => runtime.list(ctx.session).await?,
            None => Vec::new(),
        };
        let result: Vec<_> = agents.into_iter().map(descriptor_json).collect();
        Ok(ToolOutput::ok(serde_json::to_string(&result)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::Artifact;
    use crate::session::SessionId;
    use crate::subagent_phase::SubagentPhase;
    use crate::task::{TaskState, TaskStatus};

    #[test]
    fn descriptor_json_exposes_failure_message_and_stable_state() {
        let descriptor = SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent: SessionId(1),
            child: SessionId(2),
            job_id: "subagent-1".into(),
            label: "backend".into(),
            status: TaskStatus::new(TaskState::Failed).with_message("provider unavailable"),
            artifacts: Vec::<Artifact>::new(),
            notice_delivered: false,
            agent: Some("backend".into()),
            agent_origin: None,
            reference_task_ids: Vec::new(),
        };

        let json = descriptor_json(descriptor);
        assert_eq!(json["status"], "failed");
        assert_eq!(json["message"], "provider unavailable");
        assert_eq!(json["artifacts"], serde_json::json!([]));
    }

    #[test]
    fn descriptor_json_lists_progress_notes_by_name_and_size_without_their_content() {
        let mut artifacts = crate::a2a::publicar_progreso(&[], "plan", None, "1. leer").unwrap();
        artifacts.push(Artifact::text_artifact("resultado final"));
        let descriptor = SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent: SessionId(1),
            child: SessionId(2),
            job_id: "subagent-1".into(),
            label: "backend".into(),
            status: TaskStatus::new(TaskState::Working),
            artifacts,
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        };
        let json = descriptor_json(descriptor);
        assert_eq!(
            json["artifacts"],
            serde_json::json!([{"name": "plan", "bytes": 7}])
        );
        assert!(!json.to_string().contains("1. leer"));
    }
}
