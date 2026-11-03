use super::*;

/// Proveedor simulado de la compactación: responde siempre el mismo texto y guarda cada
/// petición completa que recibe.
struct SummaryProvider {
    texto: String,
    /// Cierre del flujo: `None` simula un flujo que se corta sin `Done`.
    cierre: Mutex<Option<FinishReason>>,
    /// Consumo que el proveedor declara en la respuesta, si lo declara.
    uso: Option<Usage>,
    seen: Mutex<Vec<ProviderRequest>>,
}

#[async_trait]
impl Provider for SummaryProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: false,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        self.seen.lock().unwrap().push(req);
        let mut chunks = vec![StreamChunk::TextDelta(self.texto.clone())];
        if let Some(uso) = &self.uso {
            chunks.push(StreamChunk::Usage(uso.clone()));
        }
        if let Some(razon) = *self.cierre.lock().unwrap() {
            chunks.push(StreamChunk::Done(razon));
        }
        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}

/// Entorno de compactación: un almacén con el historial persistido y un agente cuyo
/// proveedor responde `texto`.
struct EntornoDeResumen {
    _dir: tempfile::TempDir,
    store: Arc<SqliteStore>,
    sid: SessionId,
    proveedor: Arc<SummaryProvider>,
    bus: EventBus,
    agente: AgentLoop,
}

async fn entorno_de_resumen(texto: &str, historial: &[Message]) -> EntornoDeResumen {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("r.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    for mensaje in historial {
        store.append_message(sid, mensaje).await.unwrap();
    }
    let proveedor = Arc::new(SummaryProvider {
        texto: texto.to_string(),
        cierre: Mutex::new(Some(FinishReason::Stop)),
        uso: Some(Usage {
            input_tokens: 700,
            output_tokens: 90,
            reported_cost_usd: None,
        }),
        seen: Mutex::new(Vec::new()),
    });
    let bus = EventBus::new(64);
    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let agente = AgentLoop::new(
        proveedor.clone(),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        bus.clone(),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );
    EntornoDeResumen {
        _dir: dir,
        store,
        sid,
        proveedor,
        bus,
        agente,
    }
}

#[tokio::test]
async fn compact_persists_a_summary_message_and_leaves_the_log_intact() {
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cut = compact::choose_cut(&antes).expect("hay historia que resumir");

    let cancel = CancellationToken::new();
    assert!(e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap());

    let despues = e.store.messages(e.sid).await.unwrap();
    assert_eq!(despues.len(), antes.len() + 1);
    assert_eq!(&despues[..antes.len()], antes.as_slice());
    let (covers, texto) =
        compact::parse_summary(despues.last().unwrap()).expect("el resumen va al final");
    assert_eq!(covers, cut);
    assert_eq!(texto, "resumen");
}

#[tokio::test]
async fn compact_request_has_no_tools_and_ends_with_the_summarize_instruction() {
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cancel = CancellationToken::new();
    e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap();

    let vistas = e.proveedor.seen.lock().unwrap();
    assert_eq!(vistas.len(), 1, "una sola petición de resumen");
    let req = &vistas[0];
    assert!(req.tools.is_empty());
    assert_eq!(req.system, compact::SUMMARY_SYSTEM_PROMPT);
    assert_eq!(req.temperature, 0.0);
    assert_eq!(req.max_tokens, compact::SUMMARY_MAX_TOKENS);
    assert!(req.effort.is_none());
    let ultimo = req.messages.last().expect("la petición lleva mensajes");
    assert_eq!(ultimo.role, Role::User);
    assert_eq!(
        ultimo.text(),
        "Summarize the conversation above following your instructions."
    );
}

#[tokio::test]
async fn compact_caps_an_oversized_summary() {
    let e = entorno_de_resumen(&"y".repeat(40_000), &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cancel = CancellationToken::new();
    assert!(e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap());

    let despues = e.store.messages(e.sid).await.unwrap();
    let (_, texto) = compact::parse_summary(despues.last().unwrap()).unwrap();
    // El tope es exacto: el hueco del marcador de omisión sale del propio tope.
    assert!(
        texto.len() <= compact::MAX_SUMMARY_BYTES,
        "el resumen mide {} bytes",
        texto.len()
    );
    assert!(texto.len() < 40_000, "el recorte debe haber actuado");
}

/// Resultados de compactación manual que hay en el bus, en orden de llegada.
fn manuales_del_bus(
    rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
) -> Vec<crate::event::ManualCompaction> {
    let mut vistos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::ManualCompactionEnded { outcome } = evento {
            vistos.push(outcome);
        }
    }
    vistos
}

