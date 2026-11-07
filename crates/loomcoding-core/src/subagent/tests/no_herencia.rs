//! Garantía A2A: un subagente local lanzado por el lanzador real no hereda nada del padre.
//!
//! El padre tiene política de cierre, observador de herramientas, contribuidor de prompt y una
//! herramienta solo de padre. Lanza un hijo con su herramienta `subagent`, que pasa por el lanzador
//! real, y se comprueba que el hijo no recibe ninguna de esas piezas, que su trabajo no publica
//! `FlowChanged` y que solo recorre estados A2A existentes.

use super::*;
use crate::agent::{
    AgentLoop, CloseDecision, CloseGateCtx, PromptContributor, ToolObserveCtx, ToolObserver,
    TurnCloseGate,
};
use crate::event::LiveEvent;
use crate::flow::FlowSummary;
use crate::provider::{Capabilities, Provider, ProviderRequest, ProviderStream};
use crate::session::SessionId;
use crate::task::TaskState;
use crate::tools::subagent::SubagentTool;
use crate::tools::{Tool, ToolCtx, ToolOutput};

/// Texto que solo debe aparecer en el prompt del padre.
const SECCION_DEL_PADRE: &str = "SECCION-SOLO-DEL-PADRE: esta sección nunca llega a un subagente.";

/// Proveedor con guion: cada petición consume el siguiente paso y, agotado, responde un texto final.
/// Graba el prompt de sistema y los nombres de herramienta de cada petición.
struct ProveedorConGuion {
    guion: std::sync::Mutex<std::collections::VecDeque<Vec<StreamChunk>>>,
    vistas: std::sync::Mutex<Vec<(String, Vec<String>)>>,
}

