//! Gauntlet real contra un servidor OpenAI-compatible (por ejemplo `llama-server`). Ignorada por
//! defecto: gasta tiempo de GPU y escribe en el directorio indicado. Se lanza con:
//! `cargo test -p loomcoding-core --test gauntlet_local_live -- --ignored --nocapture`
//!
//! Cabe señalar que esta prueba hace de «padre» determinista: arranca el Gauntlet, llama a `advance`
//! hasta que llega a la aceptación final y **no acepta**; esa decisión es de una persona. Vuelca un
//! informe JSON con las fases, sus tiempos y las métricas de cada hijo.
//!
//! Variables: `LOOMCODING_LIVE_CWD` (directorio del proyecto), `LOOMCODING_LIVE_ARTIFACT` (ruta
//! relativa), `LOOMCODING_LIVE_BAR` (rutas relativas separadas por comas), `LOOMCODING_LIVE_GOAL_FILE`
//! (fichero con el objetivo), `LOOMCODING_LIVE_REPORT` (ruta del informe JSON). Opcionales:
//! `LOOMCODING_LIVE_BASE_URL` (por defecto `http://127.0.0.1:8091/v1`), `LOOMCODING_LIVE_MODEL`
//! (`qwen3.5-9b`), `LOOMCODING_LIVE_MAX_ROUNDS` (2), `LOOMCODING_LIVE_TIMEOUT_MIN` (60).

use loomcoding_core::config::{LoomcodingConfig, ProviderConfig, ProviderKind};
use loomcoding_core::event::{EventBus, LiveEvent};
use loomcoding_core::job::LocalJobRegistry;
use loomcoding_core::permission::PermissionService;
use loomcoding_core::provider::openai::OpenAiCompatProvider;
use loomcoding_core::provider::Provider;
use loomcoding_core::registry::{AgentRegistry, BuiltInSource, FileSource, ProfileLevel};
use loomcoding_core::session::sqlite::SqliteStore;
use loomcoding_core::session::SessionStore;
use loomcoding_core::subagent::InProcessSubagentLauncher;
use loomcoding_core::tools::gauntlet::GauntletTool;
use loomcoding_core::tools::{project_registry, Tool, ToolCtx};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("falta la variable {name}"))
}

fn var_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

const BUILDER_PROFILE: &str = "---\nname: constructor\ndescription: Implementa la pieza pedida\n---\n\nEres el constructor. Haces exactamente lo que pide el objetivo, con ediciones pequeñas y sin borrar nada que no debas. Edita con `edit` o `multiedit`; no reescribas archivos existentes enteros ni crees ficheros auxiliares. Cuando termines, di en un mensaje escrito qué cambiaste.\n";