#[tokio::test]
async fn compact_now_summarizes_without_waiting_for_the_threshold() {
    use crate::event::ManualCompaction;
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let mut rx = e.bus.subscribe();

    let resultado = e.agente.compact_now(e.sid, CancellationToken::new()).await;

    assert_eq!(resultado, ManualCompaction::Compacted);
    assert_eq!(manuales_del_bus(&mut rx), vec![ManualCompaction::Compacted]);
    let despues = e.store.messages(e.sid).await.unwrap();
    assert!(compact::parse_summary(despues.last().unwrap()).is_some());
}

#[tokio::test]
async fn compact_now_with_nothing_to_summarize_calls_no_model() {
    use crate::event::ManualCompaction;
    let e = entorno_de_resumen("resumen", &historial_grande(2)).await;
    let mut rx = e.bus.subscribe();

    let resultado = e.agente.compact_now(e.sid, CancellationToken::new()).await;

    assert_eq!(resultado, ManualCompaction::NothingToSummarize);
    assert_eq!(
        manuales_del_bus(&mut rx),
        vec![ManualCompaction::NothingToSummarize]
    );
    assert!(e.proveedor.seen.lock().unwrap().is_empty());
    assert_eq!(e.store.messages(e.sid).await.unwrap().len(), 2);
}

#[tokio::test]
async fn compact_now_reports_failure_when_the_summary_is_empty() {
    use crate::event::ManualCompaction;
    let e = entorno_de_resumen(
        "  
 ",
        &historial_grande(100),
    )
    .await;
    let mut rx = e.bus.subscribe();

    let resultado = e.agente.compact_now(e.sid, CancellationToken::new()).await;

    assert_eq!(resultado, ManualCompaction::Failed);
    assert_eq!(manuales_del_bus(&mut rx), vec![ManualCompaction::Failed]);
    assert_eq!(e.store.messages(e.sid).await.unwrap().len(), 100);
}