impl ProveedorConGuion {
    fn nuevo(guion: Vec<Vec<StreamChunk>>) -> Arc<Self> {
        Arc::new(Self {
            guion: std::sync::Mutex::new(guion.into()),
            vistas: std::sync::Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl Provider for ProveedorConGuion {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> crate::error::Result<ProviderStream> {
        let nombres = req.tools.iter().map(|t| t.name.clone()).collect();
        self.vistas
            .lock()
            .unwrap()
            .push((req.system.clone(), nombres));
        let paso = self.guion.lock().unwrap().pop_front().unwrap_or_else(|| {
            vec![
                StreamChunk::TextDelta("hecho".into()),
                StreamChunk::Done(FinishReason::Stop),
            ]
        });
        Ok(Box::pin(futures::stream::iter(paso)))
    }
}

/// Una ronda del modelo que pide una herramienta.
fn llamada(nombre: &str, input: serde_json::Value) -> Vec<StreamChunk> {
    vec![
        StreamChunk::ToolCallComplete {
            id: format!("id-{nombre}"),
            name: nombre.into(),
            input,
        },
        StreamChunk::Done(FinishReason::ToolCalls),
    ]
}

/// Una ronda del modelo que responde con texto final.
fn respuesta(texto: &str) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(texto.into()),
        StreamChunk::Done(FinishReason::Stop),
    ]
}

/// Herramienta que el hijo sí puede usar.
struct Eco;

#[async_trait]
impl Tool for Eco {
    fn name(&self) -> &str {
        "eco"
    }
    fn description(&self) -> &str {
        "devuelve su entrada"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn run(
        &self,
        _ctx: ToolCtx,
        input: serde_json::Value,
        _cancel: CancellationToken,
    ) -> crate::error::Result<ToolOutput> {
        Ok(ToolOutput::ok(input.to_string()))
    }
}

/// Herramienta que solo ve el agente principal.
struct SoloPadre;

#[async_trait]
impl Tool for SoloPadre {
    fn name(&self) -> &str {
        "solo_padre"
    }
    fn description(&self) -> &str {
        "solo el agente principal"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    fn parent_only(&self) -> bool {
        true
    }
    async fn run(
        &self,
        _ctx: ToolCtx,
        _input: serde_json::Value,
        _cancel: CancellationToken,
    ) -> crate::error::Result<ToolOutput> {
        Ok(ToolOutput::ok("ok"))
    }
}

/// Observador que anota la sesión de cada consulta y devuelve siempre un resumen.
#[derive(Default)]
struct ObservadorContador {
    consultas: std::sync::Mutex<Vec<SessionId>>,
}

#[async_trait]
impl ToolObserver for ObservadorContador {
    async fn observe(&self, ctx: &ToolObserveCtx) -> Option<FlowSummary> {
        self.consultas.lock().unwrap().push(ctx.session);
        Some(FlowSummary {
            kind: "prueba".into(),
            id: 0,
            phase: "explorando".into(),
            round: 0,
            max_rounds: 0,
            terminal: false,
            label: "prueba · explorando".into(),
        })
    }
}

/// Política que siempre pide reinyectar y anota la sesión de cada consulta.
#[derive(Default)]
struct PuertaQueReinyecta {
    consultas: std::sync::Mutex<Vec<SessionId>>,
}

#[async_trait]
impl TurnCloseGate for PuertaQueReinyecta {
    async fn check(&self, ctx: &CloseGateCtx, _attempt: u32) -> CloseDecision {
        self.consultas.lock().unwrap().push(ctx.session);
        CloseDecision::Reinject {
            message: "sigue".into(),
            notice: None,
        }
    }
}

/// Contribuidor con una sección distintiva, solo para el padre.
struct ContribuidorDelPadre;

impl PromptContributor for ContribuidorDelPadre {
    fn section(&self) -> Option<String> {
        Some(SECCION_DEL_PADRE.into())
    }
}

/// Lo que se observa tras el turno del padre y la vida de su hijo.
struct Observado {
    padre: SessionId,
    hijo: SessionId,
    cierres: Vec<SessionId>,
    observaciones: Vec<SessionId>,
    peticiones_padre: Vec<(String, Vec<String>)>,
    peticiones_hijo: Vec<(String, Vec<String>)>,
    eventos: Vec<LiveEvent>,
    estados_del_hijo: Vec<TaskState>,
    estado_final: TaskState,
}

/// Espera (con cota) a que `padre` tenga un hijo en estado terminal y devuelve su sesión y estado.
async fn esperar_hijo_terminal(store: &SqliteStore, padre: SessionId) -> (SessionId, TaskState) {
    for _ in 0..400 {
        let hijos = store.list_subagents(padre).await.unwrap();
        if let Some(hijo) = hijos.iter().find(|h| h.status.state.is_terminal()) {
            return (hijo.child, hijo.status.state);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("el hijo de {padre:?} no terminó");
}

/// Recoge los eventos del bus hasta que se queda en silencio durante un rato.
async fn recoger(rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>) -> Vec<LiveEvent> {
    let mut eventos = Vec::new();
    while let Ok(Ok(evento)) = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await {
        eventos.push(evento);
    }
    eventos
}

/// Padre con la política, el observador, el contribuidor y la herramienta de padre; lanza un hijo con
/// su herramienta `subagent` a través del lanzador real y devuelve lo que se pudo observar.
async fn lanzar_desde_el_padre() -> Observado {
    let dir = tempfile::tempdir().unwrap();
    let (store, sid) = tienda_de_prueba(dir.path(), "no_herencia.db").await;
    let bus = EventBus::new(512);
    let mut rx = bus.subscribe();

    // Hijo: llama a `eco` y luego responde.
    let proveedor_hijo = ProveedorConGuion::nuevo(vec![
        llamada("eco", serde_json::json!({})),
        respuesta("hijo listo"),
    ]);
    let mut herramientas_hijo = ToolRegistry::new();
    herramientas_hijo.register(Arc::new(Eco));
    herramientas_hijo.register(Arc::new(SoloPadre));
    let dyn_hijo: Arc<dyn Provider> = proveedor_hijo.clone();
    let lanzador = Arc::new(InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(dyn_hijo)),
        store.clone(),
        Arc::new(herramientas_hijo),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.path().to_path_buf(),
        Arc::new(LocalJobRegistry::default()),
        bus.clone(),
    ));

    // Padre: delega una vez y luego responde.
    let proveedor_padre = ProveedorConGuion::nuevo(vec![
        llamada(
            "subagent",
            serde_json::json!({"label": "hijo", "prompt": "haz lo tuyo"}),
        ),
        respuesta("padre listo"),
    ]);
    let mut herramientas_padre = ToolRegistry::new();
    herramientas_padre.register(Arc::new(SubagentTool));
    herramientas_padre.register(Arc::new(SoloPadre));
    let puerta = Arc::new(PuertaQueReinyecta::default());
    let observador = Arc::new(ObservadorContador::default());
    let mut config = config_de_prueba();
    config.max_steps = 20;
    let padre = AgentLoop::new(
        proveedor_padre.clone(),
        store.clone(),
        Arc::new(herramientas_padre),
        bus.clone(),
        Arc::new(PermissionService::allow_all()),
        config,
        dir.path().to_path_buf(),
    )
    .with_close_gate(puerta.clone())
    .with_tool_observer(observador.clone())
    .with_prompt_contributor(Arc::new(ContribuidorDelPadre))
    .with_subagent_launcher(lanzador);

    padre
        .run(sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    let (hijo, estado_final) = esperar_hijo_terminal(&store, sid).await;
    let eventos = recoger(&mut rx).await;

    let estados_del_hijo = eventos
        .iter()
        .filter_map(|evento| match evento {
            LiveEvent::SubagentChanged(d) if d.child == hijo => Some(d.status.state),
            LiveEvent::SubagentSettled { child, status, .. } if *child == hijo => Some(*status),
            _ => None,
        })
        .collect();
    let cierres = puerta.consultas.lock().unwrap().clone();
    let observaciones = observador.consultas.lock().unwrap().clone();
    let peticiones_padre = proveedor_padre.vistas.lock().unwrap().clone();
    let peticiones_hijo = proveedor_hijo.vistas.lock().unwrap().clone();
    Observado {
        padre: sid,
        hijo,
        cierres,
        observaciones,
        peticiones_padre,
        peticiones_hijo,
        eventos,
        estados_del_hijo,
        estado_final,
    }
}

#[tokio::test]
async fn un_hijo_local_no_consulta_la_politica_de_cierre_ni_el_observador() {
    let o = lanzar_desde_el_padre().await;
    // El padre sí las consulta: la prueba mide algo real.
    assert!(
        !o.cierres.is_empty(),
        "la política del padre no se consultó"
    );
    assert!(
        !o.observaciones.is_empty(),
        "el observador del padre no se consultó"
    );
    // El hijo cerró su turno (llamó a `eco`, pidió respuesta y terminó) sin consultarlas.
    assert_eq!(o.estado_final, TaskState::Completed);
    assert!(o.peticiones_hijo.len() >= 2, "{:?}", o.peticiones_hijo);
    assert_eq!(
        o.cierres.iter().filter(|s| **s == o.hijo).count(),
        0,
        "la política de cierre no debe consultarse para el hijo"
    );
    assert_eq!(
        o.observaciones.iter().filter(|s| **s == o.hijo).count(),
        0,
        "el observador no debe consultarse para el hijo"
    );
    assert!(o.cierres.iter().all(|s| *s == o.padre));
    assert!(o.observaciones.iter().all(|s| *s == o.padre));
}

#[tokio::test]
async fn un_hijo_local_no_recibe_el_prompt_ni_las_herramientas_del_padre() {
    let o = lanzar_desde_el_padre().await;
    let (sistema_padre, herramientas_padre) = &o.peticiones_padre[0];
    assert!(sistema_padre.contains(SECCION_DEL_PADRE), "{sistema_padre}");
    assert!(herramientas_padre.iter().any(|n| n == "solo_padre"));

    for (sistema, herramientas) in &o.peticiones_hijo {
        assert!(!sistema.contains(SECCION_DEL_PADRE), "{sistema}");
        assert!(
            !herramientas.iter().any(|n| n == "solo_padre"),
            "{herramientas:?}"
        );
    }
    // Sanidad: el hijo sí tiene sus propias herramientas.
    assert!(o.peticiones_hijo[0].1.iter().any(|n| n == "eco"));
}

#[tokio::test]
async fn el_trabajo_del_hijo_no_publica_flow_changed_y_solo_recorre_estados_a2a() {
    let o = lanzar_desde_el_padre().await;
    let flujos = o.eventos.iter().filter(|e| es_flujo(e)).count();
    let del_padre = o.observaciones.iter().filter(|s| **s == o.padre).count();
    assert!(del_padre >= 1);
    assert_eq!(
        flujos, del_padre,
        "solo debe haber un FlowChanged por cada resumen del padre"
    );

    // Ningún flujo añade estados: el enum de A2A sigue siendo el de siempre y el hijo solo usa esos.
    assert_eq!(
        TaskState::ALL.len(),
        8,
        "revisar la garantía A2A si cambia la lista"
    );
    assert_eq!(o.estado_final, TaskState::Completed);
    let permitidos = [
        TaskState::Submitted,
        TaskState::Working,
        TaskState::Completed,
    ];
    assert!(!o.estados_del_hijo.is_empty());
    for estado in &o.estados_del_hijo {
        assert!(
            permitidos.contains(estado),
            "estado inesperado del hijo: {estado:?}"
        );
    }
}

/// Un `FlowChanged` directo o envuelto en un `ChildEvent`.
fn es_flujo(evento: &LiveEvent) -> bool {
    match evento {
        LiveEvent::FlowChanged { .. } => true,
        LiveEvent::ChildEvent { event, .. } => matches!(**event, LiveEvent::FlowChanged { .. }),
        _ => false,
    }
}
