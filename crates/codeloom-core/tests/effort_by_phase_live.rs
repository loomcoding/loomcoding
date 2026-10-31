//! Benchmark del esfuerzo de razonamiento por fase (C2); spec en
//! `docs/specs/2026-10-31-codeloom-reasoning-effort-benchmark.md`. Las pruebas vivas van ignoradas:
//! gastan saldo y exigen `OPENROUTER_API_KEY`. Se lanzan, una etapa cada vez, con:
//! `cargo test -p codeloom-core --test effort_by_phase_live stage0 -- --ignored --nocapture`
//!
//! La muestra es pequeña y de un solo modelo: sirve para ver magnitudes, no para fijar una regla.
//! Cada etapa tiene su tope de gasto y se detiene antes de la siguiente llamada si lo supera.

use codeloom_core::agent::AgentLoop;
use codeloom_core::config::{CodeLoomConfig, ProviderConfig, ProviderKind};
use codeloom_core::event::EventBus;
use codeloom_core::message::{ContentPart, Role};
use codeloom_core::permission::PermissionService;
use codeloom_core::provider::anthropic::AnthropicProvider;
use codeloom_core::session::sqlite::SqliteStore;
use codeloom_core::session::SessionStore;
use codeloom_core::tools::ToolRegistry;
use regex::Regex;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const MODEL: &str = "anthropic/claude-haiku-4.5";
const STAGE0_CAP_USD: f64 = 0.05;
const STAGE1_CAP_USD: f64 = 0.40;
const REPS: usize = 2;

const INSTRUCTION: &str = "Review the following Python function for defects. Reply with a short list of findings, one line each, \
naming the problem. If the function has no defects, reply with exactly NO_DEFECTS.\n\n";

/// Un fragmento a revisar. `defect_pattern` es `None` en el caso limpio.
struct Case {
    name: &'static str,
    code: &'static str,
    defect_pattern: Option<&'static str>,
}

const CASES: [Case; 6] = [
    Case {
        name: "pagination-off-by-one",
        code: "def page(items, number, size=10):\n    \"\"\"Items on page `number` (1-based): exactly `size` per page, last page may be shorter.\"\"\"\n    start = (number - 1) * size\n    return items[start:start + size + 1]\n",
        defect_pattern: Some(r"off[- ]by[- ]one|one (extra|more)|size ?\+ ?1|\+ ?1|more than|too many|extra (item|element)"),
    },
    Case {
        name: "mutable-default",
        code: "def add_tag(tag, tags=[]):\n    \"\"\"Return a list with `tag` appended to `tags` (a new list per call when none is given).\"\"\"\n    tags.append(tag)\n    return tags\n",
        defect_pattern: Some(r"mutable|default (argument|arg|value|parameter)|shared (between|across)"),
    },
    Case {
        name: "sql-injection",
        code: "def find_user(conn, name):\n    \"\"\"Return the row of the user called `name`, or None.\"\"\"\n    cur = conn.execute(f\"SELECT * FROM users WHERE name = '{name}'\")\n    return cur.fetchone()\n",
        defect_pattern: Some(r"injection|parameteri[sz]ed|placeholder|f-string|sanitiz"),
    },
    Case {
        name: "swallowed-exception",
        code: "def save_all(records, store):\n    \"\"\"Save every record; return True only if all of them were saved.\"\"\"\n    for record in records:\n        try:\n            store.save(record)\n        except Exception:\n            pass\n    return True\n",
        defect_pattern: Some(r"swallow|silent|ignor|suppress|hid(es|den|ing)|bare|broad"),
    },
    Case {
        name: "strict-comparison",
        code: "def overdue(invoices, today):\n    \"\"\"Invoices whose due date is strictly before `today`.\"\"\"\n    return [i for i in invoices if i[\"due\"] <= today]\n",
        defect_pattern: Some(r"<=|strict|inclusive|equal|same day|due today"),
    },
    Case {
        name: "clean",
        code: "def clamp(value, low, high):\n    \"\"\"Limit `value` to the inclusive range [low, high]; low must not exceed high.\"\"\"\n    if low > high:\n        raise ValueError(\"low must not exceed high\")\n    return max(low, min(value, high))\n",
        defect_pattern: None,
    },
];

/// Resultado de puntuar una respuesta: acierto y, solo en el caso limpio, falso positivo.
#[derive(Debug, PartialEq, Eq)]
struct Score {
    hit: bool,
    false_positive: bool,
}

fn score(case: &Case, answer: &str) -> Score {
    match case.defect_pattern {
        Some(pattern) => Score {
            hit: Regex::new(&format!("(?i){pattern}"))
                .unwrap()
                .is_match(answer),
            false_positive: false,
        },
        None => {
            let clean = answer.contains("NO_DEFECTS");
            Score {
                hit: clean,
                false_positive: !clean,
            }
        }
    }
}

/// Acumula el gasto de una etapa y dice si cabe otra llamada.
struct Spend {
    cap: f64,
    spent: f64,
}

