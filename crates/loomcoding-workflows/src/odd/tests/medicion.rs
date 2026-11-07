//! Contadores de medición: cada acción sube su contador y ninguno más.

use super::*;
use crate::odd::gate::OddGate;
use crate::OddStats;
use loomcoding_core::agent::{CloseDecision, CloseGateCtx, TurnCloseGate};

fn ctx(e: &Entorno) -> CloseGateCtx {
    CloseGateCtx {
        session: e.sesion,
        cwd: e.proyecto.path().to_path_buf(),
        store: e.store.clone(),
    }
}

/// Marca `T1` con una llamada de `bash` ya registrada en la sesión.
fn evidencia_valida(feature: &str) -> Value {
    json!({
        "op": "done",
        "feature": feature,
        "id": "T1",
        "evidence": {"text": "cargo test: 1 passed", "call_id": "call_1"},
    })
}

#[tokio::test]
async fn crear_un_documento_sube_solo_documentos_creados() {
    let e = entorno().await;
    assert_eq!(e.estado.estadisticas(), OddStats::default());
    e.crear_basico("f", &["Tarea uno"]).await;
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            documentos_creados: 1,
            ..OddStats::default()
        }
    );
}

#[tokio::test]
async fn marcar_una_tarea_sube_solo_tareas_marcadas() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let antes = e.estado.estadisticas();
    let out = e.llamar(evidencia_valida("f")).await;
    assert!(!out.is_error, "{}", out.content);
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            tareas_marcadas: antes.tareas_marcadas + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn bloquear_una_tarea_sube_solo_tareas_bloqueadas() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let antes = e.estado.estadisticas();
    let out = e
        .llamar(json!({"op": "block", "feature": "f", "id": "T1", "reason": "falta la clave"}))
        .await;
    assert!(!out.is_error, "{}", out.content);
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            tareas_bloqueadas: antes.tareas_bloqueadas + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn reinyectar_el_cierre_sube_solo_cierres_bloqueados() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let gate = OddGate::new(e.estado.clone());
    let antes = e.estado.estadisticas();
    let decision = gate.check(&ctx(&e), 0).await;
    assert!(
        matches!(decision, CloseDecision::Reinject { .. }),
        "{decision:?}"
    );
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            cierres_bloqueados: antes.cierres_bloqueados + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn cierre_sin_progreso_sube_solo_cierres_sin_progreso() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno", "Tarea dos"]).await;
    let gate = OddGate::new(e.estado.clone());
    gate.check(&ctx(&e), 0).await;
    let antes = e.estado.estadisticas();
    let decision = gate.check(&ctx(&e), 1).await;
    assert!(
        matches!(decision, CloseDecision::Close { notice: Some(_) }),
        "{decision:?}"
    );
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            cierres_sin_progreso: antes.cierres_sin_progreso + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn necesita_tu_decision_sube_solo_cierres_por_decision() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let out = e
        .llamar(json!({"op": "block", "feature": "f", "id": "T1", "reason": "falta la clave"}))
        .await;
    assert!(!out.is_error, "{}", out.content);
    let gate = OddGate::new(e.estado.clone());
    let antes = e.estado.estadisticas();
    let decision = gate.check(&ctx(&e), 0).await;
    assert!(
        matches!(decision, CloseDecision::Close { notice: Some(_) }),
        "{decision:?}"
    );
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            cierres_por_decision: antes.cierres_por_decision + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn documento_ilegible_sube_solo_cierres_por_documento_ilegible() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    std::fs::write(e.ruta("f"), "esto no es un documento de feature\n").unwrap();
    let gate = OddGate::new(e.estado.clone());
    let antes = e.estado.estadisticas();
    let decision = gate.check(&ctx(&e), 0).await;
    assert!(
        matches!(decision, CloseDecision::Close { notice: Some(_) }),
        "{decision:?}"
    );
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            cierres_por_documento_ilegible: antes.cierres_por_documento_ilegible + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn evidencia_con_llamada_inexistente_sube_solo_rechazos_de_evidencia() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let antes = e.estado.estadisticas();
    let out = e
        .llamar(json!({
            "op": "done", "feature": "f", "id": "T1",
            "evidence": {"text": "ok", "call_id": "call_fantasma"},
        }))
        .await;
    assert!(out.is_error);
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            rechazos_de_evidencia: antes.rechazos_de_evidencia + 1,
            ..antes
        }
    );
}

#[tokio::test]
async fn done_sin_evidencia_sube_rechazos_de_evidencia_y_un_done_sin_id_no() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let antes = e.estado.estadisticas();
    let sin_evidencia = e
        .llamar(json!({"op": "done", "feature": "f", "id": "T1"}))
        .await;
    assert!(sin_evidencia.is_error);
    assert_eq!(
        e.estado.estadisticas(),
        OddStats {
            rechazos_de_evidencia: antes.rechazos_de_evidencia + 1,
            ..antes
        }
    );
    // Un `done` sin `id` es un error de forma, no de evidencia: el contador no sube.
    let antes = e.estado.estadisticas();
    let sin_id = e
        .llamar(json!({"op": "done", "feature": "f", "evidence": {"text": "ok", "call_id": "x"}}))
        .await;
    assert!(sin_id.is_error);
    assert_eq!(e.estado.estadisticas(), antes);
}
