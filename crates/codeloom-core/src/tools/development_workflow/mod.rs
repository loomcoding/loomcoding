use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::question::{Answer, Question, QuestionOption};
use crate::workflow::{DevelopmentWorkflow, WorkflowId, WorkflowPhase};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

mod advance;
mod corrections;
mod decide;
mod helpers;
mod phases;
mod start;
mod status;

use advance::*;
use corrections::*;
use decide::*;
use helpers::*;
use phases::*;
use start::*;
use status::*;

#[cfg(test)]
mod tests;

pub struct DevelopmentWorkflowTool;

#[async_trait]
impl Tool for DevelopmentWorkflowTool {
    fn name(&self) -> &str {
        "development_workflow"
    }
    fn description(&self) -> &str {
        "Coordinate planner, implementation, and independent review. Planner, implementer, and reviewer must be three distinct local agent names. After start, end the turn and wait for the completion notice; do not poll jobs or sleep. After planner completion, call advance with the exact spec_path and plan_path from its report. Never create or repair planner artifacts as the parent. If the planner did not create both files, report the blocker and stop. Actions: start, advance, accept (final user decision), cancel, status."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{
            "action":{"type":"string","enum":["start","advance","accept","cancel","status"]},
            "task":{"type":"string","maxLength":8000},"planner":{"type":"string","maxLength":64,"description":"Local agent name; must differ from implementer and reviewer."},
            "implementer":{"type":"string","maxLength":64,"description":"Local agent name; must differ from planner and reviewer."},"reviewer":{"type":"string","maxLength":64,"description":"Local agent name; must differ from planner and implementer."},
            "spec_path":{"type":"string","maxLength":512},"plan_path":{"type":"string","maxLength":512}
        },"required":["action"]})
    }
    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let store = ctx
            .store
            .as_ref()
            .ok_or_else(|| CoreError::Tool("workflow storage unavailable".into()))?;
        let launcher = ctx
            .subagents
            .as_ref()
            .ok_or_else(|| CoreError::Tool("subagents unavailable".into()))?;
        let action = input["action"].as_str().unwrap_or_default();
        match action {
            "start" => action_start(&ctx, &input, &cancel, store, launcher).await,
            "status" => action_status(&ctx, store, launcher).await,
            _ => {
                let workflow = store
                    .active_development_workflow(ctx.session)
                    .await?
                    .ok_or_else(|| CoreError::Tool("no active development workflow".into()))?;
                match action {
                    "cancel" => action_cancel(&ctx, store, workflow).await,
                    "accept" => action_accept(&ctx, &cancel, store, workflow).await,
                    "advance" => {
                        action_advance(&ctx, &input, &cancel, store, launcher, workflow).await
                    }
                    _ => Err(CoreError::Tool("unknown workflow action".into())),
                }
            }
        }
    }
}
