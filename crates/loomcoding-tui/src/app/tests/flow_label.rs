//! Etiqueta del modo y flujo activo en la cabecera: los modos llegan de la configuración, el flujo
//! persistente y su vuelta a la base, el flujo no persistente y la precedencia entre ambos.

use super::*;
use loomcoding_core::flow::FlowSummary;
use loomcoding_core::gauntlet::{GauntletId, GauntletPhase, GauntletSummary};
use loomcoding_core::session::SessionId;
use loomcoding_core::workflow::{WorkflowId, WorkflowPhase, WorkflowSummary};

/// Modo de prueba: `kind`, etiqueta base y su interruptor.
fn modo(kind: &str, label: &str, enabled: bool) -> FlowMode {
    FlowMode {
        kind: kind.into(),
        label: label.into(),
        enabled,
    }
}

/// Aplicación de una sesión con dos modos de prueba: `a` (encendido) y `b` (encendido).
fn app_con_modos() -> App {
    let mut app = App::new().with_flow_modes(vec![modo("a", "AAA", true), modo("b", "BBB", true)]);
    app.session_id = Some(SessionId(7));
    app
}

/// Evento persistente de un modo, tal como lo publica su observador.
fn persistente(kind: &str, label: &str, terminal: bool) -> LiveEvent {
    LiveEvent::FlowChanged {
        summary: FlowSummary {
            kind: kind.into(),
            id: 0,
            phase: label.into(),
            round: 0,
            max_rounds: 0,
            terminal,
            label: label.into(),
            persistent: true,
        },
        actor: String::new(),
        detail: String::new(),
    }
}

/// Evento de un flujo no persistente que no es de ningún modo.
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
            persistent: false,
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
fn con_un_modo_encendido_la_etiqueta_base_es_su_nombre_sin_fase() {
    let app = app_con_modos();
    assert_eq!(etiqueta(&app).as_deref(), Some("AAA"));
}

#[test]
fn la_base_es_la_del_primer_modo_encendido() {
    let app = App::new().with_flow_modes(vec![modo("a", "AAA", false), modo("b", "BBB", true)]);
    assert_eq!(etiqueta(&app).as_deref(), Some("BBB"));
}

#[test]
fn sin_modos_encendidos_no_hay_etiqueta() {
    let app = App::new().with_flow_modes(vec![modo("a", "AAA", false)]);
    assert_eq!(etiqueta(&app), None);
    let sin_modos = App::new();
    assert_eq!(etiqueta(&sin_modos), None);
}

#[test]
fn flow_changed_persistente_fija_la_etiqueta_con_su_fase() {
    let mut app = app_con_modos();
    app.apply(persistente("a", "AAA · implementing", false));
    let fase = app.flow_label().expect("hay etiqueta");
    assert_eq!(fase.completa, "AAA · implementing");
    assert_eq!(fase.corta, "AAA");
}

#[test]
fn flow_changed_persistente_terminal_vuelve_a_la_base() {
    let mut app = app_con_modos();
    app.apply(persistente("a", "AAA · implementing", false));
    app.apply(persistente("a", "AAA · completed", true));
    assert_eq!(etiqueta(&app).as_deref(), Some("AAA"));
}

#[test]
fn turn_ended_devuelve_el_persistente_a_la_base_de_su_modo() {
    let mut app = app_con_modos();
    app.apply(persistente("b", "BBB · exploring", false));
    assert_eq!(etiqueta(&app).as_deref(), Some("BBB · exploring"));
    app.apply(LiveEvent::TurnEnded);
    // La base de `b`, no la del primer modo (`a`).
    assert_eq!(etiqueta(&app).as_deref(), Some("BBB"));
}

#[test]
fn flujo_no_persistente_no_vuelve_a_la_base_de_otro() {
    let mut app = app_con_modos();
    app.apply(persistente("a", "AAA · fase", false));
    app.apply(generico(false));
    assert_eq!(etiqueta(&app).as_deref(), Some("prueba · checking"));
    // El fin de turno no toca el flujo no persistente activo.
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(etiqueta(&app).as_deref(), Some("prueba · checking"));
    // Al acabar, vuelve el persistente que estaba debajo.
    app.apply(generico(true));
    assert_eq!(etiqueta(&app).as_deref(), Some("AAA"));
}

