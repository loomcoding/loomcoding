use super::*;

/// Proveedor de resumen controlable. Solo la primera petición de resumen espera a `compuerta`;
/// las siguientes responden al momento, así que una vía síncrona posterior no se bloquea. Cada
/// petición de resumen avisa por el canal de llegadas en cuanto entra en el proveedor.
struct ResumenRetenido {
    resumen: String,
    compuerta: Semaphore,
    llegadas: mpsc::UnboundedSender<()>,
    peticiones: AtomicUsize,
    normales: Mutex<VecDeque<Vec<StreamChunk>>>,
    /// Si existe, cada respuesta normal lo cancela: simula que la persona cancela el turno.
    cancelar_turno: Option<CancellationToken>,
}

impl ResumenRetenido {
    /// Devuelve el proveedor y el receptor de llegadas de peticiones de resumen.
    fn nuevo(
        resumen: &str,
        normales: Vec<Vec<StreamChunk>>,
        cancelar_turno: Option<CancellationToken>,
    ) -> (Self, mpsc::UnboundedReceiver<()>) {
        let (llegadas, recibidas) = mpsc::unbounded_channel();
        let proveedor = Self {
            resumen: resumen.to_string(),
            compuerta: Semaphore::new(0),
            llegadas,
            peticiones: AtomicUsize::new(0),
            normales: Mutex::new(VecDeque::from(normales)),
            cancelar_turno,
        };
        (proveedor, recibidas)
    }

    /// Peticiones de resumen recibidas hasta ahora, retenidas o no.
    fn peticiones_de_resumen(&self) -> usize {
        self.peticiones.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Provider for ResumenRetenido {
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
    ) -> Result<ProviderStream> {
        if es_peticion_de_resumen(&req) {
            let orden = self.peticiones.fetch_add(1, Ordering::SeqCst);
            let _ = self.llegadas.send(());
            if orden == 0 {
                self.compuerta
                    .acquire()
                    .await
                    .expect("la compuerta no se cierra")
                    .forget();
            }
            return Ok(Box::pin(futures::stream::iter(vec![
                StreamChunk::TextDelta(self.resumen.clone()),
                usage_chunk(700, 90),
                StreamChunk::Done(FinishReason::Stop),
            ])));
        }
        if let Some(cancelar) = &self.cancelar_turno {
            cancelar.cancel();
        }
        let guion = self
            .normales
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                vec![
                    StreamChunk::TextDelta("hecho".into()),
                    StreamChunk::Done(FinishReason::Stop),
                ]
            });
        Ok(Box::pin(futures::stream::iter(guion)))
    }
}

struct EntornoEnSegundoPlano {
    _dir: tempfile::TempDir,
    store: Arc<SqliteStore>,
    sid: SessionId,
    proveedor: Arc<ResumenRetenido>,
    llegadas: mpsc::UnboundedReceiver<()>,
    bus: EventBus,
    agente: AgentLoop,
}

/// Entorno de un agente con el proveedor controlable, ventana de 50 000 tokens y el historial
/// dado ya guardado. `background` fija `[compaction] background`.
async fn entorno_en_segundo_plano(
    proveedor: (ResumenRetenido, mpsc::UnboundedReceiver<()>),
    historial: &[Message],
    background: bool,
    presupuesto: BudgetConfig,
) -> EntornoEnSegundoPlano {
    let (proveedor, llegadas) = proveedor;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("b.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    for mensaje in historial {
        store.append_message(sid, mensaje).await.unwrap();
    }
    let proveedor = Arc::new(proveedor);
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 10,
        effort: None,
    };
    let bus = EventBus::new(1024);
    let agente = AgentLoop::new(
        proveedor.clone(),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        bus.clone(),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_catalog(catalogo_de_ventana(50_000))
    .with_compaction(compact::CompactionConfig { background })
    .with_budget(presupuesto);
    EntornoEnSegundoPlano {
        _dir: dir,
        store,
        sid,
        proveedor,
        llegadas,
        bus,
        agente,
    }
}

