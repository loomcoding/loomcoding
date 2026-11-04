use super::*;

/// Herramienta que devuelve 4 000 bytes: cada resultado pesa 1 000 tokens estimados.
struct Relleno;

#[async_trait]
impl Tool for Relleno {
    fn name(&self) -> &str {
        "relleno"
    }
    fn description(&self) -> &str {
        "r"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object"})
    }
    fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    async fn run(
        &self,
        _c: ToolCtx,
        _i: serde_json::Value,
        _x: CancellationToken,
    ) -> Result<ToolOutput> {
        Ok(ToolOutput::ok("x".repeat(4000)))
    }
}

/// Respuesta normal que pide una llamada a `relleno` con una entrada distinta por paso, para
/// que `should_stop` no corte el turno por repetición.
fn llamada_relleno(n: usize) -> Vec<StreamChunk> {
    vec![
        StreamChunk::ToolCallComplete {
            id: format!("t{n}"),
            name: "relleno".into(),
            input: serde_json::json!({ "i": n }),
        },
        StreamChunk::Done(FinishReason::ToolCalls),
    ]
}

/// Como `proveedor_de`, pero las primeras `n` peticiones normales las rechaza por exceso de
/// contexto.
fn proveedor_que_desborda(resumen: &str, n: usize) -> PlanProvider {
    let proveedor = proveedor_de(resumen, false, vec![]);
    *proveedor.desbordes.lock().unwrap() = n;
    proveedor
}

#[tokio::test]
async fn a_context_overflow_rejection_compacts_and_retries_the_request() {
    // Ventana enorme: la compactación preventiva no actúa; sólo el rechazo del proveedor.
    let historial = historial_grande(100);
    let e = entorno_de_turno(
        proveedor_que_desborda("resumen", 1),
        catalogo_de_ventana(10_000_000),
        &historial,
        ToolRegistry::new(),
    )
    .await;

    e.agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
        .expect("el turno se recupera del rechazo");

    let vistas = e.proveedor.seen.lock().unwrap();
    let normales: Vec<_> = vistas
        .iter()
        .filter(|r| !es_peticion_de_resumen(r))
        .collect();
    assert_eq!(normales.len(), 2, "la petición rechazada y su reintento");
    assert_eq!(vistas.len(), 3, "más una de resumen");
    assert!(
        normales[1].messages.len() < normales[0].messages.len(),
        "el reintento lleva el historial resumido"
    );
    assert!(compact::parse_summary(&normales[1].messages[0]).is_some());
}

#[tokio::test]
async fn a_context_overflow_that_persists_stops_after_the_compaction_limit() {
    let historial = historial_grande(100);
    let e = entorno_de_turno(
        proveedor_que_desborda("resumen", usize::MAX),
        catalogo_de_ventana(10_000_000),
        &historial,
        ToolRegistry::new(),
    )
    .await;

    let Err(error) = e
        .agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
    else {
        panic!("sin remedio, el error debe llegar");
    };
    assert!(compact::is_context_overflow(&error), "{error}");

    let vistas = e.proveedor.seen.lock().unwrap();
    let resumenes = vistas.iter().filter(|r| es_peticion_de_resumen(r)).count();
    assert!(
        resumenes <= compact::MAX_COMPACTIONS_PER_TURN,
        "{resumenes} resúmenes"
    );
}

/// Historial alternado: los pares son llamadas del asistente y los impares sus resultados. Cada
/// mensaje pesa unos 1 000 tokens, de modo que el corte natural de una cola de 10 000 tokens
/// cae en un resultado y debe retroceder hasta su llamada.
fn historial_con_herramientas(n: usize) -> Vec<Message> {
    (0..n)
        .map(|i| {
            if i % 2 == 0 {
                Message::assistant(vec![ContentPart::ToolCall {
                    id: format!("c{i}"),
                    name: "relleno".into(),
                    input: serde_json::json!({ "i": format!("{i}{}", "p".repeat(4000)) }),
                }])
            } else {
                Message::new(
                    Role::Tool,
                    vec![ContentPart::ToolResult {
                        call_id: format!("c{}", i - 1),
                        content: "x".repeat(4000),
                        is_error: false,
                    }],
                )
            }
        })
        .collect()
}