#[test]
fn flujo_generico_no_persistente_pinta_su_etiqueta_y_terminal_la_borra() {
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(generico(false));
    assert_eq!(etiqueta(&app).as_deref(), Some("prueba · checking"));
    app.apply(generico(true));
    assert_eq!(etiqueta(&app), None);
}

#[test]
fn turn_ended_no_borra_un_flujo_pesado_activo() {
    let mut app = app_con_modos();
    app.apply(gauntlet(GauntletPhase::Building, 1));
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(
        etiqueta(&app).as_deref(),
        Some("gauntlet #3 · construyendo · ronda 1/2")
    );
}

#[test]
fn gauntlet_activo_manda_sobre_el_persistente_y_al_acabar_vuelve_a_el() {
    let mut app = app_con_modos();
    app.apply(persistente("a", "AAA · implementing", false));
    app.apply(gauntlet(GauntletPhase::Judging, 1));
    // El persistente sigue cambiando de fase mientras el gauntlet corre: no se pinta.
    app.apply(persistente("a", "AAA · reviewing", false));
    assert_eq!(
        etiqueta(&app).as_deref(),
        Some("gauntlet #3 · juzgando · ronda 1/2")
    );
    app.apply(gauntlet(GauntletPhase::Completed, 1));
    assert_eq!(etiqueta(&app).as_deref(), Some("AAA · reviewing"));
}

#[test]
fn gauntlet_terminal_sin_modos_no_deja_etiqueta() {
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(gauntlet(GauntletPhase::Building, 1));
    assert!(app.flow_label().is_some());
    app.apply(gauntlet(GauntletPhase::Completed, 1));
    assert_eq!(etiqueta(&app), None);
}

#[test]
fn workflow_activo_pinta_su_fase_y_acaba_a_la_base() {
    let mut app = app_con_modos();
    app.apply(workflow(WorkflowPhase::AwaitingPlanApproval));
    assert_eq!(
        etiqueta(&app).as_deref(),
        Some("flujo #3 · aprobar spec+plan · ronda 1")
    );
    app.apply(workflow(WorkflowPhase::Completed));
    assert_eq!(etiqueta(&app).as_deref(), Some("AAA"));
}

#[test]
fn etiqueta_de_flujo_pesado_tiene_forma_corta_sin_fase() {
    let mut app = app_con_modos();
    app.apply(gauntlet(GauntletPhase::Building, 2));
    let e = app.flow_label().expect("hay etiqueta");
    assert_eq!(e.corta, "gauntlet #3 · ronda 2/2");
}

#[test]
fn modo_apagado_ignora_sus_flow_changed_persistentes() {
    let mut app = App::new().with_flow_modes(vec![modo("a", "AAA", false), modo("b", "BBB", true)]);
    app.session_id = Some(SessionId(7));
    app.apply(persistente("a", "AAA · implementing", false));
    assert_eq!(etiqueta(&app).as_deref(), Some("BBB"));
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(etiqueta(&app).as_deref(), Some("BBB"));
}

#[test]
fn modo_desconocido_ignora_sus_flow_changed_persistentes() {
    let mut app = app_con_modos();
    app.apply(persistente("zeta", "ZZZ · fase", false));
    assert_eq!(etiqueta(&app).as_deref(), Some("AAA"));
}

#[test]
fn con_todos_los_modos_apagados_no_pinta_nada_ni_con_sus_eventos() {
    let mut app = App::new().with_flow_modes(vec![modo("a", "AAA", false)]);
    app.session_id = Some(SessionId(7));
    assert_eq!(etiqueta(&app), None);
    app.apply(persistente("a", "AAA · implementing", false));
    assert_eq!(etiqueta(&app), None);
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(etiqueta(&app), None);
}

#[test]
fn sin_configuracion_la_aplicacion_nueva_no_muestra_etiqueta() {
    let app = App::new();
    assert_eq!(etiqueta(&app), None);
}
