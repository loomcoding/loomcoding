//! Acción `status`.

use super::*;

/// Acción `status`: estado del flujo y de su hijo activo, sin avanzar nada.
pub(super) async fn action_status(
    ctx: &ToolCtx,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    launcher: &std::sync::Arc<dyn crate::subagent::SubagentLauncher>,
) -> Result<ToolOutput> {
    let mut workflow = match store.active_development_workflow(ctx.session).await? {
        Some(workflow) => workflow,
        None => store
            .latest_development_workflow(ctx.session)
            .await?
            .ok_or_else(|| CoreError::Tool("no development workflow exists".into()))?,
    };
    let mut child_state = None;
    let mut result_available = false;
    let mut next_action = "continue according to the workflow phase";
    let phase_child = workflow.active_child.or_else(|| match workflow.phase {
        WorkflowPhase::Specifying => workflow.planner_child,
        WorkflowPhase::Implementing | WorkflowPhase::ChangesRequested => {
            workflow.implementation_children.last().copied()
        }
        WorkflowPhase::Reviewing => workflow.reviewer_children.last().copied(),
        _ => None,
    });
    if let Some(child) = phase_child {
        let descriptor = launcher
            .list(ctx.session)
            .await?
            .into_iter()
            .find(|item| item.child == child)
            .ok_or_else(|| CoreError::Tool(format!("workflow child {} is missing", child.0)))?;
        child_state = Some(descriptor.status.state);
        if !descriptor.status.state.is_terminal() {
            next_action = "wait for this subagent; it is still running";
        } else if descriptor.status.state == crate::task::TaskState::Completed {
            result_available = descriptor
                .result_text()
                .is_some_and(|report| !report.trim().is_empty());
            if !result_available {
                return fail_workflow(
                    ctx,
                    store.as_ref(),
                    workflow,
                    "workflow child completed without a result",
                )
                .await;
            }
            next_action =
                "read the report and call advance; do not claim the agent is still running";
            // Keep the phase-specific id for `advance`; active_child means running only.
            if workflow.active_child.is_some() {
                workflow.active_child = None;
                workflow = store
                    .update_development_workflow(
                        workflow,
                        "coordinator",
                        "child completed; awaiting advance",
                    )
                    .await?;
                ctx.bus
                    .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                        workflow.summary(),
                    ));
            }
        } else {
            workflow.phase = if matches!(
                descriptor.status.state,
                crate::task::TaskState::Canceled | crate::task::TaskState::Rejected
            ) {
                WorkflowPhase::Cancelled
            } else {
                WorkflowPhase::Failed
            };
            workflow.active_child = None;
            let detail = descriptor.status.message.as_ref().map(|message| {
                message
                    .parts
                    .iter()
                    .filter_map(crate::a2a::Part::as_text)
                    .collect::<Vec<_>>()
                    .join(" ")
            });
            let report = detail
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| {
                    format!("workflow child ended in {:?}", descriptor.status.state)
                });
            workflow.last_report = Some(report.clone());
            workflow = store
                .update_development_workflow(workflow, "coordinator", &report)
                .await?;
            ctx.bus
                .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                    workflow.summary(),
                ));
            next_action = "report the child failure and stop; no later phase may start";
        }
    }
    Ok(ToolOutput::ok(
        serde_json::to_string(&serde_json::json!({
            "workflow": workflow.summary(), "child_state": child_state,
            "result_available": result_available, "next_action": next_action
        }))
        .unwrap_or_default(),
    ))
}
