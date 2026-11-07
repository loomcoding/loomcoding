//! Etiqueta del modo y flujo activo en la cabecera: ODD (fase genérica de `FlowChanged`), los
//! flujos pesados (`gauntlet`, `development_workflow`) y su precedencia.

use super::*;
use loomcoding_core::config::OddConfig;
use loomcoding_core::flow::FlowSummary;
use loomcoding_core::gauntlet::{GauntletId, GauntletPhase, GauntletSummary};
use loomcoding_core::session::SessionId;
use loomcoding_core::workflow::{WorkflowId, WorkflowPhase, WorkflowSummary};

/// Aplicación de una sesión con ODD encendido por configuración.
fn app_con_odd() -> App {
    let mut app = App::new().with_odd_config(OddConfig { enabled: true });
    app.session_id = Some(SessionId(7));
    app
}

/// Evento de ODD, tal como lo publica su observador.
fn odd(fase: &str, terminal: bool) -> LiveEvent {
    LiveEvent::FlowChanged {
        summary: FlowSummary {
            kind: "odd".into(),
            id: 0,
            phase: fase.into(),
            round: 0,
            max_rounds: 0,
            terminal,
            label: format!("ODD · {fase}"),
        },
        actor: String::new(),
        detail: String::new(),
    }
}

/// Evento de un flujo genérico que no es ODD.
fn generico(terminal: bool) -> LiveEvent {
    LiveEvent::FlowChanged {
        summary: FlowSummary {
            kind: "prueba".into(),
            id: 4,
            phase: "checking".into(),
            round: 1,
            max_rounds: 3,
            terminal,
            label: "prueba · checking".into(),
        },
        actor: String::new(),
        detail: String::new(),
    }
}

fn gauntlet(fase: GauntletPhase, ronda: u8) -> LiveEvent {
    LiveEvent::GauntletChanged {
        summary: GauntletSummary {
            id: GauntletId(3),
            parent: SessionId(7),
            phase: fase,
            round: ronda,
            max_rounds: 2,
            builder: "constructor".into(),
            critic: "critico".into(),
            active_child: None,
            updated_at_ms: 0,
        },
        actor: String::new(),
        detail: String::new(),
    }
}

fn workflow(fase: WorkflowPhase) -> LiveEvent {
    LiveEvent::DevelopmentWorkflowChanged(WorkflowSummary {
        id: WorkflowId(3),
        parent: SessionId(7),
        phase: fase,
        planner: "planner".into(),
        implementer: "builder".into(),
        reviewer: "reviewer".into(),
        active_child: None,
        correction_round: 1,
        spec_path: None,
        plan_path: None,
        updated_at_ms: 0,
    })
}

/// Texto completo de la etiqueta, o `None` si no hay.
fn etiqueta(app: &App) -> Option<String> {
    app.flow_label().map(|e| e.completa.clone())
}

#[test]
fn con_odd_encendido_la_etiqueta_base_es_odd_sin_fase() {
    let app = app_con_odd();
    assert_eq!(etiqueta(&app).as_deref(), Some("ODD"));
}

#[test]
fn flow_changed_de_odd_fija_la_etiqueta_con_su_fase() {
    let mut app = app_con_odd();
    app.apply(odd("implementing", false));
    let fase = app.flow_label().expect("hay etiqueta");
    assert_eq!(fase.completa, "ODD · implementing");
    assert_eq!(fase.corta, "ODD");
}

#[test]
fn flow_changed_de_odd_terminal_vuelve_a_la_base() {
    let mut app = app_con_odd();
    app.apply(odd("implementing", false));
    app.apply(odd("completed", true));
    assert_eq!(etiqueta(&app).as_deref(), Some("ODD"));
}

#[test]
fn turn_ended_devuelve_odd_a_la_base_sin_fase() {
    let mut app = app_con_odd();
    app.apply(odd("exploring", false));
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(etiqueta(&app).as_deref(), Some("ODD"));
}

#[test]
fn flujo_generico_no_terminal_pinta_su_etiqueta_y_terminal_la_borra() {
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(generico(false));
    assert_eq!(etiqueta(&app).as_deref(), Some("prueba · checking"));
    app.apply(generico(true));
    assert_eq!(etiqueta(&app), None);
}

#[test]
fn turn_ended_no_borra_un_flujo_pesado_activo() {
    let mut app = app_con_odd();
    app.apply(gauntlet(GauntletPhase::Building, 1));
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(
        etiqueta(&app).as_deref(),
        Some("gauntlet #3 · construyendo · ronda 1/2")
    );
}

#[test]
fn gauntlet_activo_manda_sobre_odd_y_al_acabar_vuelve_a_odd() {
    let mut app = app_con_odd();
    app.apply(odd("implementing", false));
    app.apply(gauntlet(GauntletPhase::Judging, 1));
    // ODD sigue cambiando de fase mientras el gauntlet corre: no se pinta.
    app.apply(odd("reviewing", false));
    assert_eq!(
        etiqueta(&app).as_deref(),
        Some("gauntlet #3 · juzgando · ronda 1/2")
    );
    app.apply(gauntlet(GauntletPhase::Completed, 1));
    assert_eq!(etiqueta(&app).as_deref(), Some("ODD · reviewing"));
}

#[test]
fn gauntlet_terminal_con_odd_apagado_no_deja_etiqueta() {
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(gauntlet(GauntletPhase::Building, 1));
    assert!(app.flow_label().is_some());
    app.apply(gauntlet(GauntletPhase::Completed, 1));
    assert_eq!(etiqueta(&app), None);
}

#[test]
fn workflow_activo_pinta_su_fase_y_acaba_a_odd() {
    let mut app = app_con_odd();
    app.apply(workflow(WorkflowPhase::AwaitingPlanApproval));
    assert_eq!(
        etiqueta(&app).as_deref(),
        Some("flujo #3 · aprobar spec+plan · ronda 1")
    );
    app.apply(workflow(WorkflowPhase::Completed));
    assert_eq!(etiqueta(&app).as_deref(), Some("ODD"));
}

#[test]
fn etiqueta_de_flujo_pesado_tiene_forma_corta_sin_fase() {
    let mut app = app_con_odd();
    app.apply(gauntlet(GauntletPhase::Building, 2));
    let e = app.flow_label().expect("hay etiqueta");
    assert_eq!(e.corta, "gauntlet #3 · ronda 2/2");
}

#[test]
fn odd_apagado_no_pinta_nada_ni_con_eventos_de_odd() {
    let mut app = App::new().with_odd_config(OddConfig { enabled: false });
    app.session_id = Some(SessionId(7));
    assert_eq!(etiqueta(&app), None);
    app.apply(odd("implementing", false));
    assert_eq!(etiqueta(&app), None);
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(etiqueta(&app), None);
}

#[test]
fn sin_configuracion_la_aplicacion_nueva_no_muestra_odd() {
    let app = App::new();
    assert_eq!(etiqueta(&app), None);
}
