use super::*;

const PADRE: SessionId = SessionId(1);

/// Padre abierto con los hijos `(id, estado)` ya cargados.
fn app_con_hijos(hijos: &[(i64, TaskState)]) -> App {
    let mut app = App::new();
    app.load_session(PADRE, Vec::new());
    app.load_subagents(
        PADRE,
        hijos
            .iter()
            .map(|&(id, estado)| child_descriptor(id, estado, PADRE))
            .collect(),
    );
    app
}

fn cambia(app: &mut App, id: i64, estado: TaskState) {
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        id, estado, PADRE,
    )));
}

#[test]
fn el_despacho_cubre_los_ocho_estados_de_a2a() {
    for estado in TaskState::ALL {
        let esperado = match estado {
            TaskState::InputRequired => ChildDispatch::Answer,
            TaskState::Submitted | TaskState::Working => ChildDispatch::Queue,
            _ => ChildDispatch::Reject,
        };
        assert_eq!(child_dispatch(estado), esperado, "{estado:?}");
    }
}

#[test]
fn el_destinatario_por_defecto_es_la_sesion_abierta() {
    let app = app_con_hijos(&[(7, TaskState::Working)]);
    assert_eq!(app.composer_target, ComposerTarget::Open);
    assert_eq!(app.composer_target_label(), None);
}

#[test]
fn focus_fija_como_destinatario_a_un_hijo_directo() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    assert!(app.focus_child(SessionId(7)));
    assert_eq!(app.composer_target, ComposerTarget::Child(SessionId(7)));
    let etiqueta = app.composer_target_label().expect("hay destinatario");
    assert!(etiqueta.contains("hijo-7"), "{etiqueta}");
    assert!(etiqueta.contains("working"), "{etiqueta}");
}

#[test]
fn focus_rechaza_un_hijo_desconocido_y_avisa() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    assert!(!app.focus_child(SessionId(99)));
    assert_eq!(app.composer_target, ComposerTarget::Open);
    assert!(app.slash_feedback.is_some());
}

#[test]
fn el_comando_focus_se_analiza_al_enviar() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.composer.insert_str("/focus 7");
    assert!(app.try_focus_command());
    assert_eq!(app.composer_target, ComposerTarget::Child(SessionId(7)));
    assert!(app.composer.is_empty(), "la orden no es un mensaje");
}

#[test]
fn focus_con_argumento_invalido_es_un_error_y_no_un_mensaje() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.composer.insert_str("/focus abc");
    assert!(app.try_focus_command());
    assert_eq!(app.composer_target, ComposerTarget::Open);
    assert!(app.slash_feedback.is_some());
}

#[test]
fn focus_sin_argumento_vuelve_a_la_sesion_abierta() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("/focus");
    assert!(app.try_focus_command());
    assert_eq!(app.composer_target, ComposerTarget::Open);
}

#[test]
fn un_texto_que_solo_empieza_parecido_no_es_el_comando() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.composer.insert_str("/focused on the task");
    assert!(!app.try_focus_command());
    assert_eq!(app.composer.text(), "/focused on the task");
}

#[test]
fn ctrl_g_recorre_los_hijos_vivos_y_vuelve_a_la_sesion() {
    let mut app = app_con_hijos(&[
        (7, TaskState::Working),
        (8, TaskState::Completed),
        (9, TaskState::InputRequired),
    ]);
    app.cycle_composer_target();
    assert_eq!(app.composer_target, ComposerTarget::Child(SessionId(7)));
    app.cycle_composer_target();
    assert_eq!(
        app.composer_target,
        ComposerTarget::Child(SessionId(9)),
        "el hijo terminado se salta"
    );
    app.cycle_composer_target();
    assert_eq!(app.composer_target, ComposerTarget::Open);
}

