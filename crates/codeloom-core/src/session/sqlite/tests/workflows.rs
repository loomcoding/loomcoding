//! Flujos de desarrollo: estado y bitácora persisten al reabrir.

use super::*;
use crate::workflow::{DevelopmentWorkflow, WorkflowId, WorkflowPhase};
use tempfile::tempdir;

fn workflow(parent: SessionId) -> DevelopmentWorkflow {
    DevelopmentWorkflow {
        id: WorkflowId(0),
        parent,
        task: "add a safe feature".into(),
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
        last_report: None,
        created_at_ms: 0,
        updated_at_ms: 0,
    }
}

#[tokio::test]
async fn workflow_state_and_append_only_events_survive_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("sessions.db");
    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let parent = store.create_session("project").await.unwrap();
    let created = store
        .create_development_workflow(workflow(parent), "planner started")
        .await
        .unwrap();

    let mut pending = created.clone();
    pending.phase = WorkflowPhase::AwaitingPlanApproval;
    pending.spec_path = Some("docs/spec.md".into());
    pending.spec_sha256 = Some("a".repeat(64));
    let pending = store
        .update_development_workflow(pending, "parent", "plan ready")
        .await
        .unwrap();

    let mut stale = created;
    stale.phase = WorkflowPhase::Implementing;
    assert!(store
        .update_development_workflow(stale, "parent", "must not overwrite")
        .await
        .is_err());

    let events = store.development_workflow_events(pending.id).await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].from, Some(WorkflowPhase::Specifying));
    assert_eq!(events[1].to, WorkflowPhase::AwaitingPlanApproval);
    drop(store);

    let reopened = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let loaded = reopened
        .development_workflow(pending.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded, pending);
    assert_eq!(
        reopened.active_development_workflow(parent).await.unwrap(),
        Some(loaded)
    );
}
