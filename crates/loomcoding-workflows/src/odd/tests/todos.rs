//! Proyección del documento a la lista de tareas (`TodosChanged`) y al avance de la cabecera
//! (`FlowChanged`). Pruebas de la función pura, de la herramienta y del observador.

use super::*;
use crate::odd::doc::{Task, TaskStatus};
use crate::odd::observer::{resumen_de_flujo, OddObserver};
use crate::odd::todos::{progress, project, MAX_CONTENIDO_BYTES};
use crate::odd::tool::releer;
use loomcoding_core::agent::{ToolObserveCtx, ToolObserver};
use loomcoding_core::event::LiveEvent;
use loomcoding_core::flow::FlowSummary;
use loomcoding_core::todo::{TodoItem, TodoStatus};
use tokio::sync::broadcast::Receiver;

fn tarea(id: &str, texto: &str, status: TaskStatus, motivo: Option<&str>) -> Task {
    Task {
        id: id.into(),
        text: texto.into(),
        status,
        evidence: None,
        reason: motivo.map(str::to_string),
    }
}

fn doc_con(tareas: Vec<Task>) -> FeatureDoc {
    FeatureDoc {
        feature: "f".into(),
        objective: "o".into(),
        tasks: tareas,
        ..FeatureDoc::default()
    }
}

fn item(content: &str, status: TodoStatus) -> TodoItem {
    TodoItem {
        content: content.into(),
        status,
    }
}

/// Todo lo que haya en el bus hasta ahora.
fn drenar(rx: &mut Receiver<LiveEvent>) -> Vec<LiveEvent> {
    let mut eventos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        eventos.push(evento);
    }
    eventos
}

fn todos_publicados(eventos: &[LiveEvent]) -> Vec<Vec<TodoItem>> {
    eventos
        .iter()
        .filter_map(|e| match e {
            LiveEvent::TodosChanged { items, .. } => Some(items.clone()),
            _ => None,
        })
        .collect()
}

fn flujos_publicados(eventos: &[LiveEvent]) -> Vec<FlowSummary> {
    eventos
        .iter()
        .filter_map(|e| match e {
            LiveEvent::FlowChanged { summary, .. } => Some(summary.clone()),
            _ => None,
        })
        .collect()
}

// ---------- Función pura ----------

#[test]
fn project_mapea_hecha_pendiente_y_bloqueada_con_su_motivo() {
    let doc = doc_con(vec![
        tarea("T1", "uno", TaskStatus::Pending, None),
        tarea("T2", "dos", TaskStatus::Done, None),
        tarea("T3", "tres", TaskStatus::Blocked, Some("falta la clave")),
    ]);
    assert_eq!(
        project(&doc),
        vec![
            item("T1: uno", TodoStatus::Pending),
            item("T2: dos", TodoStatus::Completed),
            item("T3: tres [bloqueada: falta la clave]", TodoStatus::Pending),
        ]
    );
}

#[test]
fn project_no_usa_in_progress_y_respeta_el_orden() {
    let doc = doc_con(vec![
        tarea("T2", "b", TaskStatus::Done, None),
        tarea("T1", "a", TaskStatus::Pending, None),
    ]);
    let items = project(&doc);
    assert_eq!(items[0].content, "T2: b");
    assert_eq!(items[1].content, "T1: a");
    assert!(items.iter().all(|i| i.status != TodoStatus::InProgress));
    assert_eq!(project(&doc), items, "la proyección es estable");
}

#[test]
fn project_acota_cada_contenido_a_200_bytes_en_frontera_de_caracter() {
    let largo = "ñ".repeat(150); // 300 bytes
    let doc = doc_con(vec![
        tarea("T1", &largo, TaskStatus::Pending, None),
        tarea("T2", &largo, TaskStatus::Blocked, Some(&"é".repeat(150))),
    ]);
    let items = project(&doc);
    assert_eq!(MAX_CONTENIDO_BYTES, 200);
    assert!(items[0].content.len() <= MAX_CONTENIDO_BYTES);
    assert!(items[0].content.starts_with("T1: "));
    assert!(items[1].content.len() <= MAX_CONTENIDO_BYTES);
    assert!(
        items[1].content.ends_with(']') && items[1].content.contains(" [bloqueada: "),
        "el sufijo del bloqueo sobrevive al recorte: {}",
        items[1].content
    );
}

