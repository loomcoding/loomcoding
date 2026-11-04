//! Compactación del historial: manual, en segundo plano y por desbordamiento.

use super::*;

impl AgentLoop {
    /// Resume ahora el historial antiguo de `session`, sin esperar al umbral (`/compact`). Usa la
    /// misma vía que la compactación automática: activa un resumen en segundo plano si lo hay y,
    /// si no, lo genera con el modelo. Publica siempre `ManualCompactionEnded`, también si falla,
    /// y no devuelve error: un fallo es un resultado, no una excepción. No debe llamarse con un
    /// turno en curso sobre la misma sesión; quien llama (la interfaz) lo garantiza.
    pub async fn compact_now(
        &self,
        session: SessionId,
        cancel: CancellationToken,
    ) -> crate::event::ManualCompaction {
        use crate::event::ManualCompaction;
        let outcome = match self.store.messages(session).await {
            Err(error) => {
                tracing::warn!(error = %error, "no se pudo leer el historial para compactar");
                ManualCompaction::Failed
            }
            Ok(history) if compact::choose_cut(&history).is_none() => {
                ManualCompaction::NothingToSummarize
            }
            Ok(history) => {
                let tokens_before =
                    compact::estimate_context_tokens(&history, self.system_prompt().len()) as u64;
                match self
                    .compact_or_activate(session, &history, tokens_before, &cancel)
                    .await
                {
                    Ok(true) => ManualCompaction::Compacted,
                    Ok(false) => ManualCompaction::Failed,
                    Err(error) => {
                        tracing::warn!(error = %error, "no se pudo resumir el historial");
                        ManualCompaction::Failed
                    }
                }
            }
        };
        self.bus
            .publish(LiveEvent::ManualCompactionEnded { outcome });
        outcome
    }

    /// Resume lo antiguo del historial de `session` y lo persiste como mensaje de resumen. Devuelve
    /// `Ok(true)` si compactó, y `Ok(false)` si no había nada que resumir, si el modelo devolvió un
    /// resumen vacío o si se canceló (en cuyo caso no se persiste nada). Un fallo del proveedor se
    /// propaga como `Err`. El consumo de la llamada de resumen se apunta y se publica como
    /// `CompactionUsage` aunque el resumen se descarte.
    pub(super) async fn compact(
        &self,
        session: SessionId,
        history: &[Message],
        tokens_before: u64,
        cancel: &CancellationToken,
    ) -> Result<bool> {
        let provider = self.provider.read().unwrap().clone();
        let config = self.config.read().unwrap().clone();
        let model = config.model;
        let base_url = config.provider.base_url;
        let (resumen, uso) = generate_summary(provider, model.clone(), history, cancel).await?;

        // La llamada de resumen cuesta lo mismo que cualquier otra, se acepte el resumen o no:
        // el consumo se apunta siempre para que el coste de compactar sea visible. Se publica como
        // `CompactionUsage` para que no mueva el medidor de contexto.
        if let Some(uso) = &uso {
            self.bus.publish(LiveEvent::CompactionUsage {
                input_tokens: uso.input_tokens,
                output_tokens: uso.output_tokens,
            });
            if let Some(usd) = uso.reported_cost_usd {
                self.bus.publish(LiveEvent::ReportedCost { usd });
            }
            self.record_usage(
                session,
                &model,
                &base_url,
                uso,
                crate::provider::CacheUsage::default(),
                crate::message::UsageSource::Provider,
                true,
                &[],
            )
            .await;
        }
        let Some(resumen) = resumen else {
            return Ok(false);
        };

        self.activate(session, &resumen, tokens_before, Activation::Sync)
            .await?;
        Ok(true)
    }

