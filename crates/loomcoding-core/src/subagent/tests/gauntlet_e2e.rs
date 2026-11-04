//! Gauntlet de punta a punta: herramienta real, lanzador real y un proveedor simulado.
//!
//! Cabe señalar que el proveedor hace de constructor y de crítico según lo que le piden: el
//! constructor pule el artefacto solo cuando recibe la brecha, y el crítico ciego juzga leyendo los
//! ficheros de verdad. Sin red real.

use super::*;
use crate::gauntlet::{GauntletPhase, MAX_ROUNDS_LIMIT};
use crate::tools::gauntlet::GauntletTool;

const PULIDO: &str = "versión pulida";

/// Constructor y crítico simulados sobre el mismo proveedor.
struct ProveedorDelGauntlet {
    proyecto: std::path::PathBuf,
    /// Si es `false`, el constructor nunca arregla nada y el crítico siempre prefiere la barra.
    el_constructor_arregla: bool,
    peticiones_de_critico: Arc<std::sync::Mutex<Vec<String>>>,
}

impl ProveedorDelGauntlet {
    fn responder(&self, peticion: &crate::provider::ProviderRequest) -> String {
        let prompt = peticion
            .messages
            .iter()
            .rev()
            .find(|m| m.role == crate::message::Role::User)
            .map(|m| m.text())
            .unwrap_or_default();
        if prompt.starts_with("You are an independent critic") {
            self.peticiones_de_critico
                .lock()
                .unwrap()
                .push(prompt.clone());
            return self.juzgar(&prompt);
        }
        if self.el_constructor_arregla && prompt.contains("márgenes") {
            std::fs::write(self.proyecto.join("out/portada.md"), PULIDO).unwrap();
        }
        "He revisado la pieza.".into()
    }

    /// Lee los ficheros reales de cada candidato y prefiere el que esté pulido.
    fn juzgar(&self, prompt: &str) -> String {
        let candidato = |letra: &str| -> String {
            let cabecera = format!("Candidate {letra}:\n- ");
            let inicio = prompt.find(&cabecera).unwrap() + cabecera.len();
            let ruta = prompt[inicio..].lines().next().unwrap().to_owned();
            std::fs::read_to_string(self.proyecto.join(ruta)).unwrap()
        };
        let (a, b) = (candidato("A"), candidato("B"));
        let (preferido, hueco) = match (a == PULIDO, b == PULIDO) {
            (true, false) => ("A", "B"),
            (false, true) => ("B", "A"),
            _ => {
                // Ninguno está pulido: gana la barra, que es la que no tiene el texto del constructor.
                let barra = if a.contains("constructor") { "B" } else { "A" };
                return format!(
                    "Falta pulido.\n{{\"preferred\":\"{barra}\",\"biggest_gap\":\"los márgenes están descuadrados\"}}"
                );
            }
        };
        let _ = hueco;
        format!("Todo a la altura.\n{{\"preferred\":\"{preferido}\"}}")
    }
}

#[async_trait]
impl Provider for ProveedorDelGauntlet {
    fn capabilities(&self) -> crate::provider::Capabilities {
        crate::provider::Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: crate::provider::ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<crate::provider::ProviderStream> {
        // El crítico diligente abre los dos candidatos con `view` antes de juzgar.
        let prompt = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == crate::message::Role::User)
            .map(|m| m.text())
            .unwrap_or_default();
        let ya_leyo = req
            .messages
            .iter()
            .any(|m| m.role == crate::message::Role::Tool);
        if prompt.starts_with("You are an independent critic") && !ya_leyo {
            let mut chunks = Vec::new();
            for letra in ["A", "B"] {
                let cabecera = format!(
                    "Candidate {letra}:
- "
                );
                let inicio = prompt.find(&cabecera).unwrap() + cabecera.len();
                let ruta = prompt[inicio..].lines().next().unwrap().to_owned();
                chunks.push(StreamChunk::ToolCallComplete {
                    id: format!("view-{letra}"),
                    name: "view".into(),
                    input: serde_json::json!({ "path": ruta }),
                });
            }
            chunks.push(StreamChunk::Done(FinishReason::ToolCalls));
            return Ok(Box::pin(futures::stream::iter(chunks)));
        }
        Ok(Box::pin(futures::stream::iter(vec![
            StreamChunk::TextDelta(self.responder(&req)),
            StreamChunk::Done(FinishReason::Stop),
        ])))
    }
}

