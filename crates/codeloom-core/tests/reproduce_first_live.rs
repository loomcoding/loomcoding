//! Prueba real de D4.3: ¿cambia la conducta (y el resultado) del agente si se le exige reproducir el
//! fallo antes de arreglarlo? Ignorada por defecto: gasta saldo y exige `OPENROUTER_API_KEY`.
//! Se lanza con:
//! `cargo test -p codeloom-core --test reproduce_first_live -- --ignored --nocapture`
//!
//! Es un solo caso con pocas repeticiones: sirve para ver conducta y descartar efectos enormes, no
//! para concluir una regla. Tope de gasto fijo (`CAP_USD`); se aborta al superarlo.

use codeloom_core::agent::AgentLoop;
use codeloom_core::config::{CodeLoomConfig, ProviderConfig, ProviderKind};
use codeloom_core::event::EventBus;
use codeloom_core::message::ContentPart;
use codeloom_core::permission::PermissionService;
use codeloom_core::provider::anthropic::AnthropicProvider;
use codeloom_core::session::sqlite::SqliteStore;
use codeloom_core::session::SessionStore;
use codeloom_core::tools::bash::BashTool;
use codeloom_core::tools::edit::EditTool;
use codeloom_core::tools::view::ViewTool;
use codeloom_core::tools::ToolRegistry;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const CAP_USD: f64 = 0.25;
const RUNS_PER_VARIANT: usize = 3;
const MODEL: &str = "anthropic/claude-haiku-4.5";

const INVENTORY: &str = r#"def low_stock(items, threshold=5):
    """Names of items whose quantity is strictly below threshold, sorted."""
    return sorted(i["name"] for i in items if i["qty"] <= threshold)


def total_value(items):
    """Total stock value in euros, rounded to cents."""
    return sum(i["qty"] * i["price"] for i in items)


def restock_plan(items, target=10):
    """{name: units to order} for items below target."""
    plan = {}
    for i in items:
        if i["qty"] < target:
            plan[i["name"]] = target - i["qty"]
    return plan
"#;

const TESTS: &str = r#"import unittest
from inventory import low_stock, total_value, restock_plan

ITEMS = [
    {"name": "bolt", "qty": 5, "price": 0.1},
    {"name": "nut", "qty": 3, "price": 0.2},
    {"name": "gear", "qty": 12, "price": 1.15},
]


class T(unittest.TestCase):
    def test_low_stock_is_strict(self):
        self.assertEqual(low_stock(ITEMS), ["nut"])

    def test_total_value_is_rounded_to_cents(self):
        self.assertEqual(total_value(ITEMS), 14.9)

    def test_restock_plan(self):
        self.assertEqual(restock_plan(ITEMS), {"bolt": 5, "nut": 7})


if __name__ == "__main__":
    unittest.main()
"#;

const BASE_TASK: &str = "The tests in test_inventory.py are failing. Fix inventory.py so that all tests pass. Do not modify the tests.";
const REPRODUCE_FIRST: &str = " Work in this order: first reproduce the failure by running the tests and reading their output; then make the fix; then run the tests again to confirm.";

struct Outcome {
    solved: bool,
    ran_tests_before_editing: bool,
    tool_calls: usize,
    cost: f64,
}