/// Comprueba que cada resultado de herramienta de la petición lleva su llamada delante y que,
/// si la petición empieza por un resumen, la cola no empieza por un resultado.
fn assert_emparejado(mensajes: &[Message]) {
    if compact::parse_summary(&mensajes[0]).is_some() {
        assert_ne!(
            mensajes[1].role,
            Role::Tool,
            "la cola no puede empezar por un resultado de herramienta"
        );
    }
    let mut llamadas = std::collections::HashSet::new();
    for mensaje in mensajes {
        for parte in &mensaje.parts {
            match parte {
                ContentPart::ToolCall { id, .. } => {
                    llamadas.insert(id.clone());
                }
                ContentPart::ToolResult { call_id, .. } => {
                    assert!(
                        llamadas.contains(call_id),
                        "resultado {call_id} sin su llamada delante"
                    );
                }
                _ => {}
            }
        }
    }
}

#[tokio::test]
async fn request_is_summary_plus_tail_once_the_window_is_reached() {
    let historial = historial_grande(100);
    let e = entorno_de_turno(
        proveedor_de("resumen", false, vec![]),
        catalogo_de_ventana(1000),
        &historial,
        ToolRegistry::new(),
    )
    .await;
    let mut esperado = historial.clone();
    esperado.push(Message::user("hi"));
    let cut = compact::choose_cut(&esperado).expect("hay historia que resumir");

    e.agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let vistas = e.proveedor.seen.lock().unwrap();
    assert_eq!(vistas.len(), 2, "una petición de resumen y una normal");
    assert!(es_peticion_de_resumen(&vistas[0]));
    let normal = &vistas[1];
    assert!(!es_peticion_de_resumen(normal));
    let (covers, _) =
        compact::parse_summary(&normal.messages[0]).expect("la petición empieza por el resumen");
    assert_eq!(covers, cut);
    assert_eq!(normal.messages.len(), 1 + (esperado.len() - cut));
    assert_emparejado(&normal.messages);
}

#[tokio::test]
async fn store_keeps_every_message_after_compaction() {
    let historial = historial_grande(100);
    let e = entorno_de_turno(
        proveedor_de("resumen", false, vec![]),
        catalogo_de_ventana(1000),
        &historial,
        ToolRegistry::new(),
    )
    .await;
    let antes = e.store.messages(e.sid).await.unwrap();

    e.agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    // Tras el turno: lo anterior íntegro, el «hi», el resumen y la respuesta del asistente.
    let despues = e.store.messages(e.sid).await.unwrap();
    assert_eq!(despues.len(), antes.len() + 3);
    assert_eq!(&despues[..antes.len()], antes.as_slice());
    assert_eq!(despues[antes.len()].text(), "hi");
    assert!(compact::parse_summary(&despues[antes.len() + 1]).is_some());
}

#[tokio::test]
async fn no_compaction_when_the_window_is_unknown() {
    let historial = historial_grande(100);
    let e = entorno_de_turno(
        proveedor_de("resumen", false, vec![]),
        Arc::new(RwLock::new(crate::catalog::Catalog::default())),
        &historial,
        ToolRegistry::new(),
    )
    .await;

    e.agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let vistas = e.proveedor.seen.lock().unwrap();
    assert_eq!(vistas.len(), 1, "solo la petición normal, sin resumen");
    assert!(!es_peticion_de_resumen(&vistas[0]));
    assert_eq!(vistas[0].messages.len(), historial.len() + 1);
}