#[test]
fn ctrl_g_sin_hijos_vivos_avisa_y_no_cambia() {
    let mut app = app_con_hijos(&[(8, TaskState::Completed)]);
    app.cycle_composer_target();
    assert_eq!(app.composer_target, ComposerTarget::Open);
    assert!(app.slash_feedback.is_some());
}

#[test]
fn enviar_a_un_hijo_que_pide_entrada_responde_a_su_pregunta() {
    let mut app = app_con_hijos(&[(7, TaskState::InputRequired)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("sí, adelante");
    assert!(app.submit_to_child());
    assert_eq!(
        app.answer_subagent_requested,
        Some((SessionId(7), "sí, adelante".to_string()))
    );
    assert!(app.composer.is_empty());
}

#[test]
fn enviar_a_un_hijo_en_curso_lo_encola_sin_enviar_nada() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("cuando puedas");
    assert!(app.submit_to_child());
    assert_eq!(app.answer_subagent_requested, None);
    assert_eq!(app.queued_for_child(SessionId(7)), 1);
    assert!(app.composer.is_empty());
}

#[test]
fn lo_encolado_se_entrega_cuando_el_hijo_pide_entrada() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("la respuesta");
    app.submit_to_child();
    cambia(&mut app, 7, TaskState::InputRequired);
    assert_eq!(
        app.answer_subagent_requested,
        Some((SessionId(7), "la respuesta".to_string()))
    );
    assert_eq!(app.queued_for_child(SessionId(7)), 0);
}

#[test]
fn lo_encolado_se_descarta_con_aviso_si_el_hijo_termina() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("tarde");
    app.submit_to_child();
    cambia(&mut app, 7, TaskState::Completed);
    assert_eq!(app.queued_for_child(SessionId(7)), 0);
    assert_eq!(app.answer_subagent_requested, None);
    assert!(app.slash_feedback.is_some(), "no se pierde en silencio");
}

#[test]
fn enviar_a_un_hijo_terminado_se_rechaza_y_conserva_el_texto() {
    let mut app = app_con_hijos(&[(7, TaskState::Completed)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("una más");
    assert!(app.submit_to_child());
    assert_eq!(app.answer_subagent_requested, None);
    assert_eq!(app.composer.text(), "una más", "el borrador no se pierde");
    assert!(app.slash_feedback.is_some());
}

#[test]
fn enviar_a_un_hijo_que_pide_credenciales_se_rechaza() {
    let mut app = app_con_hijos(&[(7, TaskState::AuthRequired)]);
    app.focus_child(SessionId(7));
    app.composer.insert_str("mi clave");
    assert!(app.submit_to_child());
    assert_eq!(app.answer_subagent_requested, None);
    assert_eq!(app.composer.text(), "mi clave");
}

#[test]
fn la_cola_por_hijo_esta_acotada() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    for n in 0..(MAX_CHILD_QUEUE + 3) {
        app.composer.insert_str(&format!("mensaje {n}"));
        app.submit_to_child();
    }
    assert_eq!(app.queued_for_child(SessionId(7)), MAX_CHILD_QUEUE);
    assert!(
        !app.composer.is_empty(),
        "el que no cabe se queda en el borrador"
    );
}

#[test]
fn sin_destinatario_de_hijo_submit_to_child_no_actua() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.composer.insert_str("para el padre");
    assert!(!app.submit_to_child());
    assert_eq!(app.composer.text(), "para el padre");
}

#[test]
fn cambiar_de_sesion_devuelve_el_destinatario_a_la_sesion_abierta() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    app.load_session(SessionId(7), Vec::new());
    assert_eq!(app.composer_target, ComposerTarget::Open);
}

#[test]
fn si_el_hijo_destino_desaparece_el_destinatario_vuelve_a_la_sesion() {
    let mut app = app_con_hijos(&[(7, TaskState::Working)]);
    app.focus_child(SessionId(7));
    app.load_subagents(PADRE, Vec::new());
    assert_eq!(app.composer_target, ComposerTarget::Open);
}
