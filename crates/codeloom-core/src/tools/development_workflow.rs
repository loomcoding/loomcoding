use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::question::{Answer, Question, QuestionOption};
use crate::workflow::{DevelopmentWorkflow, WorkflowId, WorkflowPhase};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

pub struct DevelopmentWorkflowTool;

#[async_trait]
impl Tool for DevelopmentWorkflowTool {
    fn name(&self) -> &str {
        "development_workflow"
    }
    fn description(&self) -> &str {
        "Coordinate the approved planner, implementation, and independent review workflow. Actions: start, advance, accept (final user decision), cancel, status."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{
            "action":{"type":"string","enum":["start","advance","accept","cancel","status"]},
            "task":{"type":"string","maxLength":8000},"planner":{"type":"string","maxLength":64},
            "implementer":{"type":"string","maxLength":64},"reviewer":{"type":"string","maxLength":64},
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
        if action == "start" {
            if store
                .active_development_workflow(ctx.session)
                .await?
                .is_some()
            {
                return Err(CoreError::Tool(
                    "a development workflow is already active".into(),
                ));
            }
            let task = bounded(&input, "task", 8_000)?;
            let planner = bounded(&input, "planner", 64)?;
            let implementer = bounded(&input, "implementer", 64)?;
            let reviewer = bounded(&input, "reviewer", 64)?;
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
                        version: 1,
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
            let prompt = format!("Produce a repository-specific spec and implementation plan for this task. Do not implement. Return the exact spec and plan file paths, assumptions, and acceptance checks.\n\nTask:\n{task}\n\nPrior review findings, if any:\n{}", workflow.last_report.as_deref().unwrap_or("none"));
            workflow = launch(
                store.as_ref(),
                ctx.session,
                workflow,
                launcher.as_ref(),
                prompt,
                &cancel,
            )
            .await?;
            ctx.bus
                .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                    workflow.summary(),
                ));
            Ok(ToolOutput::ok(format!(
                "Started workflow {} in specifying phase.",
                workflow.id.0
            )))
        } else if action == "status" {
            let workflow = match store.active_development_workflow(ctx.session).await? {
                Some(workflow) => workflow,
                None => store
                    .latest_development_workflow(ctx.session)
                    .await?
                    .ok_or_else(|| CoreError::Tool("no development workflow exists".into()))?,
            };
            Ok(ToolOutput::ok(
                serde_json::to_string(&workflow.summary()).unwrap_or_default(),
            ))
        } else {
            let mut workflow = store
                .active_development_workflow(ctx.session)
                .await?
                .ok_or_else(|| CoreError::Tool("no active development workflow".into()))?;
            match action {
                "cancel" => {
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
                "accept" => {
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
                "advance" => {
                    if workflow.phase == WorkflowPhase::ChangesRequested {
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
                                    .update_development_workflow(
                                        workflow,
                                        "user",
                                        "user requested plan revision",
                                    )
                                    .await?;
                                let prompt = format!("Revise the approved spec and plan based on these review findings. Do not implement.\nTask: {}\nFindings:\n{}", workflow.task, workflow.last_report.as_deref().unwrap_or(""));
                                workflow = launch(
                                    store.as_ref(),
                                    ctx.session,
                                    workflow,
                                    launcher.as_ref(),
                                    prompt,
                                    &cancel,
                                )
                                .await?;
                                ctx.bus.publish(
                                    crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                        workflow.summary(),
                                    ),
                                );
                                return Ok(ToolOutput::ok("Plan revision started; implementation remains blocked pending approval."));
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
                            &cancel,
                        )
                        .await?;
                        ctx.bus
                            .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                workflow.summary(),
                            ));
                        return Ok(ToolOutput::ok(
                            "Review findings sent to implementer for correction.",
                        ));
                    }
                    let descriptors = launcher.list(ctx.session).await?;
                    let child = workflow
                        .active_child
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
                        let updated = store
                            .update_development_workflow(
                                workflow,
                                "coordinator",
                                "workflow child did not complete successfully",
                            )
                            .await?;
                        ctx.bus
                            .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                updated.summary(),
                            ));
                        return Ok(ToolOutput::err(
                            "workflow child did not complete successfully; no phase was advanced",
                        ));
                    }
                    let report = descriptor.result_text().ok_or_else(|| {
                        CoreError::Tool("workflow child completed without a result".into())
                    })?;
                    if report.len() > 8_000 {
                        return Err(CoreError::Tool("report exceeds 8000 bytes".into()));
                    }
                    workflow.last_report = Some(report.to_owned());
                    workflow.active_child = None;
                    match workflow.phase {
                        WorkflowPhase::Specifying => {
                            workflow.phase = WorkflowPhase::AwaitingPlanApproval;
                            let spec_path = bounded(&input, "spec_path", 512)?;
                            let plan_path = bounded(&input, "plan_path", 512)?;
                            if !report.contains(&spec_path) || !report.contains(&plan_path) {
                                return Err(CoreError::Tool(
                                    "spec and plan paths must come from the planner result".into(),
                                ));
                            }
                            let spec = artifact(&ctx.cwd, &spec_path)?;
                            let plan = artifact(&ctx.cwd, &plan_path)?;
                            let exact_artifacts = format!(
                                "SPEC ({spec_path}):\n{spec}\n\nPLAN ({plan_path}):\n{plan}"
                            );
                            if exact_artifacts.len() > crate::workflow::MAX_WORKFLOW_ARTIFACT_BYTES
                            {
                                return Err(CoreError::Tool(
                                    "combined spec and plan exceed 20000 bytes".into(),
                                ));
                            }
                            workflow.spec_path = Some(spec_path);
                            workflow.plan_path = Some(plan_path);
                            workflow.spec_sha256 = Some(hash(spec.as_bytes()));
                            workflow.plan_sha256 = Some(hash(plan.as_bytes()));
                            workflow.last_report = Some(exact_artifacts.clone());
                            workflow = store
                                .update_development_workflow(
                                    workflow,
                                    "planner",
                                    "spec and plan ready",
                                )
                                .await?;
                            ctx.bus
                                .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                    workflow.summary(),
                                ));
                            let question = Question { header: "Plan approval".into(), question: format!("Approve these exact spec and plan files to start implementation?\n\n{}", exact_artifacts), options: vec![QuestionOption { label: "Approve and start implementation".into(), description: String::new() }, QuestionOption { label: "Cancel".into(), description: String::new() }], multi_select: false };
                            let questions = [question];
                            let answers = tokio::select! { _ = cancel.cancelled() => None, a = ctx.permission.ask_user(&questions) => a };
                            let approved = answers.as_ref().and_then(|a| a.first()).is_some_and(
                                |a: &Answer| a.selected == ["Approve and start implementation"],
                            );
                            if !approved {
                                workflow.phase = WorkflowPhase::Cancelled;
                                workflow = store
                                    .update_development_workflow(
                                        workflow,
                                        "user",
                                        "plan not approved",
                                    )
                                    .await?;
                                ctx.bus.publish(
                                    crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                        workflow.summary(),
                                    ),
                                );
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
                                &cancel,
                            )
                            .await?;
                            ctx.bus
                                .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                    workflow.summary(),
                                ));
                            Ok(ToolOutput::ok("Plan approved; implementation started."))
                        }
                        WorkflowPhase::Implementing | WorkflowPhase::ChangesRequested => {
                            verify_artifacts(&ctx.cwd, &workflow)?;
                            workflow.phase = WorkflowPhase::Reviewing;
                            workflow = store
                                .update_development_workflow(
                                    workflow,
                                    "implementer",
                                    &format!("implementation report: {}", report),
                                )
                                .await?;
                            let prompt = format!("Review independently against the task and approved artifacts. Return actionable findings or exactly CLEAN. Do not edit.\nTask: {}\nApproved artifacts:\n{}\nImplementation report:\n{}", workflow.task, workflow.last_report.as_deref().unwrap_or(""), report);
                            workflow = launch(
                                store.as_ref(),
                                ctx.session,
                                workflow,
                                launcher.as_ref(),
                                prompt,
                                &cancel,
                            )
                            .await?;
                            ctx.bus
                                .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                    workflow.summary(),
                                ));
                            Ok(ToolOutput::ok("Implementation delivered to reviewer."))
                        }
                        WorkflowPhase::Reviewing => {
                            workflow.phase = if report.trim().eq_ignore_ascii_case("CLEAN") {
                                WorkflowPhase::AwaitingFinalAcceptance
                            } else {
                                WorkflowPhase::ChangesRequested
                            };
                            workflow = store
                                .update_development_workflow(workflow, "reviewer", &report)
                                .await?;
                            ctx.bus
                                .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
                                    workflow.summary(),
                                ));
                            Ok(ToolOutput::ok(format!("Review recorded: {:?}. Parent must request final user acceptance or start an in-scope correction.", workflow.phase)))
                        }
                        _ => Err(CoreError::Tool(
                            "workflow cannot advance from this phase".into(),
                        )),
                    }
                }
                _ => Err(CoreError::Tool("unknown workflow action".into())),
            }
        }
    }
}