struct Montaje {
    _dir: tempfile::TempDir,
    proyecto: std::path::PathBuf,
    store: Arc<SqliteStore>,
    sesion: SessionId,
    lanzador: Arc<InProcessSubagentLauncher>,
    bus: EventBus,
    criticos: Arc<std::sync::Mutex<Vec<String>>>,
}

async fn montaje(el_constructor_arregla: bool) -> Montaje {
    let dir = tempfile::tempdir().unwrap();
    let proyecto = dir.path().join("proyecto");
    let perfiles = dir.path().join("perfiles");
    std::fs::create_dir_all(proyecto.join("out")).unwrap();
    std::fs::create_dir_all(proyecto.join("ref")).unwrap();
    std::fs::create_dir_all(&perfiles).unwrap();
    std::fs::write(proyecto.join("out/portada.md"), "versión del constructor").unwrap();
    std::fs::write(proyecto.join("ref/portada.md"), "la referencia").unwrap();
    escribir_perfil(
        &perfiles,
        "constructor",
        "---\nname: constructor\ndescription: construye\n---\n\nConstruyes.\n",
    );
    escribir_perfil(
        &perfiles,
        "critico",
        "---\nname: critico\ndescription: critica\n---\n\nCriticas.\n",
    );
    let (store, sesion) = tienda_de_prueba(dir.path(), "g.db").await;
    let criticos = Arc::new(std::sync::Mutex::new(Vec::new()));
    let proveedor: Arc<dyn Provider> = Arc::new(ProveedorDelGauntlet {
        proyecto: proyecto.clone(),
        el_constructor_arregla,
        peticiones_de_critico: criticos.clone(),
    });
    let bus = EventBus::new(256);
    let lanzador = Arc::new(
        InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(proveedor)),
            store.clone(),
            Arc::new(ToolRegistry::new()),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            proyecto.clone(),
            Arc::new(LocalJobRegistry::default()),
            bus.clone(),
        )
        .with_registry(registro_global(&perfiles)),
    );
    Montaje {
        _dir: dir,
        proyecto,
        store,
        sesion,
        lanzador,
        bus,
        criticos,
    }
}

impl Montaje {
    async fn accion(&self, entrada: serde_json::Value, acepta: bool) -> ToolOutput {
        self.intentar(entrada, acepta).await.unwrap()
    }

    async fn intentar(&self, entrada: serde_json::Value, acepta: bool) -> Result<ToolOutput> {
        let ctx = ToolCtx {
            cwd: self.proyecto.clone(),
            store: Some(self.store.clone()),
            permission: Arc::new(PermissionService::with_responder(Box::new(FixedAnswer(
                Some(if acepta {
                    "Accept and complete"
                } else {
                    "Cancel"
                }),
            )))),
            session: self.sesion,
            jobs: Arc::new(LocalJobRegistry::default()),
            bus: self.bus.clone(),
            subagents: Some(self.lanzador.clone()),
        };
        GauntletTool
            .run(ctx, entrada, CancellationToken::new())
            .await
    }

    async fn fase(&self) -> GauntletPhase {
        self.store
            .latest_gauntlet(self.sesion)
            .await
            .unwrap()
            .unwrap()
            .phase
    }

    /// Llama a `advance` hasta que la fase sale de `desde` (mientras el hijo corre, `advance` no cambia nada).
    async fn avanzar_desde(&self, desde: GauntletPhase) -> GauntletPhase {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                self.accion(serde_json::json!({"action": "advance"}), true)
                    .await;
                let fase = self.fase().await;
                if fase != desde {
                    return fase;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("el gauntlet debería salir de {desde:?}"))
    }