async fn run_once(key: &str, task: &str, effort: Option<&str>) -> Outcome {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("inventory.py"), INVENTORY).unwrap();
    std::fs::write(dir.path().join("test_inventory.py"), TESTS).unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ViewTool));
    tools.register(Arc::new(EditTool));
    tools.register(Arc::new(BashTool));
    let agent = AgentLoop::new(
        Arc::new(AnthropicProvider::new("https://openrouter.ai/api", key)),
        store.clone(),
        Arc::new(tools),
        EventBus::new(256),
        Arc::new(PermissionService::allow_all()),
        CodeLoomConfig {
            provider: ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url: "https://openrouter.ai/api".into(),
                api_key_env: "OPENROUTER_API_KEY".into(),
            },
            model: MODEL.into(),
            max_steps: 14,
            effort: effort.map(String::from),
        },
        dir.path().to_path_buf(),
    );
    let _ = agent.run(sid, task.into(), CancellationToken::new()).await;

    let mut order = Vec::new();
    for message in store.messages(sid).await.unwrap() {
        for part in message.parts {
            if let ContentPart::ToolCall { name, input, .. } = part {
                order.push((name, input.to_string()));
            }
        }
    }
    let first_edit = order.iter().position(|(name, _)| name == "edit");
    let first_test_run = order
        .iter()
        .position(|(name, input)| name == "bash" && input.contains("test"));
    let ran_tests_before_editing = match (first_test_run, first_edit) {
        (Some(run), Some(edit)) => run < edit,
        (Some(_), None) => true,
        _ => false,
    };
    let cost: f64 = store
        .session_usage(sid)
        .await
        .unwrap()
        .iter()
        .map(|record| record.cost().unwrap_or(0.0))
        .sum();
    let solved = std::process::Command::new("python")
        .args(["-m", "unittest", "-q"])
        .current_dir(dir.path())
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    Outcome {
        solved,
        ran_tests_before_editing,
        tool_calls: order.len(),
        cost,
    }
}

#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn reproduce_first_versus_baseline() {
    let key = std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY");
    let variants = [
        ("baseline", BASE_TASK.to_string()),
        ("reproduce-first", format!("{BASE_TASK}{REPRODUCE_FIRST}")),
    ];
    let mut spent = 0.0_f64;
    for (label, task) in &variants {
        let (mut solved, mut ran_first, mut calls, mut cost) = (0, 0, 0, 0.0);
        let mut runs = 0;
        for _ in 0..RUNS_PER_VARIANT {
            if spent >= CAP_USD {
                println!("cap reached, stopping before the next run");
                break;
            }
            let outcome = run_once(&key, task, None).await;
            runs += 1;
            spent += outcome.cost;
            solved += usize::from(outcome.solved);
            ran_first += usize::from(outcome.ran_tests_before_editing);
            calls += outcome.tool_calls;
            cost += outcome.cost;
            println!(
                "{label}: solved={} tests_before_edit={} tool_calls={} cost={:.5}",
                outcome.solved, outcome.ran_tests_before_editing, outcome.tool_calls, outcome.cost
            );
        }
        println!(
            "SUMMARY {label}: runs={runs} solved={solved} tests_before_edit={ran_first} tool_calls={calls} cost={cost:.5}"
        );
    }
    println!("total_spent_usd~{spent:.5}");
    assert!(spent < CAP_USD + 0.05, "tope de gasto superado");
}

/// Etapa 2 del benchmark de esfuerzo (C2): el mismo agente y la misma tarea que la línea base, con
/// `effort = low`. Tope de gasto propio; la comparación es con la medición ya publicada.
#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn stage2_low_effort_on_execution() {
    const STAGE2_CAP_USD: f64 = 0.35;
    let key = std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY");
    let (mut spent, mut solved, mut calls, mut runs) = (0.0_f64, 0, 0, 0);
    for _ in 0..4 {
        if spent >= STAGE2_CAP_USD {
            println!("cap reached, stopping before the next run");
            break;
        }
        let outcome = run_once(&key, BASE_TASK, Some("low")).await;
        runs += 1;
        spent += outcome.cost;
        solved += usize::from(outcome.solved);
        calls += outcome.tool_calls;
        println!(
            "low: solved={} tool_calls={} cost={:.5}",
            outcome.solved, outcome.tool_calls, outcome.cost
        );
    }
    println!("SUMMARY low: runs={runs} solved={solved} tool_calls={calls} cost={spent:.5}");
    assert!(spent < STAGE2_CAP_USD + 0.05, "tope de gasto superado");
}
