//! Operación `done` y la evidencia que la respalda.

use super::*;

/// Evidencia válida: texto corto y un identificador de llamada.
fn done(feature: &str, id: &str, texto: &str, call_id: &str) -> Value {
    json!({
        "op": "done",
        "feature": feature,
        "id": id,
        "evidence": {"text": texto, "call_id": call_id},
    })
}

#[tokio::test]
async fn done_con_llamada_de_herramienta_permitida_marca_la_casilla() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno", "Tarea dos", "Tarea tres"])
        .await;
    e.registrar_llamada(e.sesion, "call_bash", "bash").await;
    e.registrar_llamada(e.sesion, "call_start", "job_start")
        .await;
    e.registrar_llamada(e.sesion, "call_out", "job_output")
        .await;

    for (id, call) in [
        ("T1", "call_bash"),
        ("T2", "call_start"),
        ("T3", "call_out"),
    ] {
        let out = e.llamar(done("f", id, "cargo test: 12 passed", call)).await;
        assert!(!out.is_error, "{id}: {}", out.content);
    }
    let md = e.leer("f");
    assert!(
        md.contains("- [x] T1 Tarea uno — evidencia: cargo test: 12 passed [call: call_bash]"),
        "{md}"
    );
    let doc = parsear(&md);
    assert_eq!(
        doc.tasks[0].evidence.as_deref(),
        Some("cargo test: 12 passed [call: call_bash]")
    );
    assert!(doc.pending().is_empty());
}

#[tokio::test]
async fn done_valido_queda_anunciado_en_el_resultado() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let out = e.llamar(done("f", "T1", "tests verdes", "call_1")).await;
    assert!(!out.is_error);
    assert!(out.content.contains("T1"), "{}", out.content);
    assert!(out.content.contains("call_1"), "{}", out.content);
}

#[tokio::test]
async fn done_sin_evidencia_o_incompleta_se_rechaza_y_el_documento_queda_intacto() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let antes = e.leer("f");

    let sin_campo = e.llamar(json!({"op":"done","feature":"f","id":"T1"})).await;
    assert!(sin_campo.is_error);
    assert!(
        sin_campo.content.contains("evidence"),
        "{}",
        sin_campo.content
    );

    let sin_call = e
        .llamar(json!({"op":"done","feature":"f","id":"T1","evidence":{"text":"ok"}}))
        .await;
    assert!(sin_call.is_error);
    assert!(sin_call.content.contains("call_id"), "{}", sin_call.content);

    let texto_vacio = e.llamar(done("f", "T1", "   ", "call_1")).await;
    assert!(texto_vacio.is_error);
    assert!(
        texto_vacio.content.contains("vacío"),
        "{}",
        texto_vacio.content
    );

    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn done_con_llamada_inexistente_se_rechaza() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let antes = e.leer("f");
    let out = e.llamar(done("f", "T1", "ok", "call_fantasma")).await;
    assert!(out.is_error);
    assert!(
        out.content.contains("no existe en esta sesión"),
        "{}",
        out.content
    );
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn done_con_herramienta_no_permitida_se_rechaza_y_nombra_las_validas() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_view", "view").await;
    let antes = e.leer("f");
    let out = e.llamar(done("f", "T1", "ok", "call_view")).await;
    assert!(out.is_error);
    assert!(out.content.contains("view"), "{}", out.content);
    assert!(out.content.contains("bash"), "{}", out.content);
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn done_no_acepta_llamadas_de_otra_sesion() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let otra = e.store.create_session("proyecto").await.unwrap();
    e.registrar_llamada(otra, "call_ajena", "bash").await;
    let out = e.llamar(done("f", "T1", "ok", "call_ajena")).await;
    assert!(out.is_error);
    assert!(
        out.content.contains("no existe en esta sesión"),
        "{}",
        out.content
    );
}