#[test]
fn project_con_mas_de_cincuenta_tareas_muestra_49_y_un_resumen() {
    let tareas: Vec<Task> = (1..=55)
        .map(|i| tarea(&format!("T{i}"), "x", TaskStatus::Pending, None))
        .collect();
    let items = project(&doc_con(tareas));
    assert_eq!(items.len(), 50);
    assert_eq!(items[48].content, "T49: x");
    assert_eq!(items[49].content, "(+6 más)");
    assert_eq!(items[49].status, TodoStatus::Pending);
}

#[test]
fn exactamente_cincuenta_tareas_no_llevan_resumen() {
    let tareas: Vec<Task> = (1..=50)
        .map(|i| tarea(&format!("T{i}"), "x", TaskStatus::Done, None))
        .collect();
    let items = project(&doc_con(tareas));
    assert_eq!(items.len(), 50);
    assert_eq!(items[49].content, "T50: x");
    assert_eq!(items[49].status, TodoStatus::Completed);
}

#[test]
fn progress_cuenta_las_hechas_y_el_total() {
    let doc = doc_con(vec![
        tarea("T1", "a", TaskStatus::Done, None),
        tarea("T2", "b", TaskStatus::Pending, None),
        tarea("T3", "c", TaskStatus::Blocked, Some("x")),
        tarea("T4", "d", TaskStatus::Done, None),
    ]);
    assert_eq!(progress(&doc), (2, 4));
    assert_eq!(progress(&FeatureDoc::default()), (0, 0));
}

#[test]
fn resumen_de_flujo_sin_avance_conserva_la_etiqueta_de_t4() {
    let sin = resumen_de_flujo("planning", None);
    assert_eq!(sin.label, "ODD · planning");
    assert_eq!((sin.round, sin.max_rounds), (0, 0));
    assert_eq!(
        resumen_de_flujo("planning", Some((0, 0))).label,
        "ODD · planning"
    );

    let con = resumen_de_flujo("planning", Some((2, 5)));
    assert_eq!(con.label, "ODD · planning (2/5)");
    assert_eq!((con.round, con.max_rounds), (2, 5));
    assert_eq!(con.kind, "odd");
    assert_eq!(con.phase, "planning");
    assert!(!con.terminal);
}

// ---------- Herramienta ----------

#[tokio::test]
async fn create_publica_la_lista_pendiente_y_el_avance_cero() {
    let e = entorno().await;
    let ctx = e.ctx(true);
    let mut rx = ctx.bus.subscribe();
    let out = e
        .llamar_con(
            ctx,
            json!({"op":"create","feature":"f","objective":"o","tasks":["uno","dos"]}),
        )
        .await;
    assert!(!out.is_error, "{}", out.content);
    let eventos = drenar(&mut rx);
    let listas = todos_publicados(&eventos);
    assert_eq!(listas.len(), 1);
    assert_eq!(
        listas[0],
        vec![
            item("T1: uno", TodoStatus::Pending),
            item("T2: dos", TodoStatus::Pending),
        ]
    );
    let flujos = flujos_publicados(&eventos);
    assert_eq!(flujos.len(), 1);
    assert_eq!(flujos[0].label, "ODD · planning (0/2)");
}

#[tokio::test]
async fn marcar_una_tarea_publica_completada_y_el_avance() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno", "Tarea dos"]).await;
    e.registrar_llamada(e.sesion, "c1", "bash").await;
    let ctx = e.ctx(true);
    let mut rx = ctx.bus.subscribe();
    let out = e
        .llamar_con(
            ctx,
            json!({"op":"done","feature":"f","id":"T1","evidence":{"text":"cargo test: 1 passed","call_id":"c1"}}),
        )
        .await;
    assert!(!out.is_error, "{}", out.content);
    let eventos = drenar(&mut rx);
    let listas = todos_publicados(&eventos);
    assert_eq!(listas.len(), 1);
    assert_eq!(listas[0][0], item("T1: Tarea uno", TodoStatus::Completed));
    assert_eq!(listas[0][1], item("T2: Tarea dos", TodoStatus::Pending));
    let flujos = flujos_publicados(&eventos);
    assert_eq!(flujos.len(), 1);
    assert_eq!(flujos[0].kind, "odd");
    assert_eq!(flujos[0].phase, "planning");
    assert_eq!(flujos[0].label, "ODD · planning (1/2)");
    assert_eq!((flujos[0].round, flujos[0].max_rounds), (1, 2));
    assert!(matches!(
        eventos.iter().find(|ev| matches!(ev, LiveEvent::FlowChanged { .. })),
        Some(LiveEvent::FlowChanged { actor, .. }) if actor.as_str() == "loom"
    ));
}

