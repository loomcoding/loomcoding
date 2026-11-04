//! Utilidades del flujo: fallo del flujo, acotado de entradas, artefactos y lanzamiento de hijos.

use super::*;

pub(super) async fn fail_workflow(
    ctx: &ToolCtx,
    store: &dyn crate::session::SessionStore,
    mut workflow: DevelopmentWorkflow,
    reason: &str,
) -> Result<ToolOutput> {
    workflow.phase = WorkflowPhase::Failed;
    workflow.active_child = None;
    workflow.last_report = Some(reason.to_string());
    let updated = store
        .update_development_workflow(workflow, "coordinator", reason)
        .await?;
    ctx.bus
        .publish(crate::event::LiveEvent::DevelopmentWorkflowChanged(
            updated.summary(),
        ));
    Ok(ToolOutput::err(format!(
        "Workflow failed: {reason}. Do not create or implement artifacts as the parent; report this blocker to the user."
    )))
}

pub(super) fn bounded(input: &serde_json::Value, key: &str, max: usize) -> Result<String> {
    input[key]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= max)
        .map(str::to_owned)
        .ok_or_else(|| CoreError::Tool(format!("{key} must be 1 to {max} bytes")))
}

pub(super) fn planner_platform_instruction() -> &'static str {
    if cfg!(windows) {
        "This project runs on Windows. Inspect with view, ls, glob, or grep. Do not use POSIX commands such as `pwd`, `ls -la`, or `cat` through bash; bash runs cmd.exe here. If shell is essential, invoke PowerShell explicitly. Create new artifacts with the write tool."
    } else {
        "Inspect the project with the available read tools and create new artifacts with the write tool."
    }
}

pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn artifact(cwd: &std::path::Path, relative: &str) -> Result<String> {
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

pub(super) fn verify_artifacts(
    cwd: &std::path::Path,
    workflow: &DevelopmentWorkflow,
) -> Result<()> {
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

pub(super) async fn launch(
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