const CRITIC_PROFILE: &str = "---\nname: critico\ndescription: Juzga dos candidatos a ciegas\n---\n\nEres un crítico independiente. Lees cada candidato UNA sola vez con `view` (dos lecturas en total) y juzgas solo lo que contienen. No uses `grep`, `bash` ni más lecturas: tras las dos lecturas respondes. Termina siempre con el objeto JSON que se te pide en la última línea; sin ese JSON tu trabajo no sirve.\n";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "necesita un servidor local en marcha y variables LOOMCODING_LIVE_*"]
async fn gauntlet_against_a_local_server() {
    let cwd = PathBuf::from(var("LOOMCODING_LIVE_CWD"));
    let artifact = var("LOOMCODING_LIVE_ARTIFACT");
    let bar: Vec<String> = var("LOOMCODING_LIVE_BAR")
        .split(',')
        .map(|path| path.trim().to_string())
        .collect();
    let goal = std::fs::read_to_string(var("LOOMCODING_LIVE_GOAL_FILE")).expect("objetivo");
    let report_path = PathBuf::from(var("LOOMCODING_LIVE_REPORT"));
    let base_url = var_or("LOOMCODING_LIVE_BASE_URL", "http://127.0.0.1:8091/v1");
    let model = var_or("LOOMCODING_LIVE_MODEL", "qwen3.5-9b");
    let max_rounds: u64 = var_or("LOOMCODING_LIVE_MAX_ROUNDS", "2").parse().unwrap();
    let timeout = Duration::from_secs(
        60 * var_or("LOOMCODING_LIVE_TIMEOUT_MIN", "60")
            .parse::<u64>()
            .unwrap(),
    );

    let scratch = tempfile::tempdir().unwrap();
    let profiles = scratch.path().join("perfiles");
    std::fs::create_dir_all(&profiles).unwrap();
    std::fs::write(profiles.join("constructor.md"), BUILDER_PROFILE).unwrap();
    std::fs::write(profiles.join("critico.md"), CRITIC_PROFILE).unwrap();

    // La base de datos queda junto al informe para poder revisar los mensajes de cada hijo.
    let db_path = report_path.with_extension("db");
    let _ = std::fs::remove_file(&db_path);
    let store = Arc::new(SqliteStore::open(db_path.to_str().unwrap()).await.unwrap());
    let session = store.create_session("gauntlet-local-live").await.unwrap();
    let provider: Arc<dyn Provider> = Arc::new(OpenAiCompatProvider::new(&base_url, "sk-local"));
    let config = LoomcodingConfig {
        provider: ProviderConfig {
            kind: ProviderKind::OpenAiCompat,
            base_url: base_url.clone(),
            api_key_env: "LLAMA_API_KEY".into(),
        },
        model: model.clone(),
        max_steps: 30,
        effort: None,
    };
    let bus = EventBus::new(1024);
    let mut events = bus.subscribe();
    let registry = Arc::new(AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(profiles, ProfileLevel::Global)),
    ]));
    let jobs = Arc::new(LocalJobRegistry::default());
    let launcher = Arc::new(
        InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(project_registry(std::collections::HashMap::new())),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config)),
            cwd.clone(),
            jobs.clone(),
            bus.clone(),
        )
        .with_registry(registry),
    );
    let ctx = || ToolCtx {
        cwd: cwd.clone(),
        store: Some(store.clone()),
        permission: Arc::new(PermissionService::allow_all()),
        session,
        jobs: jobs.clone(),
        bus: bus.clone(),
        subagents: Some(launcher.clone()),
    };

    let started = Instant::now();
    let start = GauntletTool
        .run(
            ctx(),
            serde_json::json!({
                "action": "start", "goal": goal, "artifact": artifact, "bar": bar,
                "builder": "constructor", "critic": "critico", "max_rounds": max_rounds
            }),
            CancellationToken::new(),
        )
        .await
        .expect("start");
    assert!(!start.is_error, "{}", start.content);

    let mut log: Vec<serde_json::Value> = Vec::new();
    let mut last_phase = String::new();
    let final_phase = loop {
        while let Ok(event) = events.try_recv() {
            if let LiveEvent::GauntletChanged {
                summary,
                actor,
                detail,
            } = event
            {
                last_phase = summary.phase.as_str().to_string();
                log.push(serde_json::json!({
                    "t_s": started.elapsed().as_secs(), "phase": last_phase,
                    "round": summary.round, "actor": actor, "detail": detail,
                }));
            }
        }
        let gauntlet = store.latest_gauntlet(session).await.unwrap().unwrap();
        let phase = gauntlet.phase;
        if phase.is_terminal() || phase.as_str() == "awaiting_final_acceptance" {
            break phase.as_str().to_string();
        }
        assert!(
            started.elapsed() < timeout,
            "tiempo agotado en la fase {last_phase}"
        );
        let _ = GauntletTool
            .run(
                ctx(),
                serde_json::json!({"action": "advance"}),
                CancellationToken::new(),
            )
            .await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    };

    let gauntlet = store.latest_gauntlet(session).await.unwrap().unwrap();
    let children: Vec<serde_json::Value> = store
        .list_subagents(session)
        .await
        .unwrap()
        .into_iter()
        .map(|child| {
            serde_json::json!({
                "label": child.label, "agent": child.agent,
                "state": format!("{:?}", child.status.state),
                "status": format!("{:?}", child.status), "steps": child.steps,
                "tool_calls": child.tool_calls, "tokens_in": child.tokens_in,
                "tokens_out": child.tokens_out,
            })
        })
        .collect();
    let report = serde_json::json!({
        "final_phase": final_phase, "seconds": started.elapsed().as_secs(),
        "events": log, "children": children,
        "gauntlet": serde_json::to_value(&gauntlet).unwrap(),
    });
    std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!(
        "gauntlet terminado en fase `{final_phase}` tras {} s; informe en {}",
        started.elapsed().as_secs(),
        report_path.display()
    );
}