#[tokio::test]
async fn compact_returns_false_when_the_model_returns_nothing() {
    let e = entorno_de_resumen("  \n ", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cancel = CancellationToken::new();
    assert!(!e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap());
    assert_eq!(e.store.messages(e.sid).await.unwrap().len(), antes.len());
}

#[tokio::test]
async fn compact_discards_a_summary_whose_stream_ended_in_error_or_was_cut() {
    for cierre in [Some(FinishReason::Error), None] {
        let e = entorno_de_resumen("resumen a medias", &historial_grande(100)).await;
        *e.proveedor.cierre.lock().unwrap() = cierre;
        let antes = e.store.messages(e.sid).await.unwrap();
        let cancel = CancellationToken::new();
        assert!(!e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap());
        assert_eq!(
            e.store.messages(e.sid).await.unwrap().len(),
            antes.len(),
            "no se persiste un resumen truncado ({cierre:?})"
        );
    }
}

#[tokio::test]
async fn compact_records_the_usage_of_the_summary_call_even_if_it_is_discarded() {
    for cierre in [Some(FinishReason::Stop), Some(FinishReason::Error)] {
        let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
        *e.proveedor.cierre.lock().unwrap() = cierre;
        let antes = e.store.messages(e.sid).await.unwrap();
        let mut rx = e.bus.subscribe();
        let cancel = CancellationToken::new();
        e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap();

        let apuntes = e.store.session_usage(e.sid).await.unwrap();
        assert_eq!(apuntes.len(), 1, "un apunte por la llamada ({cierre:?})");
        assert_eq!(apuntes[0].input_tokens, 700);
        assert_eq!(apuntes[0].output_tokens, 90);
        let mut visto = false;
        // ADAPTADO: el consumo del resumen usa CompactionUsage
        while let Ok(evento) = rx.try_recv() {
            visto |= matches!(
                evento,
                LiveEvent::CompactionUsage {
                    input_tokens: 700,
                    output_tokens: 90
                }
            );
        }
        assert!(visto, "el consumo también llega a los contadores en vivo");
    }
}

#[tokio::test]
async fn generate_summary_does_not_touch_the_store_or_the_bus() {
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let mut rx = e.bus.subscribe();
    let cancel = CancellationToken::new();
    let proveedor: Arc<dyn Provider> = e.proveedor.clone();

    let (resumen, uso) = generate_summary(proveedor, "m".into(), &antes, &cancel)
        .await
        .unwrap();
    let resumen = resumen.expect("hay resumen");
    assert_eq!(resumen.text, "resumen");
    assert_eq!(resumen.covers, compact::choose_cut(&antes).unwrap());
    // El consumo sale en la respuesta, para que el llamador lo apunte.
    assert_eq!(
        uso.map(|u| (u.input_tokens, u.output_tokens)),
        Some((700, 90))
    );

    assert_eq!(e.store.messages(e.sid).await.unwrap().len(), antes.len());
    assert!(e.store.session_usage(e.sid).await.unwrap().is_empty());
    assert!(rx.try_recv().is_err(), "no publica ningún evento");
}

#[tokio::test]
async fn compact_usage_goes_through_compaction_usage_not_usage() {
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let mut rx = e.bus.subscribe();
    let cancel = CancellationToken::new();
    assert!(e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap());

    let mut consumos_de_resumen = Vec::new();
    let mut usos_normales = 0;
    while let Ok(evento) = rx.try_recv() {
        match evento {
            LiveEvent::CompactionUsage {
                input_tokens,
                output_tokens,
            } => consumos_de_resumen.push((input_tokens, output_tokens)),
            LiveEvent::Usage { .. } => usos_normales += 1,
            _ => {}
        }
    }
    assert_eq!(consumos_de_resumen, vec![(700, 90)]);
    assert_eq!(usos_normales, 0, "el resumen no debe publicar Usage");
}

#[tokio::test]
async fn compact_returns_false_when_there_is_nothing_to_summarize() {
    let historial = [Message::user("hola"), Message::user("adiós")];
    let e = entorno_de_resumen("resumen", &historial).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cancel = CancellationToken::new();
    assert!(!e.agente.compact(e.sid, &antes, 0, &cancel).await.unwrap());
    assert!(
        e.proveedor.seen.lock().unwrap().is_empty(),
        "no debe haber petición de resumen"
    );
}

#[tokio::test]
async fn compact_publishes_context_compacted() {
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cut = compact::choose_cut(&antes).expect("hay historia que resumir");
    let mut rx = e.bus.subscribe();

    let cancel = CancellationToken::new();
    assert!(e
        .agente
        .compact(e.sid, &antes, 1234, &cancel)
        .await
        .unwrap());

    let mut eventos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        eventos.push(evento);
    }
    let compactado = eventos.iter().find_map(|evento| match evento {
        LiveEvent::ContextCompacted {
            replaced,
            tokens_before,
            summary_bytes,
            ..  // ADAPTADO: campo nuevo
        } => Some((*replaced, *tokens_before, *summary_bytes)),
        _ => None,
    });
    assert_eq!(compactado, Some((cut, 1234, "resumen".len())));
    assert!(
        eventos
            .iter()
            .any(|evento| matches!(evento, LiveEvent::MessageAdded(_))),
        "el mensaje de resumen también se publica"
    );
}

#[tokio::test]
async fn activate_persists_the_message_and_publishes_the_activation() {
    let e = entorno_de_resumen("resumen", &historial_grande(100)).await;
    let antes = e.store.messages(e.sid).await.unwrap();
    let cut = compact::choose_cut(&antes).expect("hay historia que resumir");
    let mut rx = e.bus.subscribe();
    let resumen = GeneratedSummary {
        text: "resumen".to_string(),
        usage: None,
        covers: cut,
    };

    e.agente
        .activate(e.sid, &resumen, 777, Activation::Background)
        .await
        .unwrap();

    let despues = e.store.messages(e.sid).await.unwrap();
    assert_eq!(despues.len(), antes.len() + 1);
    let (covers, texto) =
        compact::parse_summary(despues.last().unwrap()).expect("el resumen va al final");
    assert_eq!(covers, cut);
    assert_eq!(texto, "resumen");

    let mut eventos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        eventos.push(evento);
    }
    let activacion = eventos.iter().find_map(|evento| match evento {
        LiveEvent::ContextCompacted {
            replaced,
            tokens_before,
            summary_bytes,
            activation,
        } => Some((
            *replaced,
            *tokens_before,
            *summary_bytes,
            activation.clone(),
        )),
        _ => None,
    });
    assert_eq!(
        activacion,
        Some((cut, 777, "resumen".len(), Activation::Background))
    );
    assert!(
        eventos
            .iter()
            .any(|evento| matches!(evento, LiveEvent::MessageAdded(_))),
        "el mensaje de resumen también se publica"
    );
}
