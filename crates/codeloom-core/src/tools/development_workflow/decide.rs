//! Acciones de la persona: `cancel` y `accept`.

use super::*;

/// Acción `cancel`: cierra el flujo como cancelado.
pub(super) async fn action_cancel(
    ctx: &ToolCtx,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    mut workflow: DevelopmentWorkflow,
) -> Result<ToolOutput> {
    workflow.phase = WorkflowPhase::Cancelled;
    let updated = store
        .update_development_workflow(workflow, "user", "cancelled")
        .await?;
    ctx.bus
        .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
            updated.summary(),
        ));
    Ok(ToolOutput::ok("Workflow cancelled."))
}

/// Acción `accept`: pide a la persona la aceptación final del resultado revisado.
pub(super) async fn action_accept(
    ctx: &ToolCtx,
    cancel: &CancellationToken,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    mut workflow: DevelopmentWorkflow,
) -> Result<ToolOutput> {
    if workflow.phase != WorkflowPhase::AwaitingFinalAcceptance {
        return Err(CoreError::Tool(
            "workflow is not awaiting final acceptance".into(),
        ));
    }
    let question = [Question {
        header: "Final acceptance".into(),
        question: format!(
            "Accept the reviewed result?\n\n{}",
            workflow.last_report.as_deref().unwrap_or("")
        ),
        options: vec![
            QuestionOption {
                label: "Accept and complete".into(),
                description: String::new(),
            },
            QuestionOption {
                label: "Cancel".into(),
                description: String::new(),
            },
        ],
        multi_select: false,
    }];
    let answers = tokio::select! { _ = cancel.cancelled() => None, a = ctx.permission.ask_user(&question) => a };
    let accepted = answers
        .as_ref()
        .and_then(|a| a.first())
        .is_some_and(|a: &Answer| a.selected == ["Accept and complete"]);
    workflow.phase = if accepted {
        WorkflowPhase::Completed
    } else {
        WorkflowPhase::Cancelled
    };
    let workflow = store
        .update_development_workflow(
            workflow,
            "user",
            if accepted {
                "final acceptance"
            } else {
                "final acceptance declined"
            },
        )
        .await?;
    ctx.bus
        .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
            workflow.summary(),
        ));
    Ok(ToolOutput::ok(if accepted {
        "Workflow completed."
    } else {
        "Workflow cancelled; result was not accepted."
    }))
}
