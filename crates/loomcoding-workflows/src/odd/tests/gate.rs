//! Política de cierre de ODD: qué devuelve el cierre según el documento, el historial y el interruptor.

use super::*;
use crate::odd::gate::OddGate;
use crate::odd::state::OddState;
use loomcoding_core::agent::{CloseDecision, CloseGateCtx, TurnCloseGate, MAX_CLOSE_MESSAGE_BYTES};

fn ctx(e: &Entorno) -> CloseGateCtx {
    CloseGateCtx {
        session: e.sesion,
        cwd: e.proyecto.path().to_path_buf(),
        store: e.store.clone(),
    }
}

/// Desempaqueta una reinyección; cualquier otra cosa hace fallar la prueba con el veredicto recibido.
fn reinyecta(decision: CloseDecision) -> (String, Option<String>) {
    match decision {
        CloseDecision::Reinject { message, notice } => (message, notice),
        otra => panic!("debía reinyectar y devolvió {otra:?}"),
    }
}

/// Desempaqueta un cierre; cualquier otra cosa hace fallar la prueba.
fn cierra(decision: CloseDecision) -> Option<String> {
    match decision {
        CloseDecision::Close { notice } => notice,
        otra => panic!("debía cerrar y devolvió {otra:?}"),
    }
}

async fn consulta(gate: &OddGate, e: &Entorno, intento: u32) -> CloseDecision {
    gate.check(&ctx(e), intento).await
}

/// Marca una tarea como hecha, con una llamada de `bash` registrada en la sesión.
async fn marcar_hecha(e: &Entorno, feature: &str, id: &str) {
    e.registrar_llamada(e.sesion, "llamada-bash", "bash").await;
    let out = e
        .llamar(json!({
            "op": "done",
            "feature": feature,
            "id": id,
            "evidence": {"text": "cargo test: 1 passed", "call_id": "llamada-bash"},
        }))
        .await;
    assert!(!out.is_error, "{}", out.content);
}

#[tokio::test]
async fn con_pendientes_reinyecta_con_la_lista() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno", "Tarea dos"]).await;
    let gate = OddGate::new(e.estado.clone());
    let (mensaje, aviso) = reinyecta(consulta(&gate, &e, 0).await);
    assert!(
        mensaje.contains("T1") && mensaje.contains("Tarea uno"),
        "{mensaje}"
    );
    assert!(
        mensaje.contains("T2") && mensaje.contains("Tarea dos"),
        "{mensaje}"
    );
    assert!(
        aviso
            .as_deref()
            .unwrap_or("")
            .contains("Tareas pendientes: 2"),
        "{aviso:?}"
    );
}

#[tokio::test]
async fn sin_pendientes_cierra() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno"]).await;
    marcar_hecha(&e, "demo", "T1").await;
    let gate = OddGate::new(e.estado.clone());
    assert_eq!(cierra(consulta(&gate, &e, 0).await), None);
}

#[tokio::test]
async fn todo_bloqueado_cierra_con_necesita_tu_decision() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno", "Tarea dos"]).await;
    for id in ["T1", "T2"] {
        let out = e
            .llamar(json!({"op": "block", "feature": "demo", "id": id, "reason": "falta la clave"}))
            .await;
        assert!(!out.is_error, "{}", out.content);
    }
    let gate = OddGate::new(e.estado.clone());
    let aviso = cierra(consulta(&gate, &e, 0).await).expect("debe haber aviso");
    assert!(aviso.contains("Necesita tu decisión"), "{aviso}");
    assert!(aviso.contains('2'), "{aviso}");
}

#[tokio::test]
async fn segunda_consulta_con_los_mismos_pendientes_cierra_sin_progreso() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno", "Tarea dos"]).await;
    let gate = OddGate::new(e.estado.clone());
    reinyecta(consulta(&gate, &e, 0).await);
    let aviso = cierra(consulta(&gate, &e, 1).await).expect("debe haber aviso");
    assert!(aviso.contains("sin progreso"), "{aviso}");
}