/// Espera al turno con un límite generoso. El límite solo evita que un fallo deje colgada la
/// suite; en el caso correcto no añade tiempo.
async fn sin_bloquear<T>(futuro: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), futuro)
        .await
        .expect("el turno no debe quedarse esperando")
}

/// Recoge los eventos hasta `BackgroundCompactionReady`, incluido.
async fn hasta_listo(rx: &mut broadcast::Receiver<LiveEvent>) -> Vec<LiveEvent> {
    sin_bloquear(async {
        let mut recogidos = Vec::new();
        loop {
            let evento = rx.recv().await.expect("el bus sigue abierto");
            let listo = matches!(evento, LiveEvent::BackgroundCompactionReady { .. });
            recogidos.push(evento);
            if listo {
                return recogidos;
            }
        }
    })
    .await
}

/// Saca de `rx` todos los eventos que ya estén publicados.
fn vaciar(rx: &mut broadcast::Receiver<LiveEvent>) -> Vec<LiveEvent> {
    let mut eventos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        eventos.push(evento);
    }
    eventos
}

/// Activaciones de las compactaciones publicadas en `eventos`, en orden.
fn activaciones(eventos: &[LiveEvent]) -> Vec<Activation> {
    eventos
        .iter()
        .filter_map(|evento| match evento {
            LiveEvent::ContextCompacted { activation, .. } => Some(activation.clone()),
            _ => None,
        })
        .collect()
}

/// Avisos de presupuesto de `eventos`, como `(porcentaje, usado)`.
fn avisos_de(eventos: &[LiveEvent]) -> Vec<(u8, f64)> {
    eventos
        .iter()
        .filter_map(|evento| match evento {
            LiveEvent::BudgetWarning {
                level_percent,
                used,
                ..
            } => Some((*level_percent, *used)),
            _ => None,
        })
        .collect()
}

/// Con 31 000 tokens y una ventana de 50 000 se cruza el 60 % (30 000) y no el 80 % (40 000).
#[tokio::test]
async fn background_summary_starts_at_the_buffer_ratio_without_blocking_the_turn() {
    let mut e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    let mut rx = e.bus.subscribe();

    let turno = sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();

    // El turno terminó mientras el resumen seguía retenido en el proveedor.
    assert_eq!(turno.final_text, "hecho");
    e.llegadas
        .recv()
        .await
        .expect("llegó la petición de resumen");
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Running
    );
    let eventos = vaciar(&mut rx);
    let arranque = eventos.iter().find_map(|evento| match evento {
        LiveEvent::BackgroundCompactionStarted { tokens_before } => Some(*tokens_before),
        _ => None,
    });
    assert!(
        arranque.is_some_and(|tokens| tokens >= 30_000),
        "se publica el arranque con los tokens del momento: {arranque:?}"
    );
    assert!(
        activaciones(&eventos).is_empty(),
        "el turno no compacta por su cuenta"
    );
    assert_eq!(e.proveedor.peticiones_de_resumen(), 1);

    // Al liberar la tarea, el resumen queda listo.
    e.proveedor.compuerta.add_permits(1);
    hasta_listo(&mut rx).await;
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Ready
    );
}

/// Un resumen listo se activa al cruzar el 80 %, sin ninguna llamada al modelo para resumir.
#[tokio::test]
async fn a_ready_summary_is_activated_without_a_model_call_at_the_compact_ratio() {
    let e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    e.proveedor.compuerta.add_permits(1);
    let mut rx = e.bus.subscribe();
    sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();
    hasta_listo(&mut rx).await;
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Ready
    );

    // Diez mensajes más llevan el contexto por encima del 80 % (40 000 tokens).
    for mensaje in historial_grande(10) {
        e.store.append_message(e.sid, &mensaje).await.unwrap();
    }
    let turno = sin_bloquear(
        e.agente
            .run(e.sid, "siguiente".into(), CancellationToken::new()),
    )
    .await
    .unwrap();

    assert_eq!(turno.final_text, "hecho");
    assert_eq!(activaciones(&vaciar(&mut rx)), vec![Activation::Background]);
    assert_eq!(
        e.proveedor.peticiones_de_resumen(),
        1,
        "solo la petición del arranque"
    );
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Idle
    );
}

