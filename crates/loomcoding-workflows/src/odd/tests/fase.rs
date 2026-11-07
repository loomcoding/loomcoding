//! Fase de ODD inferida de las herramientas (`phase_of`) y su observador (`OddObserver`).

use crate::odd::observer::OddObserver;
use crate::odd::phase::{phase_of, OddPhase};
use crate::odd::state::OddState;
use loomcoding_core::agent::{ToolObserveCtx, ToolObserver};
use loomcoding_core::session::SessionId;
use serde_json::{json, Value};

#[test]
fn tabla_de_herramientas_a_fase() {
    let casos: [(&str, Value, Option<OddPhase>); 11] = [
        ("view", json!({"path": "a.rs"}), Some(OddPhase::Exploring)),
        ("grep", json!({"pattern": "x"}), Some(OddPhase::Exploring)),
        (
            "glob",
            json!({"pattern": "*.rs"}),
            Some(OddPhase::Exploring),
        ),
        ("ls", json!({}), Some(OddPhase::Exploring)),
        (
            "edit",
            json!({"path": "a.rs"}),
            Some(OddPhase::Implementing),
        ),
        (
            "write",
            json!({"path": "a.rs"}),
            Some(OddPhase::Implementing),
        ),
        ("multiedit", json!({}), Some(OddPhase::Implementing)),
        ("ask_user", json!({}), Some(OddPhase::Deciding)),
        ("odd_track", json!({"op": "list"}), Some(OddPhase::Planning)),
        ("web", json!({}), None),
        ("todo_write", json!({}), None),
    ];
    for (herramienta, args, esperada) in casos {
        assert_eq!(phase_of(herramienta, &args), esperada, "{herramienta}");
    }
}

#[test]
fn bash_con_un_comprobador_conocido_es_comprobando() {
    let ordenes = [
        "cargo test -p loomcoding-core",
        "cargo build",
        "cargo check",
        "cargo clippy -- -D warnings",
        "cargo fmt --check",
        "npm test",
        "pytest -q",
        "go test ./...",
        "make test",
    ];
    for orden in ordenes {
        assert_eq!(
            phase_of("bash", &json!({ "command": orden })),
            Some(OddPhase::Checking),
            "{orden}"
        );
    }
}

#[test]
fn bash_ambiguo_no_cambia_la_fase() {
    for orden in ["ls -la", "echo hola", "git status"] {
        assert_eq!(
            phase_of("bash", &json!({ "command": orden })),
            None,
            "{orden}"
        );
    }
    assert_eq!(phase_of("bash", &json!({})), None, "sin orden");
}

#[test]
fn las_fases_tienen_su_nombre_en_ingles() {
    assert_eq!(OddPhase::Exploring.as_str(), "exploring");
    assert_eq!(OddPhase::Implementing.as_str(), "implementing");
    assert_eq!(OddPhase::Checking.as_str(), "checking");
    assert_eq!(OddPhase::Deciding.as_str(), "deciding");
    assert_eq!(OddPhase::Planning.as_str(), "planning");
    assert_eq!(OddPhase::Idle.as_str(), "idle");
}

fn ctx(sesion: i64, herramienta: &str, args: Value) -> ToolObserveCtx {
    ToolObserveCtx {
        session: SessionId(sesion),
        name: herramienta.into(),
        input: args,
    }
}

fn fase_de(resumen: Option<loomcoding_core::flow::FlowSummary>) -> Option<String> {
    resumen.map(|s| s.phase)
}

#[tokio::test]
async fn no_publica_dos_veces_la_misma_fase_seguida() {
    let observador = OddObserver::new(OddState::new());
    let primera = observador.observe(&ctx(1, "view", json!({}))).await;
    assert_eq!(fase_de(primera).as_deref(), Some("exploring"));
    assert!(observador
        .observe(&ctx(1, "grep", json!({})))
        .await
        .is_none());
}

#[tokio::test]
async fn publica_al_cambiar_y_al_volver_a_una_anterior() {
    let observador = OddObserver::new(OddState::new());
    assert_eq!(
        fase_de(observador.observe(&ctx(1, "view", json!({}))).await).as_deref(),
        Some("exploring")
    );
    assert_eq!(
        fase_de(observador.observe(&ctx(1, "edit", json!({}))).await).as_deref(),
        Some("implementing")
    );
    // Una herramienta sin fase no cambia nada.
    assert!(observador
        .observe(&ctx(1, "web", json!({})))
        .await
        .is_none());
    assert_eq!(
        fase_de(observador.observe(&ctx(1, "view", json!({}))).await).as_deref(),
        Some("exploring"),
        "volver a una fase anterior se publica"
    );
}

#[tokio::test]
async fn con_odd_apagado_nunca_publica() {
    let estado = OddState::new();
    estado.set_enabled(false);
    let observador = OddObserver::new(estado);
    assert!(observador
        .observe(&ctx(1, "view", json!({})))
        .await
        .is_none());
    assert!(observador
        .observe(&ctx(1, "edit", json!({})))
        .await
        .is_none());
}

#[tokio::test]
async fn las_sesiones_no_comparten_la_ultima_fase() {
    let observador = OddObserver::new(OddState::new());
    assert!(observador
        .observe(&ctx(1, "view", json!({})))
        .await
        .is_some());
    assert!(
        observador
            .observe(&ctx(2, "view", json!({})))
            .await
            .is_some(),
        "la sesión 2 aún no ha publicado nada"
    );
    assert!(observador
        .observe(&ctx(1, "view", json!({})))
        .await
        .is_none());
}

#[tokio::test]
async fn el_resumen_tiene_la_forma_del_flujo_de_odd() {
    let observador = OddObserver::new(OddState::new());
    let resumen = observador
        .observe(&ctx(1, "edit", json!({})))
        .await
        .expect("editar publica la fase");
    assert_eq!(resumen.kind, "odd");
    assert_eq!(resumen.id, 0);
    assert_eq!(resumen.phase, "implementing");
    assert_eq!(resumen.round, 0);
    assert_eq!(resumen.max_rounds, 0);
    assert!(!resumen.terminal);
    assert_eq!(resumen.label, "ODD · implementing");
}
