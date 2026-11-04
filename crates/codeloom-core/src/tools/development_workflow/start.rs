//! Acción `start`.

use super::*;

/// Acción `start`: crea el flujo y lanza al planificador.
pub(super) async fn action_start(
    ctx: &ToolCtx,
    input: &serde_json::Value,
    cancel: &CancellationToken,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    launcher: &std::sync::Arc<dyn crate::subagent::SubagentLauncher>,
) -> Result<ToolOutput> {
    if store
        .active_development_workflow(ctx.session)
        .await?
        .is_some()
    {
        return Err(CoreError::Tool(
            "a development workflow is already active".into(),
        ));
    }
    let task = bounded(input, "task", 8_000)?;
    let planner = bounded(input, "planner", 64)?;
    let implementer = bounded(input, "implementer", 64)?;
    let reviewer = bounded(input, "reviewer", 64)?;
    if planner == implementer || planner == reviewer || implementer == reviewer {
        return Err(CoreError::Tool(
            "planner, implementer, and reviewer must be distinct local agents".into(),
        ));
    }
    let now = crate::message::now_ms();
    let mut workflow = store
        .create_development_workflow(
            DevelopmentWorkflow {
                id: WorkflowId(0),
                parent: ctx.session,
                task: task.clone(),
                planner: planner.clone(),
                implementer,
                reviewer,
                phase: WorkflowPhase::Specifying,
                version: 0,
                active_child: None,
                planner_child: None,
                implementation_children: vec![],
                reviewer_children: vec![],
                correction_round: 0,
                spec_path: None,
                plan_path: None,
                spec_sha256: None,
                plan_sha256: None,
                last_report: None,
                created_at_ms: now,
                updated_at_ms: now,
            },
            "workflow started",
        )
        .await?;
    let platform = planner_platform_instruction();
    let prompt = format!("Produce a repository-specific spec and implementation plan for this task. Do not implement. Return the exact spec and plan file paths, assumptions, and acceptance checks. {platform}\n\nTask:\n{task}\n\nPrior review findings, if any:\n{}", workflow.last_report.as_deref().unwrap_or("none"));
    workflow = launch(
        store.as_ref(),
        ctx.session,
        workflow,
        launcher.as_ref(),
        prompt,
        cancel,
    )
    .await?;
    ctx.bus
        .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
            workflow.summary(),
        ));
    Ok(ToolOutput::ok(format!(
                "Started workflow {} in specifying phase. The planner is running; end this turn and wait for its completion notice. Do not poll jobs or use sleep.",
                workflow.id.0
            )))
}