/// Con el resumen en curso al cruzar el 80 %, el turno espera a esa misma tarea y no lanza
/// otra. La espera se registra antes de liberar la compuerta: el `join!` hace que la llamada
/// se quede dormida en `wait` en su primera vuelta, y la liberación ocurre en la segunda.
#[tokio::test]
async fn a_running_summary_is_awaited_instead_of_starting_a_second_one() {
    let mut e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    let mut rx = e.bus.subscribe();
    sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();
    e.llegadas
        .recv()
        .await
        .expect("llegó la petición de resumen");
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Running
    );

    let historia = e.store.messages(e.sid).await.unwrap();
    let cancel = CancellationToken::new();
    let liberar = async {
        tokio::task::yield_now().await;
        e.proveedor.compuerta.add_permits(1);
    };
    let (activado, ()) = tokio::join!(
        e.agente
            .compact_or_activate(e.sid, &historia, 40_000, &cancel),
        liberar
    );

    assert!(activado.unwrap());
    assert_eq!(e.proveedor.peticiones_de_resumen(), 1, "no se lanza otra");
    let activacion = activaciones(&vaciar(&mut rx));
    assert!(
        matches!(
            activacion.as_slice(),
            [Activation::BackgroundAwaited { .. }]
        ),
        "se espera a la tarea en curso: {activacion:?}"
    );
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Idle
    );
}

/// Si la tarea no termina dentro del plazo, se cancela y la compactación cae a la vía síncrona.
/// El tiempo es virtual: la espera de `BACKGROUND_WAIT_SECS` vence sin tardar de verdad.
#[tokio::test]
async fn a_timed_out_summary_falls_back_to_the_synchronous_path() {
    let mut e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    let mut rx = e.bus.subscribe();
    sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();
    e.llegadas
        .recv()
        .await
        .expect("llegó la petición de resumen");
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Running
    );

    let historia = e.store.messages(e.sid).await.unwrap();
    let cancel = CancellationToken::new();
    tokio::time::pause();
    let compactado = e
        .agente
        .compact_or_activate(e.sid, &historia, 40_000, &cancel)
        .await
        .unwrap();

    assert!(compactado);
    assert_eq!(activaciones(&vaciar(&mut rx)), vec![Activation::Sync]);
    assert_eq!(
        e.proveedor.peticiones_de_resumen(),
        2,
        "la de segundo plano, que caduca, y la síncrona"
    );
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Idle
    );
}

/// Un resumen listo deja de ser vigente si entre medias se persistió otro resumen: se descarta
/// y la compactación usa la vía síncrona.
#[tokio::test]
async fn a_stale_summary_is_discarded_when_another_was_persisted() {
    let e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    e.proveedor.compuerta.add_permits(1);
    let mut rx = e.bus.subscribe();
    sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();
    hasta_listo(&mut rx).await;

    // Otro resumen entra en el registro por otra vía (aquí, a mano) y luego llegan mensajes.
    let antes = e.store.messages(e.sid).await.unwrap();
    let corte = compact::choose_cut(&antes).expect("hay historia que resumir");
    e.store
        .append_message(e.sid, &compact::summary_message(corte, "otro resumen"))
        .await
        .unwrap();
    for mensaje in historial_grande(35) {
        e.store.append_message(e.sid, &mensaje).await.unwrap();
    }
    let historia = e.store.messages(e.sid).await.unwrap();
    assert!(
        compact::should_compact(&historia, 0, 50_000),
        "precondición: el contexto supera el 80 %"
    );

    let turno = sin_bloquear(
        e.agente
            .run(e.sid, "siguiente".into(), CancellationToken::new()),
    )
    .await
    .unwrap();

    assert_eq!(turno.final_text, "hecho");
    let eventos = vaciar(&mut rx);
    assert_eq!(activaciones(&eventos), vec![Activation::Sync]);
    assert!(
        eventos
            .iter()
            .any(|evento| matches!(evento, LiveEvent::BackgroundCompactionDiscarded)),
        "la interfaz debe saber que el listo caducó"
    );
    assert_eq!(
        e.proveedor.peticiones_de_resumen(),
        2,
        "el listo caducó: una de segundo plano y otra síncrona"
    );
}

