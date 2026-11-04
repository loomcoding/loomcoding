//! Tests: artifacts.

use super::*;

#[test]
fn planner_prompt_matches_the_host_shell() {
    let instruction = planner_platform_instruction();
    if cfg!(windows) {
        assert!(instruction.contains("bash runs cmd.exe"));
        assert!(instruction.contains("PowerShell"));
    } else {
        assert!(instruction.contains("available read tools"));
    }
}

#[test]
fn artifacts_are_bounded_and_cannot_escape_the_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("spec.md"), "approved spec").unwrap();
    assert_eq!(artifact(dir.path(), "spec.md").unwrap(), "approved spec");
    assert!(artifact(dir.path(), "../outside.md").is_err());
    assert_ne!(hash(b"approved spec"), hash(b"changed spec"));
}

#[test]
fn changing_an_approved_artifact_invalidates_its_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("spec.md"), "approved spec").unwrap();
    std::fs::write(dir.path().join("plan.md"), "approved plan").unwrap();
    let workflow = DevelopmentWorkflow {
        id: WorkflowId(1),
        parent: SessionId(1),
        task: "task".into(),
        planner: "planner".into(),
        implementer: "builder".into(),
        reviewer: "reviewer".into(),
        phase: WorkflowPhase::Implementing,
        version: 1,
        active_child: None,
        planner_child: None,
        implementation_children: Vec::new(),
        reviewer_children: Vec::new(),
        correction_round: 0,
        spec_path: Some("spec.md".into()),
        plan_path: Some("plan.md".into()),
        spec_sha256: Some(hash(b"approved spec")),
        plan_sha256: Some(hash(b"approved plan")),
        last_report: None,
        created_at_ms: 0,
        updated_at_ms: 0,
    };
    verify_artifacts(dir.path(), &workflow).unwrap();
    std::fs::write(dir.path().join("spec.md"), "changed after approval").unwrap();
    assert!(verify_artifacts(dir.path(), &workflow).is_err());
}