#[tokio::test]
async fn a_failing_summary_does_not_abort_the_turn_and_is_not_retried() {
    // Tres pasos con herramienta: si el fallo se reintentara en cada paso, habría más de una
    // petición de resumen.
    let historial = historial_grande(100);
    let mut herramientas = ToolRegistry::new();
    herramientas.register(Arc::new(Relleno));
    let e = entorno_de_turno(
        proveedor_de(
            "resumen",
            true,
            vec![
                llamada_relleno(1),
                llamada_relleno(2),
                vec![
                    StreamChunk::TextDelta("fin".into()),
                    StreamChunk::Done(FinishReason::Stop),
                ],
            ],
        ),
        catalogo_de_ventana(1000),
        &historial,
        herramientas,
    )
    .await;
    let antes = e.store.messages(e.sid).await.unwrap();

    let resultado = e
        .agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await;
    assert!(
        resultado.is_ok(),
        "el turno termina aunque falle el resumen"
    );

    {
        let vistas = e.proveedor.seen.lock().unwrap();
        assert_eq!(
            vistas.iter().filter(|r| es_peticion_de_resumen(r)).count(),
            1,
            "una sola petición de resumen, sin reintento"
        );
        let normales: Vec<&ProviderRequest> = vistas
            .iter()
            .filter(|r| !es_peticion_de_resumen(r))
            .collect();
        assert_eq!(
            normales.len(),
            3,
            "los tres pasos del turno llegan al proveedor"
        );
        let normal = normales[0];
        assert_eq!(
            normal.messages.len(),
            antes.len() + 1,
            "sin resumen, la petición lleva el historial completo"
        );
    }
    let despues = e.store.messages(e.sid).await.unwrap();
    assert!(
        despues.iter().all(|m| compact::parse_summary(m).is_none()),
        "no se persiste ningún resumen"
    );
}

#[tokio::test]
async fn compactions_per_turn_are_capped() {
    // 300 mensajes de 1 000 tokens. Cada paso añade un resultado de 1 000 tokens, así que
    // siempre queda algo que resumir y cada paso intentaría compactar: solo deben hacerse
    // `MAX_COMPACTIONS_PER_TURN`.
    let historial = historial_grande(300);
    let normales = vec![
        llamada_relleno(1),
        llamada_relleno(2),
        llamada_relleno(3),
        llamada_relleno(4),
        vec![
            StreamChunk::TextDelta("fin".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    ];
    let mut herramientas = ToolRegistry::new();
    herramientas.register(Arc::new(Relleno));
    let e = entorno_de_turno(
        proveedor_de("resumen", false, normales),
        catalogo_de_ventana(1000),
        &historial,
        herramientas,
    )
    .await;

    e.agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let vistas = e.proveedor.seen.lock().unwrap();
    let resumenes = vistas.iter().filter(|r| es_peticion_de_resumen(r)).count();
    assert_eq!(resumenes, compact::MAX_COMPACTIONS_PER_TURN);
    assert_eq!(vistas.len() - resumenes, 5, "las cinco peticiones normales");
}

#[tokio::test]
async fn tool_call_and_result_stay_paired_in_the_request() {
    let historial = historial_con_herramientas(101);
    let e = entorno_de_turno(
        proveedor_de("resumen", false, vec![]),
        catalogo_de_ventana(1000),
        &historial,
        ToolRegistry::new(),
    )
    .await;
    let mut esperado = historial.clone();
    esperado.push(Message::user("hi"));
    // El corte natural cae en el resultado 91 y se retrocede hasta la llamada 90.
    assert_eq!(compact::choose_cut(&esperado), Some(90));

    e.agente
        .run(e.sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let vistas = e.proveedor.seen.lock().unwrap();
    let normal = vistas
        .iter()
        .find(|r| !es_peticion_de_resumen(r))
        .expect("hay una petición normal");
    assert!(compact::parse_summary(&normal.messages[0]).is_some());
    assert_emparejado(&normal.messages);
}