#[tokio::test]
async fn bloquear_y_reabrir_actualizan_la_lista_y_el_avance() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno", "Tarea dos"]).await;
    let ctx = e.ctx(true);
    let mut rx = ctx.bus.subscribe();

    let out = e
        .llamar_con(
            ctx.clone(),
            json!({"op":"block","feature":"f","id":"T1","reason":"falta la clave"}),
        )
        .await;
    assert!(!out.is_error, "{}", out.content);
    let eventos = drenar(&mut rx);
    let listas = todos_publicados(&eventos);
    assert_eq!(
        listas[0][0],
        item(
            "T1: Tarea uno [bloqueada: falta la clave]",
            TodoStatus::Pending
        )
    );
    // Bloquear no cambia el avance (la tarea bloqueada cuenta como no hecha): la etiqueta completa
    // es la misma que la de la creación y no se republica el flujo.
    assert!(flujos_publicados(&eventos).is_empty());

    let out = e
        .llamar_con(
            ctx,
            json!({"op":"reopen","feature":"f","id":"T1","reason":"clave recibida"}),
        )
        .await;
    assert!(!out.is_error, "{}", out.content);
    let listas = todos_publicados(&drenar(&mut rx));
    assert_eq!(listas[0][0], item("T1: Tarea uno", TodoStatus::Pending));
}

#[tokio::test]
async fn una_operacion_de_solo_lectura_no_publica_nada() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let ctx = e.ctx(true);
    let mut rx = ctx.bus.subscribe();
    let out = e.llamar_con(ctx, json!({"op":"read","feature":"f"})).await;
    assert!(!out.is_error, "{}", out.content);
    assert!(drenar(&mut rx).is_empty());
}

#[tokio::test]
async fn sin_documento_no_se_publica_nada() {
    let e = entorno().await;
    let ctx = e.ctx(true);
    let mut rx = ctx.bus.subscribe();
    let out = e
        .llamar_con(
            ctx,
            json!({"op":"block","feature":"nada","id":"T1","reason":"x"}),
        )
        .await;
    assert!(out.is_error);
    assert!(drenar(&mut rx).is_empty());
}

#[tokio::test]
async fn con_odd_apagado_se_publica_la_lista_pero_no_el_flujo() {
    let e = entorno().await;
    e.estado.set_enabled(false);
    let ctx = e.ctx(true);
    let mut rx = ctx.bus.subscribe();
    let out = e
        .llamar_con(
            ctx,
            json!({"op":"create","feature":"f","objective":"o","tasks":["uno"]}),
        )
        .await;
    assert!(!out.is_error, "{}", out.content);
    let eventos = drenar(&mut rx);
    assert_eq!(todos_publicados(&eventos).len(), 1);
    assert!(flujos_publicados(&eventos).is_empty());
}

#[test]
fn releer_devuelve_none_si_el_documento_no_parsea() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("f.md");
    std::fs::write(&ruta, "sin cabecera\n").unwrap();
    assert!(releer(&ruta).is_none());
    assert!(releer(&dir.path().join("no.md")).is_none());
    let doc = doc_con(vec![tarea("T1", "uno", TaskStatus::Pending, None)]);
    std::fs::write(&ruta, doc.to_markdown()).unwrap();
    assert_eq!(releer(&ruta).map(|d| d.tasks.len()), Some(1));
}

// ---------- Observador ----------

fn ctx_observado(sesion: i64, herramienta: &str, args: Value) -> ToolObserveCtx {
    ToolObserveCtx {
        session: loomcoding_core::session::SessionId(sesion),
        name: herramienta.into(),
        input: args,
    }
}

#[tokio::test]
async fn el_observador_republica_la_misma_fase_si_cambia_el_avance() {
    let estado = crate::OddState::new();
    let observador = OddObserver::new(estado.clone());
    let sesion = loomcoding_core::session::SessionId(1);

    let primera = observador
        .observe(&ctx_observado(1, "edit", json!({})))
        .await
        .expect("publica la fase");
    assert_eq!(primera.label, "ODD · implementing");

    estado.recordar_progreso(sesion, (0, 2));
    let con_avance = observador
        .observe(&ctx_observado(1, "edit", json!({})))
        .await
        .expect("mismo fase, distinto avance: se publica");
    assert_eq!(con_avance.label, "ODD · implementing (0/2)");
    assert_eq!((con_avance.round, con_avance.max_rounds), (0, 2));

    assert!(
        observador
            .observe(&ctx_observado(1, "edit", json!({})))
            .await
            .is_none(),
        "misma fase y mismo avance: no se duplica"
    );
}