    /// Compacta el historial, o activa el resumen en segundo plano si lo hay. Sin
    /// `[compaction] background` es exactamente `compact`. Con él: un resumen listo y vigente se
    /// activa sin llamar al modelo; uno en curso se espera como mucho `BACKGROUND_WAIT_SECS`, y
    /// si no llega a tiempo, o no sirve, se cae a la vía síncrona. Devuelve `Ok(true)` si el
    /// historial cambió y `Ok(false)` si no se compactó porque no hubo resumen o se canceló el
    /// turno. Cancelar el turno no aborta la tarea en curso: sigue para el próximo turno.
    pub(super) async fn compact_or_activate(
        &self,
        session: SessionId,
        history: &[Message],
        tokens_before: u64,
        cancel: &CancellationToken,
    ) -> Result<bool> {
        if !self.compaction.background {
            return self.compact(session, history, tokens_before, cancel).await;
        }
        match self.background.state(session) {
            background::SlotState::Ready => {
                let vigente = self
                    .background
                    .take_ready(session, background::current_base(history));
                if let Some(pendiente) = vigente {
                    self.activate(
                        session,
                        &pendiente.summary,
                        tokens_before,
                        Activation::Background,
                    )
                    .await?;
                    return Ok(true);
                }
                // Resumen listo pero caduco: se tira y la interfaz deja de marcarlo como pendiente.
                self.bus.publish(LiveEvent::BackgroundCompactionDiscarded);
            }
            background::SlotState::Running => {
                let t0 = tokio::time::Instant::now();
                let espera = tokio::select! {
                    _ = cancel.cancelled() => return Ok(false),
                    resultado = self.background.wait(
                        session,
                        std::time::Duration::from_secs(background::BACKGROUND_WAIT_SECS),
                    ) => resultado,
                };
                if let background::WaitOutcome::Ready(pendiente) = espera {
                    if pendiente.base == background::current_base(history) {
                        let waited_ms = t0.elapsed().as_millis() as u64;
                        self.activate(
                            session,
                            &pendiente.summary,
                            tokens_before,
                            Activation::BackgroundAwaited { waited_ms },
                        )
                        .await?;
                        return Ok(true);
                    }
                    // Esperado, pero de otra base: caduco, se descarta igual que arriba.
                    self.bus.publish(LiveEvent::BackgroundCompactionDiscarded);
                }
            }
            background::SlotState::Idle => {}
        }
        // Sin resumen utilizable: la vía síncrona de siempre.
        self.compact(session, history, tokens_before, cancel).await
    }

    /// Activa un resumen ya generado: lo persiste como mensaje de resumen y publica `MessageAdded`
    /// y `ContextCompacted`. No hace llamadas al modelo. `activation` indica cómo entra en efecto.
    pub(super) async fn activate(
        &self,
        session: SessionId,
        resumen: &GeneratedSummary,
        tokens_before: u64,
        activation: Activation,
    ) -> Result<()> {
        let mensaje = compact::summary_message(resumen.covers, &resumen.text);
        self.store.append_message(session, &mensaje).await?;
        self.bus.publish(LiveEvent::MessageAdded(mensaje));
        self.bus.publish(LiveEvent::ContextCompacted {
            replaced: resumen.covers,
            tokens_before,
            summary_bytes: resumen.text.len(),
            activation,
        });
        Ok(())
    }
}

/// Genera en segundo plano el resumen del tramo antiguo y lo entrega a `finisher`. Es una tarea
/// propia con copias de lo que necesita: no toma prestado el agente ni el token de cancelación del
/// turno, así que cancelar el turno no mata el resumen. El consumo se apunta y se publica antes de
/// entregar, también si el resumen se descarta.
pub(super) async fn resumir_en_segundo_plano(
    finisher: background::Finisher,
    provider: Arc<dyn Provider>,
    request: (String, String),
    historial: Vec<Message>,
    recorder: UsageRecorder,
    bus: EventBus,
    session: SessionId,
) {
    let token = finisher.cancel_token();
    let (model, base_url) = request;
    let record_model = model.clone();
    let resultado = generate_summary(provider, model, &historial, &token).await;
    // Solo un resumen entero y de una tarea no cancelada queda listo; todo lo demás se descarta.
    let utilizable = matches!(&resultado, Ok((Some(_), _))) && !token.is_cancelled();
    if let Ok((resumen, uso)) = &resultado {
        if let Some(uso) = uso {
            bus.publish(LiveEvent::CompactionUsage {
                input_tokens: uso.input_tokens,
                output_tokens: uso.output_tokens,
            });
            if let Some(usd) = uso.reported_cost_usd {
                bus.publish(LiveEvent::ReportedCost { usd });
            }
            recorder
                .record(
                    session,
                    &record_model,
                    &base_url,
                    uso,
                    crate::provider::CacheUsage::default(),
                    crate::message::UsageSource::Provider,
                    true,
                    &[],
                )
                .await;
        }
        // Un resumen de una tarea cancelada no se anuncia: su slot ya se ha vaciado.
        if let (Some(resumen), false) = (resumen, token.is_cancelled()) {
            bus.publish(LiveEvent::BackgroundCompactionReady {
                summary_bytes: resumen.text.len(),
            });
        }
    }
    finisher.finish(resultado);
    if !utilizable {
        // Sin este aviso la interfaz seguiría mostrando «resumiendo…» de una tarea ya terminada.
        bus.publish(LiveEvent::BackgroundCompactionDiscarded);
    }
}

