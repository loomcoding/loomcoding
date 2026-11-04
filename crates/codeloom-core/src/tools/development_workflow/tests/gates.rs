//! Tests: gates.

use super::support::*;
use super::*;

#[tokio::test]
async fn yolo_does_not_approve_the_plan_gate() {
    let project = tempfile::tempdir().unwrap();
    write_artifacts(project.path());
    let directory = tempfile::tempdir().unwrap();
    let (store, session) = store_and_session(&directory.path().join("yolo-plan.db")).await;
    let launcher = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;
    tool.run(
        yolo_context(project.path(), session, store.clone(), launcher.clone()),
        serde_json::json!({
            "action":"start", "task":"test workflow",
            "planner":"planner", "implementer":"builder", "reviewer":"reviewer"
        }),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let planner_child = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap()
        .active_child
        .unwrap();
    launcher.complete(planner_child, planner_report());
    let output = tool
        .run(
            yolo_context(project.path(), session, store.clone(), launcher.clone()),
            serde_json::json!({"action":"advance", "spec_path":"docs/spec.md", "plan_path":"docs/plan.md"}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // Permitirlo todo no es aprobar el plan: la puerta exige una respuesta humana explícita.
    assert!(output.is_error);
    let workflow = store
        .latest_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow.phase, WorkflowPhase::Cancelled);
    assert!(workflow.implementation_children.is_empty());
}

#[tokio::test]
async fn yolo_does_not_grant_final_acceptance() {
    let project = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (store, session) = store_and_session(&directory.path().join("yolo-final.db")).await;
    let launcher = Arc::new(TestLauncher::new());
    let mut workflow = store
        .create_development_workflow(
            DevelopmentWorkflow {
                id: WorkflowId(0),
                parent: session,
                task: "test".into(),
                planner: "planner".into(),
                implementer: "builder".into(),
                reviewer: "reviewer".into(),
                phase: WorkflowPhase::Specifying,
                version: 0,
                active_child: None,
                planner_child: None,
                implementation_children: Vec::new(),
                reviewer_children: Vec::new(),
                correction_round: 0,
                spec_path: None,
                plan_path: None,
                spec_sha256: None,
                plan_sha256: None,
                last_report: Some("CLEAN".into()),
                created_at_ms: 0,
                updated_at_ms: 0,
            },
            "test",
        )
        .await
        .unwrap();
    for phase in [
        WorkflowPhase::AwaitingPlanApproval,
        WorkflowPhase::Implementing,
        WorkflowPhase::Reviewing,
        WorkflowPhase::AwaitingFinalAcceptance,
    ] {
        workflow.phase = phase;
        workflow = store
            .update_development_workflow(workflow, "test", "setup")
            .await
            .unwrap();
    }
    DevelopmentWorkflowTool
        .run(
            yolo_context(project.path(), session, store.clone(), launcher),
            serde_json::json!({"action":"accept"}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let after = store
        .latest_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.phase, WorkflowPhase::Cancelled);
}

#[tokio::test]
async fn non_exact_plan_approval_cancels_without_launching_implementer() {
    let project = tempfile::tempdir().unwrap();
    write_artifacts(project.path());
    let directory = tempfile::tempdir().unwrap();
    let (store, session) = store_and_session(&directory.path().join("workflow.db")).await;
    let launcher = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;
    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            launcher.clone(),
            false,
        ),
        serde_json::json!({
            "action":"start", "task":"test workflow",
            "planner":"planner", "implementer":"builder", "reviewer":"reviewer"
        }),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let planner_child = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap()
        .active_child
        .unwrap();
    launcher.complete(planner_child, planner_report());
    let output = tool
        .run(
            context(project.path(), session, store.clone(), launcher.clone(), false),
            serde_json::json!({"action":"advance", "spec_path":"docs/spec.md", "plan_path":"docs/plan.md"}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(output.is_error);
    let workflow = store
        .latest_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow.phase, WorkflowPhase::Cancelled);
    assert_eq!(launcher.children.lock().unwrap().len(), 1);
}
