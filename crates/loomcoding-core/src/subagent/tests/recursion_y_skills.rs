use super::*;

/// Proveedor de prueba que encadena delegaciones según el primer mensaje de cada conversación:
/// si el plan tiene una entrada para ese texto y aún no hay resultado de herramienta, llama a
/// `subagent`; si no, responde. Registra las herramientas que vio cada conversación.
struct ProveedorEnCadena {
    plan: std::collections::HashMap<String, (Option<String>, String)>,
    herramientas: Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
    puertas: std::collections::HashMap<String, Arc<tokio::sync::Notify>>,
    /// Tokens de entrada que declara cada respuesta; con 0 no se emite consumo.
    consumo: Arc<std::sync::atomic::AtomicU32>,
    /// Primer mensaje de la conversación de cada petición recibida, en orden.
    peticiones: Arc<std::sync::Mutex<Vec<String>>>,
    /// Prompt de sistema de la primera petición de cada conversación.
    sistemas: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

#[async_trait]
impl Provider for ProveedorEnCadena {
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
        let primero = req
            .messages
            .iter()
            .find(|mensaje| mensaje.role == crate::message::Role::User)
            .map(|mensaje| mensaje.text())
            .unwrap_or_default();
        self.peticiones.lock().unwrap().push(primero.clone());
        self.sistemas
            .lock()
            .unwrap()
            .entry(primero.clone())
            .or_insert_with(|| req.system.clone());
        self.herramientas
            .lock()
            .unwrap()
            .entry(primero.clone())
            .or_insert_with(|| req.tools.iter().map(|tool| tool.name.clone()).collect());
        let con_resultado = req
            .messages
            .iter()
            .any(|mensaje| mensaje.role == crate::message::Role::Tool);
        if let Some(puerta) = self.puertas.get(&primero) {
            puerta.notified().await;
        }
        let mut trozos = match self.plan.get(&primero) {
            Some((agente, siguiente)) if !con_resultado => vec![
                StreamChunk::ToolCallComplete {
                    id: format!("llamada-{primero}"),
                    name: "subagent".into(),
                    input: serde_json::json!({
                        "label": siguiente,
                        "prompt": siguiente,
                        "agent": agente,
                    }),
                },
                StreamChunk::Done(FinishReason::ToolCalls),
            ],
            _ => vec![
                StreamChunk::TextDelta(format!("{primero} listo")),
                StreamChunk::Done(FinishReason::Stop),
            ],
        };
        let consumo = self.consumo.load(std::sync::atomic::Ordering::SeqCst);
        if consumo > 0 {
            let cierre = trozos.len() - 1;
            trozos.insert(
                cierre,
                StreamChunk::Usage(crate::message::Usage {
                    input_tokens: consumo,
                    output_tokens: 1,
                    reported_cost_usd: None,
                }),
            );
        }
        Ok(Box::pin(futures::stream::iter(trozos)))
    }
}