/// Resumen del tramo antiguo del historial, aún sin activar. `covers` es el corte: los mensajes
/// anteriores a él quedan cubiertos por `text`. `usage` es el consumo de la llamada de resumen.
#[derive(Debug, Clone)]
pub struct GeneratedSummary {
    pub text: String,
    pub usage: Option<Usage>,
    pub covers: usize,
}

/// Pide al modelo el resumen del tramo antiguo de `history`. No persiste nada ni publica eventos:
/// el llamador decide qué hacer con el texto y con el consumo.
///
/// Devuelve `(resumen, consumo)`. El consumo va aparte porque se apunta siempre, también cuando el
/// resumen se descarta. Por eso `resumen` es `None` si no hay corte, si se cancela, si el flujo no
/// cierra bien o si el texto queda vacío, mientras que `consumo` es `Some` en cuanto la llamada
/// llegó a consumir tokens.
pub async fn generate_summary(
    provider: Arc<dyn Provider>,
    model: String,
    history: &[Message],
    cancel: &CancellationToken,
) -> Result<(Option<GeneratedSummary>, Option<Usage>)> {
    let Some(cut) = compact::choose_cut(history) else {
        return Ok((None, None));
    };

    // Lo que se resume: lo efectivo anterior al corte (incluido el resumen previo, si lo hay).
    let anteriores: Vec<Message> = compact::effective_indices(history)
        .into_iter()
        .filter(|&i| i < cut)
        .map(|i| history[i].clone())
        .collect();
    let mut mensajes = clear::clear_old_tool_results(&anteriores, clear::KEEP_TOOL_RESULTS);
    mensajes.push(Message::user(INSTRUCCION_DE_RESUMEN));

    let req = ProviderRequest {
        model,
        system: compact::SUMMARY_SYSTEM_PROMPT.to_string(),
        messages: mensajes,
        tools: vec![],
        temperature: 0.0,
        max_tokens: compact::SUMMARY_MAX_TOKENS,
        effort: None,
        programmatic_tool_calling: false,
    };
    let mut stream = tokio::select! {
        _ = cancel.cancelled() => return Ok((None, None)),
        result = provider.stream(req, cancel.clone()) => result?,
    };
    let mut recogido = String::new();
    // Un flujo que acaba en error o sin `Done` ha podido quedarse a medias: un resumen
    // truncado por un fallo se descarta en lugar de presentarse como bueno.
    let mut cierre: Option<FinishReason> = None;
    let mut uso: Option<Usage> = None;
    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => break,
            chunk = stream.next() => chunk,
        };
        match chunk {
            Some(StreamChunk::TextDelta(t)) => recogido.push_str(&t),
            Some(StreamChunk::Usage(parcial)) => {
                let total = uso.get_or_insert(Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                    reported_cost_usd: None,
                });
                total.input_tokens = total.input_tokens.saturating_add(parcial.input_tokens);
                total.output_tokens = total.output_tokens.saturating_add(parcial.output_tokens);
                total.reported_cost_usd = parcial.reported_cost_usd.or(total.reported_cost_usd);
            }
            Some(StreamChunk::Done(razon)) => {
                cierre = Some(razon);
                break;
            }
            None => break,
            Some(_) => {}
        }
    }
    if cancel.is_cancelled() {
        return Ok((None, uso));
    }
    if matches!(cierre, None | Some(FinishReason::Error)) {
        tracing::warn!("el flujo del resumen terminó sin cierre válido; se descarta");
        return Ok((None, uso));
    }

    let recogido = recogido.trim();
    if recogido.is_empty() {
        return Ok((None, uso));
    }
    // El marcador de `limit_bytes` se suma al texto conservado: se le reserva hueco para que
    // el resumen entero respete `MAX_SUMMARY_BYTES`.
    let texto = limit_bytes(
        recogido,
        compact::MAX_SUMMARY_BYTES.saturating_sub(MARCADOR_DE_RECORTE),
    )
    .into_owned();
    let resumen = GeneratedSummary {
        text: texto,
        usage: uso.clone(),
        covers: cut,
    };
    Ok((Some(resumen), uso))
}
