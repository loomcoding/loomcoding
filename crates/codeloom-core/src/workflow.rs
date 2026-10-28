//! Estado durable del flujo de desarrollo coordinado por el agente padre.

use crate::session::SessionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    Specifying,
    AwaitingPlanApproval,
    Implementing,
    Reviewing,
    ChangesRequested,
    AwaitingFinalAcceptance,
    Completed,
    Cancelled,
    Failed,
}

impl WorkflowPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Specifying => "specifying",
            Self::AwaitingPlanApproval => "awaiting_plan_approval",
            Self::Implementing => "implementing",
            Self::Reviewing => "reviewing",
            Self::ChangesRequested => "changes_requested",
            Self::AwaitingFinalAcceptance => "awaiting_final_acceptance",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for WorkflowPhase {
    type Err = serde_json::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        serde_json::from_value(serde_json::Value::String(value.to_owned()))
    }
}

impl WorkflowPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use WorkflowPhase::*;
        self == next
            || matches!(
                (self, next),
                (Specifying, AwaitingPlanApproval | Cancelled | Failed)
                    | (AwaitingPlanApproval, Implementing | Cancelled | Failed)
                    | (
                        Implementing,
                        Reviewing | AwaitingPlanApproval | Cancelled | Failed
                    )
                    | (
                        Reviewing,
                        ChangesRequested | AwaitingFinalAcceptance | Cancelled | Failed
                    )
                    | (
                        ChangesRequested,
                        Implementing | Specifying | Cancelled | Failed
                    )
                    | (AwaitingFinalAcceptance, Completed | Cancelled | Failed)
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevelopmentWorkflow {
    pub id: WorkflowId,
    pub parent: SessionId,
    pub task: String,
    pub planner: String,
    pub implementer: String,
    pub reviewer: String,
    pub phase: WorkflowPhase,
    pub version: i64,
    pub active_child: Option<SessionId>,
    pub planner_child: Option<SessionId>,
    pub implementation_children: Vec<SessionId>,
    pub reviewer_children: Vec<SessionId>,
    pub correction_round: u8,
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub spec_sha256: Option<String>,
    pub plan_sha256: Option<String>,
    pub last_report: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl DevelopmentWorkflow {
    pub fn summary(&self) -> WorkflowSummary {
        WorkflowSummary {
            id: self.id,
            parent: self.parent,
            phase: self.phase,
            planner: self.planner.clone(),
            implementer: self.implementer.clone(),
            reviewer: self.reviewer.clone(),
            active_child: self.active_child,
            correction_round: self.correction_round,
            spec_path: self.spec_path.clone(),
            plan_path: self.plan_path.clone(),
            updated_at_ms: self.updated_at_ms,
        }
    }

    pub fn from_subagent(&self, child: SessionId) -> Option<WorkflowPhase> {
        (self.active_child == Some(child)).then_some(self.phase)
    }

    pub fn attach_child(&mut self, child: SessionId) {
        self.active_child = Some(child);
        match self.phase {
            WorkflowPhase::Specifying => self.planner_child = Some(child),
            WorkflowPhase::Implementing | WorkflowPhase::ChangesRequested => {
                self.implementation_children.push(child);
                self.correction_round = self.implementation_children.len().saturating_sub(1) as u8;
            }
            WorkflowPhase::Reviewing => self.reviewer_children.push(child),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub id: WorkflowId,
    pub parent: SessionId,
    pub phase: WorkflowPhase,
    pub planner: String,
    pub implementer: String,
    pub reviewer: String,
    pub active_child: Option<SessionId>,
    pub correction_round: u8,
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevelopmentWorkflowEvent {
    pub sequence: i64,
    pub workflow: WorkflowId,
    pub from: Option<WorkflowPhase>,
    pub to: WorkflowPhase,
    pub actor: String,
    pub detail: String,
    pub created_at_ms: i64,
}

impl DevelopmentWorkflowEvent {
    pub fn changed(
        sequence: i64,
        workflow: WorkflowId,
        from: Option<WorkflowPhase>,
        to: WorkflowPhase,
        actor: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            sequence,
            workflow,
            from,
            to,
            actor: actor.into(),
            detail: detail.into(),
            created_at_ms: crate::message::now_ms(),
        }
    }
}

/// Límites de entradas externas del coordinador; los artefactos aprobados son archivos locales.
pub const MAX_WORKFLOW_TASK_BYTES: usize = 8_000;
pub const MAX_WORKFLOW_ARTIFACT_BYTES: usize = 20_000;
pub const MAX_WORKFLOW_REPORT_BYTES: usize = 8_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_phases_only_follow_the_approved_lifecycle() {
        use WorkflowPhase::*;
        let legal = [
            (Specifying, AwaitingPlanApproval),
            (AwaitingPlanApproval, Implementing),
            (Implementing, Reviewing),
            (Reviewing, ChangesRequested),
            (ChangesRequested, Implementing),
            (Reviewing, AwaitingFinalAcceptance),
            (AwaitingFinalAcceptance, Completed),
            (Implementing, AwaitingPlanApproval),
        ];
        for (from, to) in legal {
            assert!(from.can_transition_to(to), "{from:?} -> {to:?}");
        }
        for terminal in [Completed, Cancelled, Failed] {
            assert!(terminal.is_terminal());
            assert!(!terminal.can_transition_to(Implementing));
        }
        assert!(!AwaitingPlanApproval.can_transition_to(Reviewing));
        assert!(!Reviewing.can_transition_to(Completed));
    }

    #[test]
    fn summary_exposes_phase_and_role_without_copying_task_content() {
        let workflow = DevelopmentWorkflow {
            id: WorkflowId(1),
            parent: SessionId(2),
            task: "private request".into(),
            planner: "planner".into(),
            implementer: "builder".into(),
            reviewer: "reviewer".into(),
            phase: WorkflowPhase::Implementing,
            version: 1,
            active_child: Some(SessionId(3)),
            planner_child: Some(SessionId(3)),
            implementation_children: Vec::new(),
            reviewer_children: Vec::new(),
            correction_round: 0,
            spec_path: None,
            plan_path: None,
            spec_sha256: None,
            plan_sha256: None,
            last_report: None,
            created_at_ms: 1,
            updated_at_ms: 2,
        };
        let summary = workflow.summary();
        assert_eq!(summary.phase, WorkflowPhase::Implementing);
        assert_eq!(summary.active_child, Some(SessionId(3)));
        assert!(!serde_json::to_string(&summary)
            .unwrap()
            .contains("private request"));
    }

    #[test]
    fn child_ids_are_kept_by_role_and_round() {
        let mut workflow = DevelopmentWorkflow {
            id: WorkflowId(1),
            parent: SessionId(2),
            task: String::new(),
            planner: "p".into(),
            implementer: "i".into(),
            reviewer: "r".into(),
            phase: WorkflowPhase::Implementing,
            version: 1,
            active_child: None,
            planner_child: None,
            implementation_children: vec![SessionId(3)],
            reviewer_children: Vec::new(),
            correction_round: 0,
            spec_path: None,
            plan_path: None,
            spec_sha256: None,
            plan_sha256: None,
            last_report: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        workflow.phase = WorkflowPhase::Reviewing;
        workflow.attach_child(SessionId(4));
        workflow.phase = WorkflowPhase::ChangesRequested;
        workflow.phase = WorkflowPhase::Implementing;
        workflow.attach_child(SessionId(5));
        assert_eq!(
            workflow.implementation_children,
            [SessionId(3), SessionId(5)]
        );
        assert_eq!(workflow.correction_round, 1);
        assert_eq!(workflow.reviewer_children, [SessionId(4)]);
    }
}
