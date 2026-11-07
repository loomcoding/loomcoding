//! Observador de herramientas: publicación de `FlowChanged`, aislamiento de pánico y plazo, y ausencia
//! de herencia en los hijos.

use super::*;
use crate::agent::tool_observer::{consult_observer, ToolObserveCtx, ToolObserver};
use crate::flow::FlowSummary;

/// Observador de prueba: devuelve siempre `resumen` y cuenta sus consultas.
struct ObservadorFijo {
    resumen: Option<FlowSummary>,
    llamadas: AtomicUsize,
}

impl ObservadorFijo {
    fn con(resumen: Option<FlowSummary>) -> Arc<Self> {
        Arc::new(Self {
            resumen,
            llamadas: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl ToolObserver for ObservadorFijo {
    async fn observe(&self, _ctx: &ToolObserveCtx) -> Option<FlowSummary> {
        self.llamadas.fetch_add(1, Ordering::SeqCst);
        self.resumen.clone()
    }
}

/// Observador que entra en pánico al consultarse.
struct ObservadorQueEntraEnPanico;

#[async_trait]
impl ToolObserver for ObservadorQueEntraEnPanico {
    async fn observe(&self, _ctx: &ToolObserveCtx) -> Option<FlowSummary> {
        panic!("fallo simulado del observador");
    }
}

/// Observador que tarda diez segundos, más que el plazo, antes de responder.
struct ObservadorLento;

#[async_trait]
impl ToolObserver for ObservadorLento {
    async fn observe(&self, _ctx: &ToolObserveCtx) -> Option<FlowSummary> {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Some(resumen("tarde"))
    }
}

fn resumen(fase: &str) -> FlowSummary {
    FlowSummary {
        kind: "flujo-de-prueba".into(),
        id: 0,
        phase: fase.into(),
        round: 0,
        max_rounds: 0,
        terminal: false,
        label: format!("prueba · {fase}"),
    }
}

/// Una ronda del modelo que pide las herramientas `nombres`, en ese orden.
fn ronda_de_herramientas(nombres: &[&str]) -> Vec<StreamChunk> {
    let mut chunks: Vec<StreamChunk> = nombres
        .iter()
        .enumerate()
        .map(|(i, nombre)| StreamChunk::ToolCallComplete {
            id: format!("c{i}"),
            name: (*nombre).into(),
            input: serde_json::json!({}),
        })
        .collect();
    chunks.push(StreamChunk::Done(FinishReason::ToolCalls));
    chunks
}

fn registro_con_view() -> ToolRegistry {
    let mut herramientas = ToolRegistry::new();
    herramientas.register(Arc::new(NamedTool("view")));
    herramientas
}

/// Guion de un turno con dos llamadas a `view` y una respuesta final.
async fn turno_con_dos_lecturas() -> EntornoDeTurno {
    entorno_de_turno(
        proveedor_de(
            "",
            false,
            vec![
                ronda_de_herramientas(&["view", "view"]),
                respuesta_con_consumo("fin", 1, 1),
            ],
        ),
        catalogo_de_ventana(100_000),
        &[],
        registro_con_view(),
    )
    .await
}

fn flujos(rx: &mut broadcast::Receiver<LiveEvent>) -> Vec<(FlowSummary, String, String)> {
    let mut out = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::FlowChanged {
            summary,
            actor,
            detail,
        } = evento
        {
            out.push((summary, actor, detail));
        }
    }
    out
}

#[tokio::test]
async fn cada_llamada_con_resumen_publica_un_flow_changed() {
    let e = turno_con_dos_lecturas().await;
    let observador = ObservadorFijo::con(Some(resumen("exploring")));
    let agente = e.agente.with_tool_observer(observador.clone());
    let mut rx = agente.bus.subscribe();
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(out.final_text, "fin");
    assert_eq!(observador.llamadas.load(Ordering::SeqCst), 2);
    let publicados = flujos(&mut rx);
    assert_eq!(
        publicados.len(),
        2,
        "una publicación por llamada con resumen"
    );
    for (summary, actor, detail) in &publicados {
        assert_eq!(summary, &resumen("exploring"));
        assert_eq!(actor, "loom");
        assert!(detail.is_empty());
    }
}

#[tokio::test]
async fn el_observador_sin_resumen_no_publica_nada() {
    let e = turno_con_dos_lecturas().await;
    let agente = e.agente.with_tool_observer(ObservadorFijo::con(None));
    let mut rx = agente.bus.subscribe();
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(out.final_text, "fin");
    assert!(flujos(&mut rx).is_empty());
}

#[tokio::test]
async fn un_observador_que_entra_en_panico_no_publica_y_el_turno_sigue() {
    let e = turno_con_dos_lecturas().await;
    let agente = e
        .agente
        .with_tool_observer(Arc::new(ObservadorQueEntraEnPanico));
    let mut rx = agente.bus.subscribe();
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .expect("el turno termina bien aunque el observador falle");
    assert_eq!(out.final_text, "fin");
    assert!(flujos(&mut rx).is_empty());
}

#[tokio::test]
async fn un_observador_que_tarda_mas_del_plazo_no_publica_y_el_turno_sigue() {
    let e = turno_con_dos_lecturas().await;
    let agente = e.agente.with_tool_observer(Arc::new(ObservadorLento));
    let mut rx = agente.bus.subscribe();
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(out.final_text, "fin");
    assert!(flujos(&mut rx).is_empty());
}

#[tokio::test]
async fn sin_observador_no_hay_flow_changed() {
    let e = turno_con_dos_lecturas().await;
    let mut rx = e.agente.bus.subscribe();
    e.agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    assert!(flujos(&mut rx).is_empty());
}

#[tokio::test]
async fn un_hijo_construido_sin_observador_no_publica_flow_changed() {
    // Así se construye un hijo local: `AgentLoop::new`, sin `with_tool_observer`.
    let e = turno_con_dos_lecturas().await;
    let config = e.agente.config.read().unwrap().clone();
    let hijo = AgentLoop::new(
        e.proveedor.clone(),
        e.store.clone(),
        Arc::new(registro_con_view()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        config,
        e.agente.cwd.clone(),
    );
    let mut rx = hijo.bus.subscribe();
    hijo.run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    assert!(flujos(&mut rx).is_empty());
}

#[test]
fn flow_changed_no_se_reenvia_a_los_hijos() {
    let evento = LiveEvent::FlowChanged {
        summary: resumen("exploring"),
        actor: "loom".into(),
        detail: String::new(),
    };
    assert!(!evento.is_child_relevant());
}

#[tokio::test]
async fn consulta_devuelve_el_resumen_del_observador() {
    let observador = ObservadorFijo::con(Some(resumen("checking")));
    let ctx = ToolObserveCtx {
        session: SessionId(1),
        name: "bash".into(),
        input: serde_json::json!({"command": "cargo test"}),
    };
    let decision = consult_observer(observador, ctx, &CancellationToken::new()).await;
    assert_eq!(decision, Some(resumen("checking")));
}

#[tokio::test]
async fn consulta_con_la_cancelacion_ya_hecha_no_llama_al_observador() {
    let observador = ObservadorFijo::con(Some(resumen("exploring")));
    let cancel = CancellationToken::new();
    cancel.cancel();
    let ctx = ToolObserveCtx {
        session: SessionId(1),
        name: "view".into(),
        input: serde_json::json!({}),
    };
    let decision = consult_observer(observador.clone(), ctx, &cancel).await;
    assert_eq!(decision, None);
    assert_eq!(observador.llamadas.load(Ordering::SeqCst), 0);
}