#[tokio::test]
async fn con_progreso_vuelve_a_reinyectar_solo_lo_pendiente() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno", "Tarea dos"]).await;
    let gate = OddGate::new(e.estado.clone());
    reinyecta(consulta(&gate, &e, 0).await);
    marcar_hecha(&e, "demo", "T1").await;
    let (mensaje, aviso) = reinyecta(consulta(&gate, &e, 1).await);
    assert!(mensaje.contains("T2"), "{mensaje}");
    assert!(
        !mensaje.contains("T1 "),
        "la tarea hecha no se lista: {mensaje}"
    );
    assert!(
        aviso
            .as_deref()
            .unwrap_or("")
            .contains("Tareas pendientes: 1"),
        "{aviso:?}"
    );
}

#[tokio::test]
async fn documento_roto_cierra_sin_panico_y_avisa() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno"]).await;
    std::fs::write(e.ruta("demo"), "esto no es un documento de feature\n").unwrap();
    let gate = OddGate::new(e.estado.clone());
    let aviso = cierra(consulta(&gate, &e, 0).await).expect("debe avisar");
    assert!(aviso.contains("ilegible"), "{aviso}");
}

#[tokio::test]
async fn documento_ausente_cierra() {
    let e = entorno().await;
    e.estado.recordar(e.sesion, "fantasma");
    let gate = OddGate::new(e.estado.clone());
    assert_eq!(cierra(consulta(&gate, &e, 0).await), None);
}

#[tokio::test]
async fn interruptor_apagado_cierra_aunque_haya_pendientes() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno"]).await;
    let gate = OddGate::new(e.estado.clone());
    e.estado.set_enabled(false);
    assert_eq!(cierra(consulta(&gate, &e, 0).await), None);
    e.estado.set_enabled(true);
    reinyecta(consulta(&gate, &e, 0).await);
}

#[tokio::test]
async fn sin_feature_en_memoria_recupera_la_del_historial_tras_reinicio() {
    let e = entorno().await;
    e.crear_basico("demo", &["Tarea uno"]).await;
    // Un proceso nuevo: memoria vacía, pero el historial de la sesión conserva la llamada.
    let llamada = Message::assistant(vec![ContentPart::ToolCall {
        id: "odd-1".into(),
        name: "odd_track".into(),
        input: json!({"op": "create", "feature": "demo"}),
    }]);
    e.store.append_message(e.sesion, &llamada).await.unwrap();
    let gate = OddGate::new(OddState::new());
    let (mensaje, _) = reinyecta(consulta(&gate, &e, 0).await);
    assert!(mensaje.contains("Tarea uno"), "{mensaje}");
}

#[tokio::test]
async fn sin_feature_ni_en_historial_cierra() {
    let e = entorno().await;
    let gate = OddGate::new(OddState::new());
    assert_eq!(cierra(consulta(&gate, &e, 0).await), None);
}

#[tokio::test]
async fn feature_con_nombre_invalido_en_el_historial_cierra() {
    let e = entorno().await;
    let llamada = Message::assistant(vec![ContentPart::ToolCall {
        id: "odd-1".into(),
        name: "odd_track".into(),
        input: json!({"op": "read", "feature": "../fuera"}),
    }]);
    e.store.append_message(e.sesion, &llamada).await.unwrap();
    let gate = OddGate::new(OddState::new());
    assert_eq!(cierra(consulta(&gate, &e, 0).await), None);
}

#[tokio::test]
async fn lista_larga_de_pendientes_produce_un_mensaje_acotado() {
    let e = entorno().await;
    let textos: Vec<String> = (0..60).map(|_| "t".repeat(300)).collect();
    let out = e
        .llamar(json!({
            "op": "create",
            "feature": "grande",
            "objective": "objetivo",
            "tasks": textos,
        }))
        .await;
    assert!(!out.is_error, "{}", out.content);
    let gate = OddGate::new(e.estado.clone());
    let (mensaje, _) = reinyecta(consulta(&gate, &e, 0).await);
    assert!(
        mensaje.len() <= MAX_CLOSE_MESSAGE_BYTES,
        "{}",
        mensaje.len()
    );
    assert!(mensaje.contains("(+50 más)"), "{mensaje}");
}
