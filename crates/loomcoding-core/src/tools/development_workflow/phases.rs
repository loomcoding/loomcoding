//! Avance por fase: especificación, implementación y revisión.

use super::*;

/// Avance desde `Specifying`: valida spec y plan, pide aprobación y lanza la implementación.
pub(super) async fn advance_specifying(
    ctx: &ToolCtx,
    input: &serde_json::Value,
    cancel: &CancellationToken,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    launcher: &std::sync::Arc<dyn crate::subagent::SubagentLauncher>,
    mut workflow: DevelopmentWorkflow,
    report: &str,
) -> Result<ToolOutput> {
    workflow.phase = WorkflowPhase::AwaitingPlanApproval;
    let (Some(spec_path), Some(plan_path)) = (
        input["spec_path"]
            .as_str()
            .filter(|path| !path.trim().is_empty() && path.len() <= 512),
        input["plan_path"]
            .as_str()
            .filter(|path| !path.trim().is_empty() && path.len() <= 512),
    ) else {
        return Ok(ToolOutput::err(format!(
                                    "The planner is complete, but advance needs its exact spec_path and plan_path. Read both paths from the planner report and call advance again. Do not create files as the parent. Planner report: {}",
                                    crate::tools::limit_bytes(report, 2_000)
                                )));
    };
    if !report.contains(spec_path) || !report.contains(plan_path) {
        return fail_workflow(
                                    ctx,
                                    store.as_ref(),
                                    workflow,
                                    "planner did not report the exact spec_path and plan_path; no implementation was started",
                                )
                                .await;
    }
    let spec = match artifact(&ctx.cwd, spec_path) {
        Ok(spec) => spec,
        Err(_) => {
            return fail_workflow(
                                        ctx,
                                        store.as_ref(),
                                        workflow,
                                        "planner reported a spec path but did not create a readable spec file; no implementation was started",
                                    )
                                    .await;
        }
    };
    let plan = match artifact(&ctx.cwd, plan_path) {
        Ok(plan) => plan,
        Err(_) => {
            return fail_workflow(
                                        ctx,
                                        store.as_ref(),
                                        workflow,
                                        "planner reported a plan path but did not create a readable plan file; no implementation was started",
                                    )
                                    .await;
        }
    };
    let exact_artifacts = format!("SPEC ({spec_path}):\n{spec}\n\nPLAN ({plan_path}):\n{plan}");
    if exact_artifacts.len() > crate::workflow::MAX_WORKFLOW_ARTIFACT_BYTES {
        return fail_workflow(
            ctx,
            store.as_ref(),
            workflow,
            "combined spec and plan exceed 20000 bytes; no implementation was started",
        )
        .await;
    }
    workflow.spec_path = Some(spec_path.to_string());
    workflow.plan_path = Some(plan_path.to_string());
    workflow.spec_sha256 = Some(hash(spec.as_bytes()));
    workflow.plan_sha256 = Some(hash(plan.as_bytes()));
    workflow.last_report = Some(exact_artifacts.clone());
    workflow = store
        .update_development_workflow(workflow, "planner", "spec and plan ready")
        .await?;
    ctx.bus
        .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
            workflow.summary(),
        ));
    let question = Question {
        header: "Plan approval".into(),
        question: format!(
            "Approve these exact spec and plan files to start implementation?\n\n{}",
            exact_artifacts
        ),
        options: vec![
            QuestionOption {
                label: "Approve and start implementation".into(),
                description: String::new(),
            },
            QuestionOption {
                label: "Cancel".into(),
                description: String::new(),
            },
        ],
        multi_select: false,
    };
    let questions = [question];
    let answers = tokio::select! { _ = cancel.cancelled() => None, a = ctx.permission.ask_user(&questions) => a };
    let approved = answers
        .as_ref()
        .and_then(|a| a.first())
        .is_some_and(|a: &Answer| a.selected == ["Approve and start implementation"]);
    if !approved {
        workflow.phase = WorkflowPhase::Cancelled;
        workflow = store
            .update_development_workflow(workflow, "user", "plan not approved")
            .await?;
        ctx.bus
            .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                workflow.summary(),
            ));
        return Ok(ToolOutput::err(
            "Plan was not explicitly approved; workflow cancelled.",
        ));
    }
    verify_artifacts(&ctx.cwd, &workflow)?;
    workflow.phase = WorkflowPhase::Implementing;
    workflow = store
        .update_development_workflow(workflow, "user", "plan approved")
        .await?;
    let prompt = format!("Implement only this approved task and plan. Report changes, checks, and limits.\nTask: {}\nApproved artifacts:\n{}", workflow.task, exact_artifacts);
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
    Ok(ToolOutput::ok("Plan approved; implementation started."))
}

/// Avance desde `Implementing`: entrega el informe del implementador al revisor.
pub(super) async fn advance_implementing(
    ctx: &ToolCtx,
    cancel: &CancellationToken,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    launcher: &std::sync::Arc<dyn crate::subagent::SubagentLauncher>,
    mut workflow: DevelopmentWorkflow,
    report: &str,
) -> Result<ToolOutput> {
    verify_artifacts(&ctx.cwd, &workflow)?;
    workflow.phase = WorkflowPhase::Reviewing;
    workflow = store
        .update_development_workflow(
            workflow,
            "implementer",
            &format!("implementation report: {}", report),
        )
        .await?;
    let prompt = format!("Review independently against the task and approved artifacts. Return actionable findings or exactly CLEAN. Do not edit.\nTask: {}\nApproved artifacts:\n{}\nImplementation report (the implementer's own claim, not evidence; verify it against the actual files and checks instead of trusting it):\n{}", workflow.task, workflow.last_report.as_deref().unwrap_or(""), report);
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
    Ok(ToolOutput::ok("Implementation delivered to reviewer."))
}

/// Avance desde `Reviewing`: registra el veredicto del revisor.
pub(super) async fn advance_reviewing(
    ctx: &ToolCtx,
    store: &std::sync::Arc<dyn crate::session::SessionStore>,
    mut workflow: DevelopmentWorkflow,
    report: &str,
) -> Result<ToolOutput> {
    workflow.phase = if report.trim().eq_ignore_ascii_case("CLEAN") {
        WorkflowPhase::AwaitingFinalAcceptance
    } else {
        WorkflowPhase::ChangesRequested
    };
    workflow = store
        .update_development_workflow(workflow, "reviewer", report)
        .await?;
    ctx.bus
        .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
            workflow.summary(),
        ));
    Ok(ToolOutput::ok(format!("Review recorded: {:?}. Parent must request final user acceptance or start an in-scope correction.", workflow.phase)))
}