struct EntornoRecursivo {
    _dir: tempfile::TempDir,
    _global: tempfile::TempDir,
    store: Arc<SqliteStore>,
    parent: SessionId,
    jobs: Arc<LocalJobRegistry>,
    lanzador: InProcessSubagentLauncher,
    herramientas: Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
    puertas: std::collections::HashMap<String, Arc<tokio::sync::Notify>>,
    consumo: Arc<std::sync::atomic::AtomicU32>,
    peticiones: Arc<std::sync::Mutex<Vec<String>>>,
    sistemas: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

impl EntornoRecursivo {
    /// Directorio de trabajo del lanzador, que hace de raíz del proyecto en las pruebas.
    fn parent_dir(&self) -> PathBuf {
        self._dir.path().to_path_buf()
    }
}

/// Lanzador con las tres herramientas de subagentes y `development_workflow` registradas, los
/// perfiles dados y el plan de delegaciones dado. `techo` es el techo de profundidad del árbol.
/// Herramienta que solo ve el agente principal: ningún hijo debe recibirla.
struct SoloPadre;

#[async_trait]
impl crate::tools::Tool for SoloPadre {
    fn name(&self) -> &str {
        "solo_padre"
    }
    fn description(&self) -> &str {
        "solo el agente principal"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object"})
    }
    fn parent_only(&self) -> bool {
        true
    }
    async fn run(
        &self,
        _c: crate::tools::ToolCtx,
        _i: serde_json::Value,
        _x: CancellationToken,
    ) -> crate::error::Result<crate::tools::ToolOutput> {
        Ok(crate::tools::ToolOutput::ok("ok"))
    }
}

#[tokio::test]
async fn a_child_never_sees_a_parent_only_tool() {
    let e = entorno_recursivo(1, &[], &[], &[]).await;
    let (_hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let vistas = herramientas_de(&e, "hijo");
    assert!(!vistas.iter().any(|n| n == "solo_padre"), "{vistas:?}");
}

async fn entorno_recursivo(
    techo: u32,
    perfiles: &[(&str, &str)],
    plan: &[(&str, Option<&str>, &str)],
    puertas: &[&str],
) -> EntornoRecursivo {
    entorno_recursivo_con_tope(techo, 8, perfiles, plan, puertas).await
}

/// Como `entorno_recursivo`, con el tope de subagentes activos del árbol.
async fn entorno_recursivo_con_tope(
    techo: u32,
    max_activos: usize,
    perfiles: &[(&str, &str)],
    plan: &[(&str, Option<&str>, &str)],
    puertas: &[&str],
) -> EntornoRecursivo {
    let puertas: std::collections::HashMap<String, Arc<tokio::sync::Notify>> = puertas
        .iter()
        .map(|primero| ((*primero).to_string(), Arc::new(tokio::sync::Notify::new())))
        .collect();
    let global = tempfile::tempdir().unwrap();
    for (nombre, texto) in perfiles {
        escribir_perfil(global.path(), nombre, texto);
    }
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "recursivo.db").await;
    let herramientas = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let consumo = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sistemas = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let proveedor: Arc<dyn Provider> = Arc::new(ProveedorEnCadena {
        consumo: consumo.clone(),
        peticiones: peticiones.clone(),
        sistemas: sistemas.clone(),
        plan: plan
            .iter()
            .map(|(primero, agente, siguiente)| {
                (
                    (*primero).to_string(),
                    (agente.map(str::to_string), (*siguiente).to_string()),
                )
            })
            .collect(),
        herramientas: herramientas.clone(),
        puertas: puertas.clone(),
    });
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(SubagentTool));
    tools.register(Arc::new(
        crate::tools::subagent_control::SubagentControlTool,
    ));
    tools.register(Arc::new(crate::tools::subagent_list::SubagentListTool));
    tools.register(Arc::new(crate::tools::gauntlet::GauntletTool));
    tools.register(Arc::new(
        crate::tools::development_workflow::DevelopmentWorkflowTool,
    ));
    tools.register(Arc::new(SoloPadre));
    let jobs = Arc::new(LocalJobRegistry::default());
    let lanzador = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(proveedor)),
        store.clone(),
        Arc::new(tools),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.path().to_path_buf(),
        jobs.clone(),
        EventBus::new(256),
    )
    .with_registry(registro_global(global.path()))
    .with_max_active(max_activos)
    .with_levels_left(root_levels(techo));
    EntornoRecursivo {
        _dir: dir,
        _global: global,
        store,
        parent,
        jobs,
        lanzador,
        herramientas,
        puertas,
        consumo,
        peticiones,
        sistemas,
    }
}

fn perfil_con_profundidad(nombre: &str, profundidad: Option<u32>) -> String {
    let extra = profundidad
        .map(|n| format!("max_depth: {n}\n"))
        .unwrap_or_default();
    format!("---\nname: {nombre}\ndescription: prueba\n{extra}---\n\nCuerpo.\n")
}