#[tokio::test]
async fn done_sin_almacen_se_rechaza_con_motivo_y_no_inventa_exito() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let antes = e.leer("f");
    let out = e
        .llamar_con(e.ctx(false), done("f", "T1", "ok", "call_1"))
        .await;
    assert!(out.is_error);
    assert!(out.content.contains("historial"), "{}", out.content);
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn done_con_texto_desmesurado_se_rechaza_con_el_total_de_500_bytes() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let antes = e.leer("f");

    // 501 bytes de texto: rechazo directo.
    let mas = e.llamar(done("f", "T1", &"e".repeat(501), "call_1")).await;
    assert!(mas.is_error);
    assert!(mas.content.contains("500"), "{}", mas.content);

    // 490 bytes de texto más « [call: call_1]» (15 bytes) suman 505: el total guardado no cabe.
    let total = e.llamar(done("f", "T1", &"e".repeat(490), "call_1")).await;
    assert!(total.is_error);
    assert!(total.content.contains("500"), "{}", total.content);

    // 480 + 15 = 495: cabe.
    let cabe = e.llamar(done("f", "T1", &"e".repeat(480), "call_1")).await;
    assert!(!cabe.is_error, "{}", cabe.content);
    assert_eq!(
        parsear(&e.leer("f")).tasks[0]
            .evidence
            .as_ref()
            .map(String::len),
        Some(495)
    );
    assert_ne!(e.leer("f"), antes);
}

#[tokio::test]
async fn done_con_separador_en_la_evidencia_o_el_identificador_se_rechaza() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let antes = e.leer("f");
    let texto = e
        .llamar(done("f", "T1", "ok — evidencia: falsa", "call_1"))
        .await;
    assert!(texto.is_error);
    assert!(texto.content.contains("separador"), "{}", texto.content);
    let call = e
        .llamar(done("f", "T1", "ok", "call_1 — bloqueada: x"))
        .await;
    assert!(call.is_error);
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn done_de_tarea_inexistente_o_ya_hecha_se_rechaza() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    e.registrar_llamada(e.sesion, "call_1", "bash").await;
    let inexistente = e.llamar(done("f", "T7", "ok", "call_1")).await;
    assert!(inexistente.is_error);
    assert!(
        inexistente.content.contains("T7"),
        "{}",
        inexistente.content
    );

    assert!(!e.llamar(done("f", "T1", "ok", "call_1")).await.is_error);
    let otra_vez = e.llamar(done("f", "T1", "ok", "call_1")).await;
    assert!(otra_vez.is_error);
    assert!(
        otra_vez.content.contains("ya está hecha"),
        "{}",
        otra_vez.content
    );
}

/// Añade `n` mensajes de relleno a la sesión, para empujar las llamadas antiguas fuera de la ventana.
async fn relleno(e: &Entorno, n: usize) {
    for _ in 0..n {
        let mensaje = Message::user("relleno".to_string());
        e.store.append_message(e.sesion, &mensaje).await.unwrap();
    }
}

/// Historial: una llamada antigua, `505` mensajes de relleno y una llamada reciente. La ventana de
/// `VENTANA_EVIDENCIA` (500) deja fuera la antigua y dentro la reciente.
async fn historial_con_llamada_antigua_y_reciente(e: &Entorno) {
    e.registrar_llamada(e.sesion, "call_vieja", "bash").await;
    relleno(e, 505).await;
    e.registrar_llamada(e.sesion, "call_reciente", "bash").await;
}

#[tokio::test]
async fn una_llamada_dentro_de_la_ventana_reciente_es_valida() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    historial_con_llamada_antigua_y_reciente(&e).await;
    let out = e.llamar(done("f", "T1", "ok", "call_reciente")).await;
    assert!(!out.is_error, "{}", out.content);
}

#[tokio::test]
async fn una_llamada_fuera_de_la_ventana_reciente_se_rechaza_con_motivo() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    historial_con_llamada_antigua_y_reciente(&e).await;
    let antes = e.leer("f");
    let out = e.llamar(done("f", "T1", "ok", "call_vieja")).await;
    assert!(out.is_error);
    assert!(
        out.content.contains("llamada fuera de la ventana reciente"),
        "{}",
        out.content
    );
    assert_eq!(e.leer("f"), antes);
}
