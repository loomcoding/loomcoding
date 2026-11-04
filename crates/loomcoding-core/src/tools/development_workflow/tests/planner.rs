//! Tests: planner.

use super::support::*;
use super::*;

#[tokio::test]
async fn planner_that_reports_missing_files_fails_without_starting_implementation() {
    let project = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (store, session) = store_and_session(&directory.path().join("missing-artifacts.db")).await;
    let launcher = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;

    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            launcher.clone(),
            true,
        ),
        serde_json::json!({
            "action":"start", "task":"smoke test",
            "planner":"planner", "implementer":"builder", "reviewer":"reviewer"
        }),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let started = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    let planner_child = started.active_child.unwrap();
    launcher.complete(planner_child, planner_report());

    let result = tool
        .run(
            context(
                project.path(),
                session,
                store.clone(),
                launcher.clone(),
                true,
            ),
            serde_json::json!({
                "action":"advance",
                "spec_path":"docs/spec.md",
                "plan_path":"docs/plan.md"
            }),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result
        .content
        .contains("did not create a readable spec file"));
    assert!(store
        .active_development_workflow(session)
        .await
        .unwrap()
        .is_none());
    let failed = store
        .latest_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.phase, WorkflowPhase::Failed);
    assert!(failed.active_child.is_none());
    assert_eq!(launcher.list(session).await.unwrap().len(), 1);
    assert_eq!(
        launcher.list(session).await.unwrap()[0].agent.as_deref(),
        Some("planner")
    );
}

#[tokio::test]
async fn missing_planner_paths_return_actionable_error_and_can_be_retried() {
    let project = tempfile::tempdir().unwrap();
    write_artifacts(project.path());
    let directory = tempfile::tempdir().unwrap();
    let (store, session) = store_and_session(&directory.path().join("planner-paths.db")).await;
    let launcher = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;

    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            launcher.clone(),
            true,
        ),
        serde_json::json!({
            "action":"start", "task":"smoke test",
            "planner":"planner", "implementer":"builder", "reviewer":"reviewer"
        }),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let started = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    let planner_child = started.active_child.unwrap();
    launcher.complete(planner_child, planner_report());

    let missing_paths = tool
        .run(
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
    assert!(missing_paths.is_error);
    assert!(missing_paths
        .content
        .contains("exact spec_path and plan_path"));
    assert!(missing_paths
        .content
        .contains("Do not create files as the parent"));
    let still_planning = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_planning.phase, WorkflowPhase::Specifying);
    assert_eq!(still_planning.active_child, Some(planner_child));

    tool.run(
        context(
            project.path(),
            session,
            store.clone(),
            launcher.clone(),
            true,
        ),
        serde_json::json!({
            "action":"advance",
            "spec_path":"docs/spec.md",
            "plan_path":"docs/plan.md"
        }),
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
    assert_eq!(launcher.list(session).await.unwrap().len(), 2);
}