/// Espera (con cota) a que todos los hijos de `padre` estén en estado terminal.
async fn esperar_hijos_terminales(store: &SqliteStore, padre: SessionId) {
    for _ in 0..200 {
        let hijos = store.list_subagents(padre).await.unwrap();
        if !hijos.is_empty() && hijos.iter().all(|hijo| hijo.status.state.is_terminal()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("los hijos de {padre:?} no terminaron");
}

fn herramientas_de(entorno: &EntornoRecursivo, primero: &str) -> Vec<String> {
    entorno
        .herramientas
        .lock()
        .unwrap()
        .get(primero)
        .cloned()
        .unwrap_or_else(|| panic!("la conversación {primero} no llegó al proveedor"))
}

#[tokio::test]
async fn with_the_default_ceiling_a_child_never_sees_the_subagent_tools() {
    let perfil = perfil_con_profundidad("coord", Some(3));
    let e = entorno_recursivo(1, &[("coord", perfil.as_str())], &[], &[]).await;
    let (_hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let vistas = herramientas_de(&e, "hijo");
    for fuera in [
        "subagent",
        "subagent_control",
        "subagent_list",
        "development_workflow",
        "gauntlet",
    ] {
        assert!(!vistas.iter().any(|n| n == fuera), "{fuera} en {vistas:?}");
    }
}

#[tokio::test]
async fn a_child_without_max_depth_is_a_leaf_even_with_a_higher_ceiling() {
    let e = entorno_recursivo(2, &[], &[], &[]).await;
    let (_hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    assert!(!herramientas_de(&e, "hijo").iter().any(|n| n == "subagent"));
}

#[tokio::test]
async fn ceiling_two_and_a_two_level_profile_let_the_child_launch_a_leaf_grandchild() {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let e = entorno_recursivo(
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &[],
    )
    .await;
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    esperar_hijos_terminales(&e.store, hijo).await;

    let del_hijo = herramientas_de(&e, "hijo");
    assert!(del_hijo.iter().any(|n| n == "subagent"), "{del_hijo:?}");
    assert!(
        del_hijo.iter().any(|n| n == "subagent_list"),
        "{del_hijo:?}"
    );
    assert!(
        !del_hijo.iter().any(|n| n == "development_workflow"),
        "{del_hijo:?}"
    );
    assert!(!del_hijo.iter().any(|n| n == "gauntlet"), "{del_hijo:?}");
    let del_nieto = herramientas_de(&e, "nieto");
    assert!(!del_nieto.iter().any(|n| n == "subagent"), "{del_nieto:?}");
    // El nieto queda guardado como tarea de su padre, con su resultado.
    let nietos = e.store.list_subagents(hijo).await.unwrap();
    assert_eq!(nietos.len(), 1);
    assert_eq!(nietos[0].status.state, TaskState::Completed);
}

#[tokio::test]
async fn ceiling_three_builds_a_chain_whose_last_member_is_a_leaf() {
    let a = perfil_con_profundidad("a", Some(3));
    let b = perfil_con_profundidad("b", Some(2));
    let e = entorno_recursivo(
        3,
        &[("a", a.as_str()), ("b", b.as_str())],
        &[("hijo", Some("b"), "nieto"), ("nieto", None, "bisnieto")],
        &[],
    )
    .await;
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("a".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    esperar_hijos_terminales(&e.store, hijo).await;
    let nieto = e.store.list_subagents(hijo).await.unwrap()[0].child;
    esperar_hijos_terminales(&e.store, nieto).await;

    assert!(herramientas_de(&e, "hijo").iter().any(|n| n == "subagent"));
    assert!(herramientas_de(&e, "nieto").iter().any(|n| n == "subagent"));
    assert!(!herramientas_de(&e, "bisnieto")
        .iter()
        .any(|n| n == "subagent"));
}

/// Recoge, hasta que se agote la espera, los eventos de nieto que llegan al bus de la raíz:
/// los descriptores cuyo padre es `hijo` y los `ChildEvent` de ese padre.
async fn eventos_de_nieto(
    rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
    hijo: SessionId,
) -> (Vec<SessionId>, usize) {
    let mut descriptores = Vec::new();
    let mut reenviados = 0;
    while let Ok(Ok(evento)) = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await {
        match evento {
            LiveEvent::SubagentChanged(d) if d.parent == hijo => descriptores.push(d.child),
            LiveEvent::ChildEvent { parent, .. } if parent == hijo => reenviados += 1,
            _ => {}
        }
    }
    (descriptores, reenviados)
}

#[tokio::test]
async fn the_root_bus_receives_the_grandchild_events_untouched() {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let e = entorno_recursivo(
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &[],
    )
    .await;
    let mut rx = e.lanzador.bus.subscribe();
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    esperar_hijos_terminales(&e.store, hijo).await;
    let nieto = e.store.list_subagents(hijo).await.unwrap()[0].child;
    let (descriptores, reenviados) = eventos_de_nieto(&mut rx, hijo).await;
    assert!(descriptores.contains(&nieto), "{descriptores:?}");
    assert!(
        reenviados > 0,
        "ningún ChildEvent del nieto llegó a la raíz"
    );
}

#[tokio::test]
async fn with_the_default_ceiling_no_grandchild_event_reaches_the_root_bus() {
    let perfil = perfil_con_profundidad("coord", Some(3));
    let e = entorno_recursivo(1, &[("coord", perfil.as_str())], &[], &[]).await;
    let mut rx = e.lanzador.bus.subscribe();
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let (descriptores, reenviados) = eventos_de_nieto(&mut rx, hijo).await;
    assert!(descriptores.is_empty());
    assert_eq!(reenviados, 0);
}

/// El nieto cuenta en la misma familia de presupuesto que la raíz: con `hard_stop`, si el gasto del
/// hijo ya agotó el límite, el nieto lanzado no llega a pedir ninguna respuesta al modelo.
#[tokio::test]
async fn hard_stop_reaches_a_grandchild_through_the_shared_family() {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let mut e = entorno_recursivo(
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &[],
    )
    .await;
    e.consumo.store(1000, std::sync::atomic::Ordering::SeqCst);
    e.lanzador = e.lanzador.with_budget(crate::budget::BudgetConfig {
        session_tokens: Some(1000),
        session_cost_usd: None,
        hard_stop: true,
    });
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // El hijo puede acabar con error (sin respuesta final): lo que importa es que se detuvo.
    let _ = e.jobs.wait(e.parent, &job).await;
    esperar_hijos_terminales(&e.store, hijo).await;
    let peticiones = e.peticiones.lock().unwrap().clone();
    assert_eq!(
        peticiones.iter().filter(|p| p.as_str() == "hijo").count(),
        1,
        "{peticiones:?}"
    );
    assert_eq!(
        peticiones.iter().filter(|p| p.as_str() == "nieto").count(),
        0,
        "el nieto no debe pedir respuesta con la familia agotada: {peticiones:?}"
    );
}

/// Prompts de sistema del hijo y del nieto con el modo de delegación dado en el lanzador.
async fn sistemas_con_modo(modo: Option<crate::config::DelegationMode>) -> (String, String) {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let mut e = entorno_recursivo(
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &[],
    )
    .await;
    e.lanzador = e.lanzador.with_delegation_mode(modo);
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    esperar_hijos_terminales(&e.store, hijo).await;
    let sistemas = e.sistemas.lock().unwrap();
    (sistemas["hijo"].clone(), sistemas["nieto"].clone())
}

#[tokio::test]
async fn a_delegating_child_gets_the_delegation_rule_of_the_root() {
    use crate::config::DelegationMode;
    let (hijo, _) = sistemas_con_modo(Some(DelegationMode::OnRequest)).await;
    assert!(hijo.contains("explicitly asks"), "{hijo}");
    let (hijo, _) = sistemas_con_modo(Some(DelegationMode::Proactive)).await;
    assert!(hijo.contains("proactively"), "{hijo}");
}

#[tokio::test]
async fn without_a_delegation_mode_the_child_prompt_has_no_rule() {
    let (hijo, _) = sistemas_con_modo(None).await;
    assert!(!hijo.contains("explicitly asks") && !hijo.contains("proactively"));
}

#[tokio::test]
async fn a_leaf_grandchild_gets_no_delegation_rule() {
    use crate::config::DelegationMode;
    let (_, nieto) = sistemas_con_modo(Some(DelegationMode::Proactive)).await;
    assert!(!nieto.contains("proactively"), "{nieto}");
}

/// Un agente de catálogo aprobado recibe en su prompt las skills que cita, tomadas de la carpeta de
/// skills de su catálogo; un agente sin catálogo no las ve.
#[tokio::test]
async fn an_approved_catalog_agent_gets_the_skills_of_its_own_catalog() {
    use crate::registry::{BuiltInSource, FileSource, ProfileLevel, TrustStore};
    let mut e = entorno_recursivo(1, &[], &[], &[]).await;
    let catalogo = tempfile::tempdir().unwrap();
    let agentes = catalogo.path().join(".agents").join("agents");
    std::fs::create_dir_all(&agentes).unwrap();
    std::fs::write(
        agentes.join("revisor.md"),
        "---
name: revisor
description: x
scope:
  skills: [\"auditar\"]
---

Cuerpo.
",
    )
    .unwrap();
    let skill = catalogo
        .path()
        .join(".agents")
        .join("skills")
        .join("auditar");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---
name: Auditar
---
Cuerpo de la skill del catálogo.",
    )
    .unwrap();
    let datos = tempfile::tempdir().unwrap();
    let registro = Arc::new(
        AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FileSource::new(agentes, ProfileLevel::Catalog)),
        ])
        .with_trust(
            TrustStore::open(datos.path().to_path_buf()).unwrap(),
            e.parent_dir(),
            false,
        ),
    );
    registro.approve("revisor").unwrap();
    e.lanzador = e.lanzador.with_registry(registro);
    let (_hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("revisor".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let sistema = e.sistemas.lock().unwrap()["hijo"].clone();
    assert!(
        sistema.contains("Cuerpo de la skill del catálogo."),
        "{sistema}"
    );
}

/// Perfil global `rev` que declara las skills dadas, y esas mismas skills en el proyecto del entorno
/// con el cuerpo `cuerpo-<id>`.
async fn entorno_con_skills(declaradas: &[&str]) -> EntornoRecursivo {
    let lista = declaradas
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let perfil = format!(
        "---
name: rev
description: x
scope:
  skills: [{lista}]
---

Cuerpo.
"
    );
    let e = entorno_recursivo(1, &[("rev", perfil.as_str())], &[], &[]).await;
    for id in declaradas {
        let dir = e.parent_dir().join(".agents").join("skills").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!(
                "---
name: {id}
---
cuerpo-{id}"
            ),
        )
        .unwrap();
    }
    e
}

fn peticion(skills: Option<&[&str]>) -> SubagentRequest {
    SubagentRequest {
        label: "h".into(),
        prompt: "hijo".into(),
        agent: Some("rev".into()),
        skills: skills.map(|ids| ids.iter().map(|id| id.to_string()).collect()),
    }
}

#[tokio::test]
async fn a_user_selection_loads_only_the_chosen_skills_of_the_profile() {
    let e = entorno_con_skills(&["a", "b", "c"]).await;
    let (_hijo, job) = e
        .lanzador
        .start_requested(
            e.parent,
            peticion(Some(&["a", "c"])),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let sistema = e.sistemas.lock().unwrap()["hijo"].clone();
    assert!(
        sistema.contains("cuerpo-a") && sistema.contains("cuerpo-c"),
        "{sistema}"
    );
    assert!(!sistema.contains("cuerpo-b"), "{sistema}");
}

#[tokio::test]
async fn without_a_selection_every_declared_skill_loads_as_before() {
    let e = entorno_con_skills(&["a", "b"]).await;
    let (_hijo, job) = e
        .lanzador
        .start_requested(e.parent, peticion(None), CancellationToken::new())
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let sistema = e.sistemas.lock().unwrap()["hijo"].clone();
    assert!(
        sistema.contains("cuerpo-a") && sistema.contains("cuerpo-b"),
        "{sistema}"
    );
}

#[tokio::test]
async fn an_invalid_selection_is_rejected_before_creating_any_task() {
    let e = entorno_con_skills(&["a", "b"]).await;
    let diecisiete: Vec<String> = (0..17).map(|n| format!("s{n}")).collect();
    let diecisiete_refs: Vec<&str> = diecisiete.iter().map(String::as_str).collect();
    for malas in [
        vec!["z"],       // no declarada
        vec!["a", "a"],  // repetida
        vec![],          // vacía
        diecisiete_refs, // demasiadas
    ] {
        let error = e
            .lanzador
            .start_requested(e.parent, peticion(Some(&malas)), CancellationToken::new())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("skills"), "{malas:?}: {error}");
    }
    assert!(e.store.list_subagents(e.parent).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_selection_for_an_agent_without_skills_is_rejected() {
    let e = entorno_recursivo(1, &[], &[], &[]).await;
    let mut pedida = peticion(Some(&["a"]));
    pedida.agent = None;
    let error = e
        .lanzador
        .start_requested(e.parent, pedida, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("skills"), "{error}");
    assert!(e.store.list_subagents(e.parent).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_profile_clipped_by_the_ceiling_is_reported_once() {
    let perfil = perfil_con_profundidad("coord", Some(3));
    let e = entorno_recursivo(1, &[("coord", perfil.as_str())], &[], &[]).await;
    let mut avisos = e.lanzador.bus.subscribe();
    for etiqueta in ["uno", "dos"] {
        let (_hijo, job) = e
            .lanzador
            .start(
                e.parent,
                etiqueta.into(),
                etiqueta.into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
    }
    let mut recortes = 0;
    while let Ok(evento) = avisos.try_recv() {
        if let LiveEvent::RuntimeNotice { message } = evento {
            if message.contains("asks for max_depth 3") {
                recortes += 1;
            }
        }
    }
    assert_eq!(recortes, 1);
}

/// Espera (con cota) a que `padre` tenga al menos un hijo guardado y devuelve el primero.
async fn esperar_primer_hijo(store: &SqliteStore, padre: SessionId) -> SessionId {
    for _ in 0..200 {
        if let Some(hijo) = store.list_subagents(padre).await.unwrap().first() {
            return hijo.child;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("{padre:?} no llegó a tener hijos");
}

#[tokio::test]
async fn the_active_cap_counts_the_whole_tree_not_each_level() {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let e = entorno_recursivo_con_tope(
        2,
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &["hijo", "nieto"],
    )
    .await;
    // La primera petición del hijo pasa; la segunda espera, para que su puesto siga ocupado.
    e.puertas["hijo"].notify_one();
    let (hijo, _job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // El nieto está lanzado y detenido en su puerta: padre e hijo ocupan los dos puestos.
    esperar_primer_hijo(&e.store, hijo).await;
    let rechazo = e
        .lanzador
        .start(
            e.parent,
            "otro".into(),
            "otro".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        rechazo
            .to_string()
            .contains("maximum of 2 active subagents"),
        "{rechazo}"
    );
    e.puertas["nieto"].notify_one();
    e.puertas["hijo"].notify_one();
    esperar_hijos_terminales(&e.store, hijo).await;
}

#[tokio::test]
async fn canceling_the_parent_cancels_the_grandchild() {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let e = entorno_recursivo(
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &["hijo", "nieto"],
    )
    .await;
    e.puertas["hijo"].notify_one();
    let cancelar = CancellationToken::new();
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            cancelar.clone(),
        )
        .await
        .unwrap();
    // El nieto está lanzado y detenido en su puerta, que nadie abre.
    esperar_primer_hijo(&e.store, hijo).await;
    cancelar.cancel();
    e.jobs.wait(e.parent, &job).await.unwrap();
    esperar_hijos_terminales(&e.store, hijo).await;
    let nietos = e.store.list_subagents(hijo).await.unwrap();
    assert_eq!(nietos[0].status.state, TaskState::Canceled);
}

#[tokio::test]
async fn grandchildren_join_the_budget_family_of_the_root_not_of_their_parent() {
    let perfil = perfil_con_profundidad("coord", Some(3));
    let e = entorno_recursivo(3, &[("coord", perfil.as_str())], &[], &[]).await;
    let raiz = e.parent;
    // El lanzador de la raíz agrupa a sus hijos bajo la propia raíz.
    assert_eq!(e.lanzador.familia(raiz), raiz);
    // Uno anidado (el de un hijo) conserva esa misma raíz, lance quien lance.
    let anidado = e.lanzador.anidado(
        e.lanzador.familia(raiz),
        e.lanzador.provider.read().unwrap().clone(),
        config_de_prueba(),
        Arc::new(PermissionService::allow_all()),
        &ToolRegistry::new(),
        2,
    );
    let hijo = SessionId(raiz.0 + 1);
    assert_eq!(anidado.familia(hijo), raiz);
    let bisnieto = anidado.anidado(
        anidado.familia(hijo),
        anidado.provider.read().unwrap().clone(),
        config_de_prueba(),
        Arc::new(PermissionService::allow_all()),
        &ToolRegistry::new(),
        1,
    );
    assert_eq!(bisnieto.familia(SessionId(raiz.0 + 2)), raiz);
    // Y comparten el mismo semáforo de activos.
    assert!(Arc::ptr_eq(&e.lanzador.active, &bisnieto.active));
}

/// Con `confirm_delegation = always` y la raíz aprobando, el hijo delega en un nieto: su pregunta
/// llega al buzón de la raíz como la de cualquier hijo. Devuelve la pregunta vista y cuántos nietos
/// existen después de contestar `respuesta`.
async fn aprobar_nieto(respuesta: &str) -> (String, usize) {
    let perfil = perfil_con_profundidad("coord", Some(2));
    let mut e = entorno_recursivo(
        2,
        &[("coord", perfil.as_str())],
        &[("hijo", None, "nieto")],
        &[],
    )
    .await;
    e.lanzador = e
        .lanzador
        .with_delegation_confirm(ConfirmDelegation::Always);
    e.lanzador.permission = Arc::new(PermissionService::with_responder(Box::new(FixedAnswer(
        Some("Approve"),
    ))));
    let (hijo, job) = e
        .lanzador
        .start(
            e.parent,
            "h".into(),
            "hijo".into(),
            Some("coord".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // La delegación del nieto espera la contestación de la persona usuaria, igual que otra pregunta.
    let mut pendiente = None;
    for _ in 0..200 {
        if let Some(entrada) = e.lanzador.pending_inputs(e.parent).into_iter().next() {
            pendiente = Some(entrada);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let pendiente = pendiente.expect("la delegación del nieto no pidió aprobación");
    assert_eq!(pendiente.child, hijo);
    assert!(
        e.store.list_subagents(hijo).await.unwrap().is_empty(),
        "el nieto no debe existir antes de la aprobación"
    );
    let pregunta = pendiente.questions[0].question.clone();
    e.lanzador
        .answer(e.parent, hijo, vec![respuesta.to_string()])
        .await
        .unwrap();
    e.jobs.wait(e.parent, &job).await.unwrap();
    let nietos = e.store.list_subagents(hijo).await.unwrap();
    for nieto in &nietos {
        esperar_hijos_terminales(&e.store, hijo).await;
        assert_eq!(nieto.parent, hijo);
    }
    (pregunta, nietos.len())
}

#[tokio::test]
async fn a_grandchild_delegation_asks_for_approval_and_launches_only_when_approved() {
    let (pregunta, nietos) = aprobar_nieto("Approve").await;
    assert!(pregunta.contains("Task: nieto"), "{pregunta}");
    assert_eq!(nietos, 1);
}

#[tokio::test]
async fn a_canceled_grandchild_delegation_leaves_no_grandchild() {
    let (_pregunta, nietos) = aprobar_nieto("Cancel").await;
    assert_eq!(nietos, 0);
}
