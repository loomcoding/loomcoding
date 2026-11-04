//! Fase `ChangesRequested`: rondas de corrección y su límite.

use super::*;

/// Fase `ChangesRequested`: envía los hallazgos al implementador o, tras dos rondas, devuelve el control.
pub(super) async fn request_corrections(
    ctx: &ToolCtx,
    cancel: &CancellationToken,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    launcher: &std::sync::Arc<dyn crate::subagent::SubagentLauncher>,
    mut workflow: DevelopmentWorkflow,
) -> Result<ToolOutput> {
    if workflow.correction_round >= 2 {
        let questions = [Question {
            header: "Review limit".into(),
            question: format!(
                "Two correction rounds are complete. What should happen?\n\n{}",
                workflow.last_report.as_deref().unwrap_or("")
            ),
            options: vec![
                QuestionOption {
                    label: "Revise the plan".into(),
                    description: String::new(),
                },
                QuestionOption {
                    label: "Continue manually".into(),
                    description: String::new(),
                },
                QuestionOption {
                    label: "Cancel".into(),
                    description: String::new(),
                },
            ],
            multi_select: false,
        }];
        let answers = tokio::select! { _ = cancel.cancelled() => None, a = ctx.permission.ask_user(&questions) => a };
        let choice = answers
            .as_ref()
            .and_then(|a| a.first())
            .and_then(|a| a.selected.first())
            .map(String::as_str);
        if choice == Some("Revise the plan") {
            workflow.phase = WorkflowPhase::Specifying;
            workflow.spec_path = None;
            workflow.plan_path = None;
            workflow.spec_sha256 = None;
            workflow.plan_sha256 = None;
            workflow = store
                .update_development_workflow(workflow, "user", "user requested plan revision")
                .await?;
            let prompt = format!("Revise the approved spec and plan based on these review findings. Do not implement.\nTask: {}\nFindings:\n{}", workflow.task, workflow.last_report.as_deref().unwrap_or(""));
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
            return Ok(ToolOutput::ok(
                "Plan revision started; implementation remains blocked pending approval.",
            ));
        }
        workflow.phase = WorkflowPhase::Cancelled;
        workflow = store
            .update_development_workflow(
                workflow,
                "user",
                if choice == Some("Continue manually") {
                    "returned to user for manual continuation"
                } else {
                    "cancelled after correction limit"
                },
            )
            .await?;
        ctx.bus
            .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                workflow.summary(),
            ));
        return Ok(ToolOutput::ok(if choice == Some("Continue manually") {
            "Workflow closed; control returned for manual continuation."
        } else {
            "Workflow cancelled."
        }));
    }
    let findings = workflow.last_report.clone().unwrap_or_default();
    workflow.phase = WorkflowPhase::Implementing;
    workflow.correction_round += 1;
    workflow = store
        .update_development_workflow(
            workflow,
            "coordinator",
            "review findings sent for correction",
        )
        .await?;
    let prompt = format!("Correct only these review findings within the approved task and plan.\nTask: {}\nFindings:\n{}", workflow.task, findings);
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
    Ok(ToolOutput::ok(
        "Review findings sent to implementer for correction.",
    ))
}