impl Spend {
    fn new(cap: f64) -> Self {
        Self { cap, spent: 0.0 }
    }
    fn allows_next(&self) -> bool {
        self.spent < self.cap
    }
    fn charge(&mut self, cost: f64) {
        self.spent += cost;
    }
}

struct Review {
    text: String,
    output_tokens: u64,
    cost: f64,
}

async fn review(key: &str, code: &str, effort: Option<&str>) -> Review {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let agent = AgentLoop::new(
        Arc::new(AnthropicProvider::new("https://openrouter.ai/api", key)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        CodeLoomConfig {
            provider: ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url: "https://openrouter.ai/api".into(),
                api_key_env: "OPENROUTER_API_KEY".into(),
            },
            model: MODEL.into(),
            max_steps: 2,
            effort: effort.map(String::from),
        },
        dir.path().to_path_buf(),
    );
    let _ = agent
        .run(
            sid,
            format!("{INSTRUCTION}{code}"),
            CancellationToken::new(),
        )
        .await;
    let text = store
        .messages(sid)
        .await
        .unwrap()
        .into_iter()
        .filter(|message| message.role == Role::Assistant)
        .flat_map(|message| message.parts)
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let usage = store.session_usage(sid).await.unwrap();
    Review {
        text,
        output_tokens: usage.iter().map(|record| record.output_tokens).sum(),
        cost: usage
            .iter()
            .map(|record| record.cost().unwrap_or(0.0))
            .sum(),
    }
}

fn key() -> String {
    std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY")
}

#[test]
fn every_seeded_case_matches_a_description_of_its_defect_and_not_a_generic_answer() {
    let descriptions = [
        "page returns one extra item: the slice end is start + size + 1",
        "mutable default argument: the list is shared between calls",
        "SQL injection through the f-string; use a parameterized query",
        "the broad except swallows every error and the function still returns True",
        "the comparison uses <= but the docstring says strictly before",
    ];
    for (case, description) in CASES.iter().zip(descriptions) {
        assert!(score(case, description).hit, "{}", case.name);
        assert!(
            !score(case, "Looks fine. Consider adding type hints.").hit,
            "{}",
            case.name
        );
    }
}

#[test]
fn the_clean_case_only_hits_with_no_defects_and_otherwise_is_a_false_positive() {
    let clean = &CASES[5];
    assert_eq!(
        score(clean, "NO_DEFECTS"),
        Score {
            hit: true,
            false_positive: false
        }
    );
    assert_eq!(
        score(clean, "- clamp: should validate that value is a number"),
        Score {
            hit: false,
            false_positive: true
        }
    );
}

#[test]
fn the_spend_accumulator_stops_the_stage_once_its_cap_is_reached() {
    let mut spend = Spend::new(0.05);
    assert!(spend.allows_next());
    spend.charge(0.03);
    assert!(spend.allows_next());
    spend.charge(0.03);
    assert!(!spend.allows_next());
}

#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn stage0_effort_reaches_the_model() {
    let key = key();
    let code = CASES[2].code;
    let mut spend = Spend::new(STAGE0_CAP_USD);
    let mut tokens = Vec::new();
    for effort in [None, Some("high")] {
        assert!(spend.allows_next(), "tope de la etapa 0 alcanzado");
        let result = review(&key, code, effort).await;
        spend.charge(result.cost);
        println!(
            "effort={effort:?} output_tokens={} cost={:.5} answer_chars={}",
            result.output_tokens,
            result.cost,
            result.text.len()
        );
        tokens.push(result.output_tokens);
    }
    println!("stage0_spent_usd~{:.5}", spend.spent);
    assert!(
        tokens[1] > tokens[0],
        "con effort=high no suben los tokens de salida: la tabla no surte efecto"
    );
}

#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn stage1_review_without_tools() {
    let key = key();
    let mut spend = Spend::new(STAGE1_CAP_USD);
    for effort in [None, Some("high")] {
        let (mut hits, mut false_positives, mut tokens, mut cost, mut runs) = (0, 0, 0_u64, 0.0, 0);
        for case in &CASES {
            for _ in 0..REPS {
                if !spend.allows_next() {
                    println!("cap reached, stopping before the next call");
                    break;
                }
                let result = review(&key, case.code, effort).await;
                spend.charge(result.cost);
                let outcome = score(case, &result.text);
                runs += 1;
                hits += usize::from(outcome.hit);
                false_positives += usize::from(outcome.false_positive);
                tokens += result.output_tokens;
                cost += result.cost;
                let shown: String = result.text.chars().take(240).collect();
                println!(
                    "{effort:?} {} hit={} fp={} out={} cost={:.5} :: {}",
                    case.name,
                    outcome.hit,
                    outcome.false_positive,
                    result.output_tokens,
                    result.cost,
                    shown.replace('\n', " | ")
                );
            }
        }
        println!(
            "SUMMARY {effort:?}: runs={runs} hits={hits} false_positives={false_positives} output_tokens={tokens} cost={cost:.5}"
        );
    }
    println!("stage1_spent_usd~{:.5}", spend.spent);
    assert!(
        spend.spent < STAGE1_CAP_USD + 0.05,
        "tope de gasto superado"
    );
}