    async fn arrancar(&self) {
        let salida = self
            .accion(
                serde_json::json!({
                    "action": "start", "goal": "pulir la portada", "artifact": "out/portada.md",
                    "bar": ["ref/portada.md"], "builder": "constructor", "critic": "critico"
                }),
                true,
            )
            .await;
        assert!(!salida.is_error, "{}", salida.content);
    }
}

#[tokio::test]
async fn a_full_gauntlet_goes_from_a_gap_to_a_rebuild_to_a_confirmed_acceptance() {
    let m = montaje(true).await;
    m.arrancar().await;
    assert_eq!(m.fase().await, GauntletPhase::Building);

    // Ronda 1: el constructor no arregla nada y el crítico ciego encuentra la brecha.
    assert_eq!(
        m.avanzar_desde(GauntletPhase::Building).await,
        GauntletPhase::Judging
    );
    assert_eq!(
        m.avanzar_desde(GauntletPhase::Judging).await,
        GauntletPhase::Rebuilding
    );
    // Ronda 2: el constructor recibe la brecha y pule; el crítico ya no ve brecha.
    assert_eq!(
        m.avanzar_desde(GauntletPhase::Rebuilding).await,
        GauntletPhase::Judging
    );
    // Sin brecha, la comprobación de sesgo repite el juicio con el orden invertido (sigue en
    // `Judging`); al ser coherente, se pide la aceptación.
    assert_eq!(
        m.avanzar_desde(GauntletPhase::Judging).await,
        GauntletPhase::AwaitingFinalAcceptance
    );

    let juicios = m.criticos.lock().unwrap().len();
    assert_eq!(
        juicios, 3,
        "dos de la ronda 1 y 2 más la comprobación de sesgo"
    );

    // Solo la persona completa, con la huella del artefacto intacta.
    let salida = m
        .accion(serde_json::json!({"action": "accept"}), true)
        .await;
    assert!(!salida.is_error, "{}", salida.content);
    assert_eq!(m.fase().await, GauntletPhase::Completed);
    assert_eq!(
        std::fs::read_to_string(m.proyecto.join("out/portada.md")).unwrap(),
        PULIDO
    );
}

#[tokio::test]
async fn the_blind_critic_never_learns_which_candidate_is_the_bar_or_what_the_builder_said() {
    let m = montaje(true).await;
    m.arrancar().await;
    m.avanzar_desde(GauntletPhase::Building).await;
    m.avanzar_desde(GauntletPhase::Judging).await;
    let prompts = m.criticos.lock().unwrap().clone();
    let prompt = prompts.first().expect("el crítico debe haber juzgado");
    assert!(prompt.contains("Candidate A") && prompt.contains("Candidate B"));
    assert!(!prompt.contains("quality bar"), "{prompt}");
    assert!(!prompt.contains("He revisado la pieza"), "{prompt}");
}

#[tokio::test]
async fn an_unfixed_gap_stops_at_the_round_limit_and_never_offers_acceptance() {
    let m = montaje(false).await;
    m.arrancar().await;
    let mut fase = GauntletPhase::Building;
    while !fase.is_terminal() {
        let siguiente = m.avanzar_desde(fase).await;
        assert_ne!(
            siguiente,
            GauntletPhase::AwaitingFinalAcceptance,
            "con la brecha abierta nunca se pide aceptar"
        );
        fase = siguiente;
    }
    assert_eq!(fase, GauntletPhase::Stopped);
    let gauntlet = m.store.latest_gauntlet(m.sesion).await.unwrap().unwrap();
    assert_eq!(gauntlet.round, gauntlet.max_rounds);
    assert!(gauntlet.max_rounds <= MAX_ROUNDS_LIMIT);
    // Aceptar un gauntlet detenido no lo completa.
    let salida = m
        .intentar(serde_json::json!({"action": "accept"}), true)
        .await;
    assert!(salida.is_err() || salida.unwrap().is_error);
    assert_eq!(m.fase().await, GauntletPhase::Stopped);
}
