//! Un turno cortado por un tope debe decir por qué: quien lo lanza sin TUI no ve nada más.

use super::steps::{final_round, round, run_with_limits};
use super::*;
use crate::agent::stop::StopReason;
use crate::config::LimitsConfig;

fn allow() -> Arc<PermissionService> {
    Arc::new(PermissionService::allow_all())
}

#[tokio::test]
async fn a_turn_cut_by_the_step_budget_says_so() {
    let scripts = vec![round("edit", 1), round("edit", 2), final_round()];
    let (outcome, _) = run_with_limits(scripts, 1, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(outcome.stopped, Some(StopReason::StepBudget));
}

#[tokio::test]
async fn a_turn_cut_by_the_read_budget_says_so() {
    let mut scripts: Vec<_> = (0..10).map(|n| round("view", n)).collect();
    scripts.push(final_round());
    let limits = LimitsConfig {
        max_read_steps: Some(3),
        ..LimitsConfig::default()
    };
    let (outcome, _) = run_with_limits(scripts, 30, limits, allow()).await;
    assert_eq!(outcome.stopped, Some(StopReason::ReadBudget));
}

#[tokio::test]
async fn a_finished_turn_reports_no_stop_and_no_unfinished_report() {
    let (outcome, _) =
        run_with_limits(vec![final_round()], 5, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.stopped, None);
    assert_eq!(outcome.unfinished_report(), None);
}

#[tokio::test]
async fn an_unfinished_turn_reports_the_reason_and_the_counts() {
    let scripts = vec![round("edit", 1), round("edit", 2), final_round()];
    let (outcome, _) = run_with_limits(scripts, 1, LimitsConfig::default(), allow()).await;
    let report = outcome.unfinished_report().expect("sin texto final");
    assert!(report.contains("step limit"), "{report}");
    assert!(report.contains("1 work steps"), "{report}");
    assert!(report.contains("0 read steps"), "{report}");
}
