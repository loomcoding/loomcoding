//! El turno: petición al modelo, herramientas y reintentos.

use super::*;

/// Cota del razonamiento que se conserva por si hay que recuperarlo como respuesta (decálogo de contexto).
const MAX_RECOVERED_REASONING_BYTES: usize = 16 * 1024;

/// Respuestas seguidas cortadas por el tope de salida que se toleran antes de dar el turno por perdido.
const MAX_CONSECUTIVE_LENGTH_CUTS: usize = 2;

/// Resultado que recibe el modelo cuando su llamada a herramienta llegó cortada por el tope de salida.
const OUTPUT_LIMIT_NOTICE: &str = "Your response was cut by the output limit, so this tool call arrived \
incomplete and was NOT run. Do not repeat it as is: split the work into smaller pieces (a shorter file, \
or several small `edit` calls) and think less before acting.";

/// Añade `piece` y, si se pasa de `cap` bytes, conserva solo la cola: la respuesta va al final.
pub(super) fn push_tail_capped(buf: &mut String, piece: &str, cap: usize) {
    buf.push_str(piece);
    if buf.len() > cap {
        let mut cut = buf.len() - cap;
        while !buf.is_char_boundary(cut) {
            cut += 1;
        }
        buf.drain(..cut);
    }
}

impl AgentLoop {
    /// Ejecuta el turno. Los avisos pendientes los entrega el llamante antes de
    /// llamar aquí; el bucle solo los vuelve a vaciar entre pasos.
    pub(super) async fn run_turn(
        &self,
        session: SessionId,
        user_prompt: Option<Message>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome> {
        let provider = self.provider.read().unwrap().clone();
        let config = self.config.read().unwrap().clone();
        // Ventana, no historial completo: esto se ejecuta en cada paso del bucle y leer
        // todo cada vez era lo que llevaba al daemon a decenas de gigabytes.
        let mut history = self.store.recent_messages(session, HISTORY_WINDOW).await?;
        if let Some(mut user) = user_prompt {
            user.id = 0;
            user.created_at_ms = crate::message::now_ms();
            self.store.append_message(session, &user).await?;
            self.bus.publish(LiveEvent::MessageAdded(user.clone()));
            history.push(user);
        }

        let system = self.system_prompt();
        let ctx = ToolCtx {
            cwd: self.cwd.clone(),
            store: Some(self.store.clone()),
            permission: self.permission.clone(),
            session,
            jobs: self.jobs.clone(),
            bus: self.bus.clone(),
            subagents: self.subagents_visibles().cloned(),
        };
        let mut steps = 0usize;
        // Rondas de solo lectura: no gastan el presupuesto de trabajo, pero tienen su propio tope.
        let mut read_steps = 0usize;
        // Prórrogas concedidas al llegar a un tope: suben ese tope y nada más.
        let (mut extensions, mut extra_steps, mut extra_read_steps) = (0usize, 0usize, 0usize);
        let mut final_text = String::new();
        let mut stopped = None;
        let mut compactions = 0usize;
        let mut compaction_failed = false;
        let mut tool_calls_this_turn = 0usize;
        // Provider continuation data is transient and is reattached after the durable history
        // reload between tool rounds. This keeps opaque provider code out of the session store.
        let mut provider_states_this_turn: Vec<(Vec<String>, crate::message::ProviderState)> =
            Vec::new();
        let mut provider_state_bytes_this_turn = 0usize;
        // Respuestas seguidas cortadas por el tope de salida.
        let mut length_cuts = 0usize;

        loop {
            if cancel.is_cancelled() {
                break;
            }
            self.flush_subagent_notices(session).await?;
            // Ventana, no historial completo: esto se ejecuta en cada paso del bucle.
            history = self.store.recent_messages(session, HISTORY_WINDOW).await?;
            let read_cap = self.limits.read_step_cap(config.max_steps);
            match crate::agent::stop::stop_reason_budgets(
                &history,
                steps,
                config.max_steps + extra_steps,
                read_steps,
                read_cap + extra_read_steps,
            ) {
                None => {}
                Some(reason)
                    if reason != crate::agent::stop::StopReason::RepeatedToolCalls
                        && extensions < crate::agent::stop::MAX_STEP_EXTENSIONS
                        && self.ask_to_continue(reason, &cancel).await =>
                {
                    extensions += 1;
                    if reason == crate::agent::stop::StopReason::StepBudget {
                        extra_steps += config.max_steps.max(1);
                    } else {
                        extra_read_steps += config.max_steps.max(1);
                    }
                }
                Some(reason) => {
                    stopped = Some(reason);
                    break;
                }
            }
            // Exploración atascada: tras muchas rondas de lectura seguidas se avisa al modelo una vez
            // (el aviso es un mensaje de usuario, así que rearma la cuenta).
            let streak = crate::agent::stop::exploration_streak(&history);
            if streak >= crate::agent::stop::STUCK_EXPLORATION_ROUNDS {
                let warning = Message::user(format!(
                    "[Notice] You have made {streak} rounds of reading without writing anything. Stop exploring: \
                     make the change with what you already know, or answer stating exactly what you still need."
                ));
                self.store.append_message(session, &warning).await?;
                self.bus.publish(LiveEvent::SessionMessageAdded {
                    session,
                    message: warning.clone(),
                });
                self.bus.publish(LiveEvent::RuntimeNotice {
                    message: format!(
                        "exploración atascada: {streak} lecturas seguidas, aviso enviado al modelo"
                    ),
                });
                history.push(warning);
            }
            // Tope duro opcional: con `hard_stop`, agotado el presupuesto de la familia no se pide otra
            // respuesta al modelo. Sin él no se toca el almacén, de modo que el coste es nulo.
            if self.budget.hard_stop {
                let familia = self
                    .store
                    .session_usage(self.budget_family.unwrap_or(session))
                    .await?;
                if let Some(agotado) = crate::budget::budget_exhausted(&self.budget, &familia) {
                    self.bus.publish(LiveEvent::RuntimeNotice {
                        message: format!(
                            "Budget exhausted ({:?}: {} of {}): the turn was stopped by hard_stop.",
                            agotado.metric, agotado.used, agotado.limit
                        ),
                    });
                    break;
                }
            }
            // Si el contexto estimado alcanza la fracción de la ventana, se resume lo antiguo antes
            // de construir la petición. Sin ventana conocida no se compacta. Un fallo no aborta el
            // turno ni se reintenta en él.
            let window = self
                .catalog
                .read()
                .unwrap()
                .context_window(&config.provider.base_url, &config.model);
            if let Some(window) = window {
                // Con segundo plano, al alcanzar el búfer se lanza el resumen y el turno sigue sin
                // esperarlo. Solo si la sesión no tiene ya una tarea en curso o un resumen listo.
                if self.compaction.background
                    && compactions < compact::MAX_COMPACTIONS_PER_TURN
                    && !compaction_failed
                    && background::should_start_background(&history, system.len(), window)
                {
                    let tokens_before =
                        compact::estimate_context_tokens(&history, system.len()) as u64;
                    if let Some(finisher) = self
                        .background
                        .try_start(session, background::current_base(&history))
                    {
                        self.bus
                            .publish(LiveEvent::BackgroundCompactionStarted { tokens_before });
                        tokio::spawn(resumir_en_segundo_plano(
                            finisher,
                            provider.clone(),
                            (config.model.clone(), config.provider.base_url.clone()),
                            history.clone(),
                            self.recorder(),
                            self.bus.clone(),
                            session,
                        ));
                    }
                }
                if compactions < compact::MAX_COMPACTIONS_PER_TURN
                    && !compaction_failed
                    && compact::should_compact(&history, system.len(), window)
                    // Sin corte posible aún no hay nada que resumir: no es un fallo y no debe
                    // apagar la compactación, porque más adelante en el turno sí puede haberlo.
                    && compact::choose_cut(&history).is_some()
                {
                    let tokens_before =
                        compact::estimate_context_tokens(&history, system.len()) as u64;
                    match self
                        .compact_or_activate(session, &history, tokens_before, &cancel)
                        .await
                    {
                        Ok(true) => {
                            compactions += 1;
                            history = self.store.messages(session).await?;
                        }
                        Ok(false) => compaction_failed = true,
                        Err(error) => {
                            tracing::warn!(error = %error, "no se pudo resumir el historial; el turno continúa sin compactar");
                            compaction_failed = true;
                        }
                    }
                    if cancel.is_cancelled() {
                        break;
                    }
                }
            }
            for (call_ids, state) in &provider_states_this_turn {
                reattach_provider_state(&mut history, call_ids, state.clone());
            }
            steps += 1;
            self.bus.publish(LiveEvent::StepStarted {
                step: steps,
                max_steps: config.max_steps,
            });
            let workflow_active = if self.subagents.is_some() {
                dispatch::active_orchestration(self.store.as_ref(), session).await?
            } else {
                false
            };
            let mut tools = if workflow_active {
                self.tools.only(dispatch::WORKFLOW_PARENT_TOOLS).specs()
            } else {
                self.tools.specs()
            };
            if self.delegation == Some(crate::config::DelegationMode::Off) {
                tools.retain(|tool| !DELEGATION_TOOLS.contains(&tool.name.as_str()));
            }
            let mut req = ProviderRequest {
                model: config.model.clone(),
                system: system.clone(),
                messages: clear::clear_old_tool_results(
                    &compact::effective_history(&history),
                    clear::KEEP_TOOL_RESULTS,
                ),
                tools,
                temperature: self.limits.temperature,
                max_tokens: self.limits.max_output_tokens,
                effort: config.effort.clone(),
                programmatic_tool_calling: programmatic_tool_calling_enabled(),
            };
            let request_id = NEXT_USAGE_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
            let request_model = req.model.clone();
            let request_base_url = config.provider.base_url.clone();
            let mut request_components = self.tools.mcp_usage_components(&req.tools);
            request_components.extend(self.system_usage_components.iter().cloned());
            request_components
                .sort_by(|left, right| (left.kind, &left.id).cmp(&(right.kind, &right.id)));
            let estimated_input = estimate_provider_request_tokens(&req);
            // Con ventana conocida, el tope de salida cede ante una entrada grande: el servidor
            // rechaza la petición entera si `entrada + max_tokens` excede el contexto.
            if let Some(window) = window {
                let fitted =
                    compact::fit_output_cap(window, u64::from(estimated_input), req.max_tokens);
                if fitted < req.max_tokens {
                    self.bus.publish(LiveEvent::RuntimeNotice {
                        message: format!(
                            "output cap reduced from {} to {fitted} tokens to fit the context window",
                            req.max_tokens
                        ),
                    });
                    req.max_tokens = fitted;
                }
            }
            self.bus.publish(LiveEvent::ContextBreakdown {
                request_id,
                breakdown: compact::breakdown_request(&req.system, &req.messages, &req.tools),
            });
            self.bus.publish(LiveEvent::RuntimeNotice {
                message: format!("provider request started for `{}`", config.model),
            });
            let abierto = tokio::select! {
                _ = cancel.cancelled() => None,
                result = provider.stream(req, cancel.clone()) => Some(result),
            };
            let Some(abierto) = abierto else {
                let estimate = Usage {
                    input_tokens: estimated_input,
                    output_tokens: 0,
                    reported_cost_usd: None,
                };
                self.record_usage(
                    session,
                    &request_model,
                    &request_base_url,
                    &estimate,
                    crate::provider::CacheUsage::default(),
                    crate::message::UsageSource::Estimate,
                    true,
                    &request_components,
                )
                .await;
                publish_usage_progress(
                    &self.bus,
                    request_id,
                    &request_model,
                    None,
                    false,
                    &estimate,
                    true,
                    &request_components,
                );
                break;
            };
            let mut stream = match abierto {
                Ok(stream) => {
                    self.bus.publish(LiveEvent::RuntimeNotice {
                        message: format!("provider response received for `{}`", config.model),
                    });
                    publish_usage_progress(
                        &self.bus,
                        request_id,
                        &request_model,
                        None,
                        false,
                        &Usage {
                            input_tokens: estimated_input,
                            output_tokens: 0,
                            reported_cost_usd: None,
                        },
                        false,
                        &request_components,
                    );
                    stream
                }
                // Red de seguridad de la compactación: la estimación local pudo quedarse corta y el
                // proveedor rechaza la petición por exceder la ventana. Se resume por fuerza y se
                // reintenta, con el mismo tope de compactaciones por turno que la vía normal, de
                // modo que el bucle no puede ser infinito. Si no hay corte posible, o el resumen
                // falla, se devuelve el error original.
                Err(error)
                    if compact::is_context_overflow(&error)
                        && compactions < compact::MAX_COMPACTIONS_PER_TURN
                        && !compaction_failed
                        && compact::choose_cut(&history).is_some() =>
                {
                    tracing::warn!(error = %error, "el proveedor rechazó el contexto por exceso; se compacta y se reintenta");
                    let tokens_before =
                        compact::estimate_context_tokens(&history, system.len()) as u64;
                    match self
                        .compact_or_activate(session, &history, tokens_before, &cancel)
                        .await
                    {
                        Ok(true) => {
                            compactions += 1;
                            continue;
                        }
                        Ok(false) if cancel.is_cancelled() => break,
                        Ok(false) | Err(_) => return Err(error),
                    }
                }
                Err(error) => {
                    let estimate = Usage {
                        input_tokens: estimated_input,
                        output_tokens: 0,
                        reported_cost_usd: None,
                    };
                    self.record_usage(
                        session,
                        &request_model,
                        &request_base_url,
                        &estimate,
                        crate::provider::CacheUsage::default(),
                        crate::message::UsageSource::Estimate,
                        true,
                        &request_components,
                    )
                    .await;
                    publish_usage_progress(
                        &self.bus,
                        request_id,
                        &request_model,
                        None,
                        false,
                        &estimate,
                        true,
                        &request_components,
                    );
                    return Err(error);
                }
            };
            let mut text_buf = String::new();
            let mut reasoning_buf = String::new();
            let mut tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
            let mut provider_usage: Option<Usage> = None;
            let mut provider_usage_final = false;
            let mut provider_cache = crate::provider::CacheUsage::default();
            let mut provider_state = None;
            let mut stream_error = None;
            let mut finish = FinishReason::Stop;

            while let Some(chunk) = tokio::select! {
                _ = cancel.cancelled() => None,
                chunk = stream.next() => chunk,
            } {
                match chunk {
                    StreamChunk::TextDelta(t) => {
                        text_buf.push_str(&t);
                        self.bus.publish(LiveEvent::Delta { text: t });
                        publish_usage_progress(
                            &self.bus,
                            request_id,
                            &request_model,
                            provider_usage.clone(),
                            provider_usage_final,
                            &Usage {
                                input_tokens: estimated_input,
                                output_tokens: estimate_stream_output(&text_buf, &tool_calls),
                                reported_cost_usd: None,
                            },
                            false,
                            &request_components,
                        );
                    }
                    StreamChunk::ReasoningDelta(r) => {
                        push_tail_capped(&mut reasoning_buf, &r, MAX_RECOVERED_REASONING_BYTES);
                        self.bus.publish(LiveEvent::ReasoningDelta { text: r });
                    }
                    StreamChunk::Usage(chunk) => {
                        provider_usage = Some(chunk);
                        provider_usage_final = true;
                        publish_usage_progress(
                            &self.bus,
                            request_id,
                            &request_model,
                            provider_usage.clone(),
                            true,
                            &Usage {
                                input_tokens: estimated_input,
                                output_tokens: estimate_stream_output(&text_buf, &tool_calls),
                                reported_cost_usd: None,
                            },
                            false,
                            &request_components,
                        );
                    }
                    StreamChunk::CacheUsage(cache) => {
                        // Como con los demás contadores, vale el máximo visto: el proveedor los repite acumulados.
                        provider_cache.read_tokens =
                            provider_cache.read_tokens.max(cache.read_tokens);
                        provider_cache.write_tokens =
                            provider_cache.write_tokens.max(cache.write_tokens);
                    }
                    StreamChunk::UsageUpdate(update) => {
                        let total = provider_usage.get_or_insert(Usage {
                            input_tokens: 0,
                            output_tokens: 0,
                            reported_cost_usd: None,
                        });
                        if let Some(input) = update.input_tokens {
                            total.input_tokens = total.input_tokens.max(input);
                        }
                        if let Some(output) = update.output_tokens {
                            total.output_tokens = total.output_tokens.max(output);
                        }
                        total.reported_cost_usd =
                            update.reported_cost_usd.or(total.reported_cost_usd);
                        provider_usage_final |= update.final_update;
                        publish_usage_progress(
                            &self.bus,
                            request_id,
                            &request_model,
                            provider_usage.clone(),
                            provider_usage_final,
                            &Usage {
                                input_tokens: estimated_input,
                                output_tokens: estimate_stream_output(&text_buf, &tool_calls),
                                reported_cost_usd: None,
                            },
                            false,
                            &request_components,
                        );
                    }
                    StreamChunk::ToolCallComplete { id, name, input } => {
                        tool_calls.push((id, name, input));
                        publish_usage_progress(
                            &self.bus,
                            request_id,
                            &request_model,
                            provider_usage.clone(),
                            provider_usage_final,
                            &Usage {
                                input_tokens: estimated_input,
                                output_tokens: estimate_stream_output(&text_buf, &tool_calls),
                                reported_cost_usd: None,
                            },
                            false,
                            &request_components,
                        );
                        tool_calls_this_turn = tool_calls_this_turn.saturating_add(1);
                        let max_per_response = crate::config::max_tool_calls_per_response();
                        let max_per_turn = crate::config::max_tool_calls_per_turn();
                        if tool_calls.len() > max_per_response
                            || tool_calls_this_turn > max_per_turn
                        {
                            let mensaje = format!(
                                "provider tool-call limit exceeded ({max_per_response} per response, {max_per_turn} per turn)"
                            );
                            return Err(crate::error::CoreError::Provider(mensaje));
                        }
                    }
                    StreamChunk::ProviderState(state) => {
                        let state_bytes = serde_json::to_vec(&state)
                            .map_err(|_| {
                                crate::error::CoreError::Provider(
                                    "provider continuation state could not be serialized".into(),
                                )
                            })?
                            .len();
                        if state_bytes > MAX_PROVIDER_STATE_BYTES
                            || provider_state_bytes_this_turn.saturating_add(state_bytes)
                                > MAX_TURN_PROVIDER_STATE_BYTES
                        {
                            return Err(crate::error::CoreError::Provider(
                                "provider continuation state exceeds the per-state or per-turn limit".into(),
                            ));
                        }
                        provider_state_bytes_this_turn += state_bytes;
                        provider_state = Some(state);
                    }
                    StreamChunk::ProviderNotice(message) => {
                        self.bus.publish(LiveEvent::RuntimeNotice { message });
                    }
                    StreamChunk::Error(error) => {
                        stream_error = Some(error);
                        break;
                    }
                    StreamChunk::Done(r) => finish = r,
                    _ => {}
                }
            }

            // Una ronda con llamadas y ninguna de escritura es de lectura: cuenta aparte.
            if !tool_calls.is_empty()
                && !tool_calls
                    .iter()
                    .any(|(_, name, _)| crate::agent::stop::is_write_tool(name))
            {
                steps = steps.saturating_sub(1);
                read_steps += 1;
            }
            let estimate = Usage {
                input_tokens: estimated_input,
                output_tokens: estimate_stream_output(&text_buf, &tool_calls),
                reported_cost_usd: None,
            };
            let usage_source = if provider_usage_final {
                crate::message::UsageSource::Provider
            } else {
                crate::message::UsageSource::Estimate
            };
            let stored_usage = if provider_usage_final {
                provider_usage.as_ref().unwrap_or(&estimate)
            } else {
                &estimate
            };
            self.record_usage(
                session,
                &request_model,
                &request_base_url,
                stored_usage,
                if provider_usage_final {
                    provider_cache
                } else {
                    crate::provider::CacheUsage::default()
                },
                usage_source,
                true,
                &request_components,
            )
            .await;
            publish_usage_progress(
                &self.bus,
                request_id,
                &request_model,
                provider_usage.clone(),
                provider_usage_final,
                &estimate,
                true,
                &request_components,
            );
            if let Some(usd) = provider_usage
                .as_ref()
                .and_then(|usage| usage.reported_cost_usd)
            {
                self.bus.publish(LiveEvent::ReportedCost { usd });
            }

            if cancel.is_cancelled() {
                finish = FinishReason::Error;
            }
            if stream_error.is_some() {
                finish = FinishReason::Error;
            }

            // Un modelo con razonamiento puede cerrar con `Stop` habiendo escrito la respuesta dentro
            // del razonamiento, sin cerrar el bloque de pensamiento: `content` llega vacío. Si el turno
            // acaba sin texto ni llamadas, se recupera ese razonamiento como respuesta y se avisa.
            if text_buf.is_empty()
                && tool_calls.is_empty()
                && stream_error.is_none()
                && !cancel.is_cancelled()
                && matches!(finish, FinishReason::Stop)
                && !reasoning_buf.trim().is_empty()
            {
                text_buf = reasoning_buf.trim().to_string();
                self.bus.publish(LiveEvent::RuntimeNotice {
                    message: "sin texto: respuesta recuperada del razonamiento del modelo".into(),
                });
            }

            let mut parts = Vec::new();
            if !text_buf.is_empty() {
                parts.push(ContentPart::Text {
                    text: text_buf.clone(),
                });
            }
            if stream_error.is_none() {
                for (id, name, input) in &tool_calls {
                    parts.push(ContentPart::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
            }
            parts.push(ContentPart::Finish { reason: finish });
            let mut assistant = Message::assistant(parts);
            assistant.usage = provider_usage.clone().filter(|_| provider_usage_final);
            assistant.provider_state = provider_state.clone();
            self.store.append_message(session, &assistant).await?;
            self.bus.publish(LiveEvent::MessageAdded(assistant.clone()));
            history.push(assistant);
            final_text = text_buf;

            if cancel.is_cancelled() {
                break;
            }
            if let Some(error) = stream_error {
                return Err(crate::error::CoreError::Provider(error));
            }

            // Respuesta cortada por el tope de salida. Si traía llamadas, llegaron incompletas: no se
            // ejecutan y se le explica al modelo por qué, para que las rehaga en trozos menores. Si no
            // traía ninguna, o se repite demasiadas veces seguidas, el turno acaba y lo dice.
            if matches!(finish, FinishReason::Length) {
                length_cuts += 1;
                if tool_calls.is_empty() || length_cuts > MAX_CONSECUTIVE_LENGTH_CUTS {
                    stopped = Some(crate::agent::stop::StopReason::OutputLimit);
                    break;
                }
                for (id, _, _) in tool_calls {
                    let tr = Message::new(
                        Role::Tool,
                        vec![ContentPart::ToolResult {
                            call_id: id,
                            content: OUTPUT_LIMIT_NOTICE.into(),
                            is_error: true,
                        }],
                    );
                    self.store.append_message(session, &tr).await?;
                    self.bus.publish(LiveEvent::MessageAdded(tr.clone()));
                    history.push(tr);
                }
                continue;
            }
            length_cuts = 0;

            if tool_calls.is_empty() {
                break;
            }
            if let Some(state) = provider_state {
                provider_states_this_turn.push((
                    tool_calls.iter().map(|(id, _, _)| id.clone()).collect(),
                    state,
                ));
            }

            let mut allowed = Vec::new();
            for (id, name, input) in tool_calls {
                let req = crate::permission::PermissionRequest {
                    tool: name.clone(),
                    action: "run".into(),
                    detail: input.to_string(),
                };
                match self.permission.check(req).await {
                    crate::permission::Decision::Allow
                    | crate::permission::Decision::AllowSession => allowed.push((id, name, input)),
                    crate::permission::Decision::Deny => {
                        let tr = Message::new(
                            Role::Tool,
                            vec![ContentPart::ToolResult {
                                call_id: id,
                                content: "permission denied".into(),
                                is_error: true,
                            }],
                        );
                        self.store.append_message(session, &tr).await?;
                        self.bus.publish(LiveEvent::MessageAdded(tr.clone()));
                        history.push(tr);
                    }
                }
            }
            let mut by_id: std::collections::HashMap<String, (String, serde_json::Value)> =
                std::collections::HashMap::new();
            for (id, name, input) in &allowed {
                by_id.insert(id.clone(), (name.clone(), input.clone()));
            }

            let results = dispatch::execute_tools(
                &self.tools,
                ctx.clone(),
                allowed,
                cancel.clone(),
                self.limits.max_parallel_tools,
                self.bus.clone(),
            )
            .await;
            let mut wait_for_workflow = false;
            for (id, out) in results {
                if !out.is_error
                    && by_id.get(&id).is_some_and(|(name, input)| {
                        matches!(name.as_str(), "development_workflow" | "gauntlet")
                            && input["action"] == "start"
                    })
                {
                    wait_for_workflow = true;
                }
                if !out.is_error {
                    if let Some((name, input)) = by_id.get(&id) {
                        if name == "edit" {
                            let path = input["path"].as_str().unwrap_or_default();
                            let old = input["old"].as_str().unwrap_or_default();
                            let new = input["new"].as_str().unwrap_or_default();
                            let (added, removed) = self.changes.numbers(path, old, new);
                            self.bus.publish(LiveEvent::FileDiff {
                                path: path.to_string(),
                                added,
                                removed,
                            });
                        }
                    }
                }
                let contenido = contenido_de_herramienta(self.spill.as_ref(), &id, &out.content);
                let tr = Message::new(
                    Role::Tool,
                    vec![ContentPart::ToolResult {
                        call_id: id,
                        content: contenido,
                        is_error: out.is_error,
                    }],
                );
                self.store.append_message(session, &tr).await?;
                self.bus.publish(LiveEvent::MessageAdded(tr.clone()));
                history.push(tr);
            }
            if wait_for_workflow {
                break;
            }
        }

        Ok(TurnOutcome {
            session,
            final_text,
            steps,
            read_steps,
            stopped,
        })
    }
}

pub(super) fn programmatic_tool_calling_enabled() -> bool {
    matches!(
        std::env::var("LOOMCODING_PROGRAMMATIC_TOOL_CALLING").as_deref(),
        Ok("1" | "true" | "yes")
    )
}

pub(super) fn reattach_provider_state(
    history: &mut [Message],
    call_ids: &[String],
    state: crate::message::ProviderState,
) -> bool {
    let Some(message) = history.iter_mut().rev().find(|message| {
        message.role == Role::Assistant
            && call_ids
                .iter()
                .all(|call_id| message.tool_calls().iter().any(|(id, _, _)| id == call_id))
    }) else {
        return false;
    };
    message.provider_state = Some(state);
    true
}