fn bounded(input: &serde_json::Value, key: &str, max: usize) -> Result<String> {
    input[key]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= max)
        .map(str::to_owned)
        .ok_or_else(|| CoreError::Tool(format!("{key} must be 1 to {max} bytes")))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn artifact(cwd: &std::path::Path, relative: &str) -> Result<String> {
    let root = cwd
        .canonicalize()
        .map_err(|e| CoreError::Tool(e.to_string()))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| CoreError::Tool(format!("cannot read workflow artifact: {e}")))?;
    if !path.starts_with(&root) {
        return Err(CoreError::Tool(
            "workflow artifacts must be inside the project".into(),
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| CoreError::Tool(e.to_string()))?;
    if bytes.len() > crate::workflow::MAX_WORKFLOW_ARTIFACT_BYTES {
        return Err(CoreError::Tool(
            "workflow artifact exceeds 20000 bytes".into(),
        ));
    }
    String::from_utf8(bytes).map_err(|_| CoreError::Tool("workflow artifacts must be UTF-8".into()))
}
fn verify_artifacts(cwd: &std::path::Path, workflow: &DevelopmentWorkflow) -> Result<()> {
    let spec = artifact(
        cwd,
        workflow
            .spec_path
            .as_deref()
            .ok_or_else(|| CoreError::Tool("missing approved spec path".into()))?,
    )?;
    let plan = artifact(
        cwd,
        workflow
            .plan_path
            .as_deref()
            .ok_or_else(|| CoreError::Tool("missing approved plan path".into()))?,
    )?;
    if Some(hash(spec.as_bytes())) != workflow.spec_sha256
        || Some(hash(plan.as_bytes())) != workflow.plan_sha256
    {
        return Err(CoreError::Tool(
            "approved spec or plan changed; cancel and request a new plan approval".into(),
        ));
    }
    Ok(())
}
async fn launch(
    store: &dyn crate::session::SessionStore,
    parent: crate::session::SessionId,
    mut workflow: DevelopmentWorkflow,
    launcher: &dyn crate::subagent::SubagentLauncher,
    prompt: String,
    cancel: &CancellationToken,
) -> Result<DevelopmentWorkflow> {
    let agent = match workflow.phase {
        WorkflowPhase::Specifying => workflow.planner.clone(),
        WorkflowPhase::Implementing | WorkflowPhase::ChangesRequested => {
            workflow.implementer.clone()
        }
        WorkflowPhase::Reviewing => workflow.reviewer.clone(),
        _ => return Err(CoreError::Tool("invalid launch phase".into())),
    };
    let launched = if workflow.phase == WorkflowPhase::Reviewing {
        launcher
            .start_read_only_review(
                parent,
                "development workflow review".into(),
                prompt,
                agent,
                cancel.clone(),
            )
            .await
    } else {
        launcher
            .start_local_workflow_agent(
                parent,
                "development workflow".into(),
                prompt,
                agent,
                cancel.clone(),
            )
            .await
    };
    let (child, _) = match launched {
        Ok(child) => child,
        Err(error) => {
            workflow.phase = WorkflowPhase::Failed;
            let _ = store
                .update_development_workflow(
                    workflow,
                    "coordinator",
                    &format!("launch failed: {error}"),
                )
                .await;
            return Err(error);
        }
    };
    workflow.attach_child(child);
    match store
        .update_development_workflow(workflow, "coordinator", "child launched")
        .await
    {
        Ok(workflow) => Ok(workflow),
        Err(error) => {
            let _ = launcher.cancel(parent, child).await;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacts_are_bounded_and_cannot_escape_the_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("spec.md"), "approved spec").unwrap();
        assert_eq!(artifact(dir.path(), "spec.md").unwrap(), "approved spec");
        assert!(artifact(dir.path(), "../outside.md").is_err());
        assert_ne!(hash(b"approved spec"), hash(b"changed spec"));
    }
}