/// Si el resumen en segundo plano no sirve (aquí, vacío), la sesión vuelve a reposo y se
/// publica el descarte: sin él, la interfaz seguiría mostrando «resumiendo…».
#[tokio::test]
async fn a_background_summary_that_fails_publishes_the_discard() {
    let e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("", vec![], None),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    e.proveedor.compuerta.add_permits(1);
    let mut rx = e.bus.subscribe();

    sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();
    let descartado = sin_bloquear(async {
        loop {
            let evento = rx.recv().await.expect("el bus sigue abierto");
            match evento {
                LiveEvent::BackgroundCompactionDiscarded => return true,
                LiveEvent::BackgroundCompactionReady { .. } => return false,
                _ => {}
            }
        }
    })
    .await;

    assert!(descartado, "un resumen vacío no queda listo");
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Idle
    );
}

/// Con `background = false` no hay ninguna petición de resumen antes del 80 %.
#[tokio::test]
async fn background_off_behaves_exactly_as_before() {
    let e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        false,
        BudgetConfig::default(),
    )
    .await;
    let mut rx = e.bus.subscribe();

    let turno = sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();

    assert_eq!(turno.final_text, "hecho");
    assert_eq!(
        e.proveedor.peticiones_de_resumen(),
        0,
        "ninguna petición de resumen antes del 80 %"
    );
    let eventos = vaciar(&mut rx);
    assert!(!eventos.iter().any(|evento| matches!(
        evento,
        LiveEvent::BackgroundCompactionStarted { .. } | LiveEvent::BackgroundCompactionReady { .. }
    )));
    assert!(activaciones(&eventos).is_empty());
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Idle
    );
}

/// Cancelar el turno después de lanzar el resumen no mata la tarea: sigue hasta quedar listo.
#[tokio::test]
async fn cancelling_the_turn_does_not_kill_the_background_summary() {
    let cancel = CancellationToken::new();
    let mut e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], Some(cancel.clone())),
        &historial_grande(31),
        true,
        BudgetConfig::default(),
    )
    .await;
    let mut rx = e.bus.subscribe();

    sin_bloquear(e.agente.run(e.sid, "hola".into(), cancel.clone()))
        .await
        .unwrap();
    assert!(
        cancel.is_cancelled(),
        "el turno se canceló tras lanzar el resumen"
    );

    e.llegadas
        .recv()
        .await
        .expect("llegó la petición de resumen");
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Running
    );
    e.proveedor.compuerta.add_permits(1);
    hasta_listo(&mut rx).await;
    assert_eq!(
        e.agente.background.state(e.sid),
        background::SlotState::Ready
    );
}

/// La llamada del resumen en segundo plano se apunta como cualquier otra y cuenta para el
/// presupuesto: 790 tokens sobre un límite de 500 generan avisos.
#[tokio::test]
async fn the_background_call_usage_is_recorded_and_counts_for_the_budget() {
    let e = entorno_en_segundo_plano(
        ResumenRetenido::nuevo("resumen", vec![], None),
        &historial_grande(31),
        true,
        presupuesto_de_tokens(500),
    )
    .await;
    e.proveedor.compuerta.add_permits(1);
    let mut rx = e.bus.subscribe();
    sin_bloquear(e.agente.run(e.sid, "hola".into(), CancellationToken::new()))
        .await
        .unwrap();

    // Los avisos se publican antes que `Ready`, así que se recogen con los eventos previos.
    let hasta = hasta_listo(&mut rx).await;

    let apuntes = e.store.session_usage(e.sid).await.unwrap();
    assert_eq!(
        apuntes.len(),
        2,
        "resumen y respuesta sin uso confirmado dejan un apunte cada uno"
    );
    assert!(apuntes
        .iter()
        .any(|record| (record.input_tokens, record.output_tokens) == (700, 90)));
    assert!(apuntes
        .iter()
        .any(|record| record.source == crate::message::UsageSource::Estimate));
    assert!(
        !avisos_de(&hasta).is_empty(),
        "el consumo del resumen cruza el presupuesto"
    );
}
