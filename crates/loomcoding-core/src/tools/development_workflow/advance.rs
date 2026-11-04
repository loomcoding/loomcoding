//! Acción `advance`: localiza el hijo terminado y reparte según la fase.

use super::*;

/// Acción `advance`: da el siguiente paso según la fase, cuando el hijo activo ya terminó.
pub(super) async fn action_advance(
    ctx: &ToolCtx,
    input: &serde_json::Value,
    cancel: &CancellationToken,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    launcher: &std::sync::Arc<dyn crate::subagent::SubagentLauncher>,
    mut workflow: DevelopmentWorkflow,
) -> Result<ToolOutput> {
    if workflow.phase == WorkflowPhase::ChangesRequested {
        return request_corrections(ctx, cancel, store, launcher, workflow).await;
    }
    let descriptors = launcher.list(ctx.session).await?;
    let child = workflow
        .active_child
        .or_else(|| match workflow.phase {
            WorkflowPhase::Specifying => workflow.planner_child,
            WorkflowPhase::Implementing | WorkflowPhase::ChangesRequested => {
                workflow.implementation_children.last().copied()
            }
            WorkflowPhase::Reviewing => workflow.reviewer_children.last().copied(),
            _ => None,
        })
        .ok_or_else(|| CoreError::Tool("workflow has no active child".into()))?;
    let descriptor = descriptors
        .into_iter()
        .find(|d| d.child == child)
        .ok_or_else(|| CoreError::Tool("workflow child is missing".into()))?;
    if !descriptor.status.state.is_terminal() {
        return Ok(ToolOutput::err("workflow child is still running"));
    }
    if descriptor.status.state != crate::task::TaskState::Completed {
        workflow.phase = if matches!(
            descriptor.status.state,
            crate::task::TaskState::Canceled | crate::task::TaskState::Rejected
        ) {
            WorkflowPhase::Cancelled
        } else {
            WorkflowPhase::Failed
        };
        workflow.active_child = None;
        let reason = format!("workflow child ended in {:?}", descriptor.status.state);
        workflow.last_report = Some(reason.clone());
        let updated = store
            .update_development_workflow(workflow, "coordinator", &reason)
            .await?;
        ctx.bus
            .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                updated.summary(),
            ));
        return Ok(ToolOutput::err(
            "workflow child did not complete successfully; no phase was advanced",
        ));
    }
    let Some(report) = descriptor
        .result_text()
        .filter(|report| !report.trim().is_empty())
    else {
        return fail_workflow(
            ctx,
            store.as_ref(),
            workflow,
            "planner completed without a result; no implementation was started",
        )
        .await;
    };
    if report.len() > 8_000 {
        return fail_workflow(
            ctx,
            store.as_ref(),
            workflow,
            "planner report exceeds 8000 bytes; no implementation was started",
        )
        .await;
    }
    workflow.last_report = Some(report.to_owned());
    workflow.active_child = None;
    match workflow.phase {
        WorkflowPhase::Specifying => {
            advance_specifying(ctx, input, cancel, store, launcher, workflow, &report).await
        }
        WorkflowPhase::Implementing | WorkflowPhase::ChangesRequested => {
            advance_implementing(ctx, cancel, store, launcher, workflow, &report).await
        }
        WorkflowPhase::Reviewing => advance_reviewing(ctx, store, workflow, &report).await,
        _ => Err(CoreError::Tool(
            "workflow cannot advance from this phase".into(),
        )),
    }
}
