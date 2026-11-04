//! Tests: status.

use super::support::*;
use super::*;

#[tokio::test]
async fn completed_workflow_resumes_from_store_and_requires_read_only_review() {
    let project = tempfile::tempdir().unwrap();
    write_artifacts(project.path());
    let directory = tempfile::tempdir().unwrap();
    let db = directory.path().join("workflow.db");
    let (store, session) = store_and_session(&db).await;
    let planner = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;
    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            planner.clone(),
            true,
        ),
        serde_json::json!({
            "action":"start", "task":"test workflow",
            "planner":"planner", "implementer":"builder", "reviewer":"reviewer"
        }),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let active = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    let planner_child = active.active_child.unwrap();
    planner.complete(planner_child, planner_report());

    // Reopen durable state and reconstruct the terminal child descriptor as after reconnect.
    drop(store);
    let store = Arc::new(SqliteStore::open(db.to_str().unwrap()).await.unwrap());
    let launcher = Arc::new(TestLauncher::new());
    launcher.children.lock().unwrap().push(SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent: session,
        child: planner_child,
        job_id: format!("job-{}", planner_child.0),
        label: "development workflow".into(),
        status: TaskStatus::new(TaskState::Completed),
        artifacts: vec![Artifact {
            artifact_id: "planner-report".into(),
            name: Some("report".into()),
            description: None,
            parts: vec![Part::text(planner_report())],
            metadata: None,
            extensions: Vec::new(),
        }],
        notice_delivered: false,
        agent: Some("planner".into()),
        agent_origin: Some("project:test".into()),
        reference_task_ids: Vec::new(),
    });

    tool.run(
        context(project.path(), session, store.clone(), launcher.clone(), true),
        serde_json::json!({"action":"advance", "spec_path":"docs/spec.md", "plan_path":"docs/plan.md"}),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let implementing = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(implementing.phase, WorkflowPhase::Implementing);
    let implementer_child = implementing.active_child.unwrap();
    assert_eq!(
        launcher
            .list(session)
            .await
            .unwrap()
            .last()
            .unwrap()
            .agent
            .as_deref(),
        Some("builder")
    );

    launcher.complete(implementer_child, "implementation complete");
    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            launcher.clone(),
            true,
        ),
        serde_json::json!({"action":"advance"}),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let reviewing = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reviewing.phase, WorkflowPhase::Reviewing);
    assert_eq!(
        launcher.read_only_reviews.lock().unwrap().as_slice(),
        ["reviewer"]
    );
    let reviewer_child = reviewing.active_child.unwrap();

    launcher.complete(reviewer_child, "CLEAN");
    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            launcher.clone(),
            true,
        ),
        serde_json::json!({"action":"advance"}),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let waiting = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(waiting.phase, WorkflowPhase::AwaitingFinalAcceptance);

    tool.run(
        context(project.path(), session, store.clone(), launcher, true),
        serde_json::json!({"action":"accept"}),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let completed = store
        .latest_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.phase, WorkflowPhase::Completed);
}

#[tokio::test]
async fn status_fails_a_completed_planner_with_an_empty_report() {
    let project = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (store, session) = store_and_session(&directory.path().join("empty-report.db")).await;
    let launcher = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;
    tool.run(context(project.path(), session, store.clone(), launcher.clone(), false), serde_json::json!({"action":"start","task":"test","planner":"planner","implementer":"builder","reviewer":"reviewer"}), CancellationToken::new()).await.unwrap();
    let child = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap()
        .active_child
        .unwrap();
    launcher.complete(child, "  ");
    let status = tool
        .run(
            context(project.path(), session, store.clone(), launcher, false),
            serde_json::json!({"action":"status"}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(status.is_error);
    let workflow = store
        .latest_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow.phase, WorkflowPhase::Failed);
    assert!(workflow.active_child.is_none());
}
