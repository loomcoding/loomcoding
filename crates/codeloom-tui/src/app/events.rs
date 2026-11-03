//! Aplicación de los `LiveEvent` al estado de la interfaz.

use super::*;

impl App {
    pub fn apply(&mut self, ev: LiveEvent) {
        match ev {
            LiveEvent::TurnStarted => {
                // Solo un turno despertado gasta presupuesto; un turno del usuario no.
                let woken = std::mem::take(&mut self.wake_in_flight);
                if woken {
                    self.auto_turns_left = self.auto_turns_left.saturating_sub(1);
                    // La causa del despertar se apunta en el transcript como línea de interfaz;
                    // no pasa al historial del modelo ni se persiste.
                    if let Some(line) = self.wake_cause_line() {
                        self.transcript.push_entry(
                            batten::state::LineKind::System,
                            batten::state::Format::Plain,
                            line,
                        );
                    }
                    self.wake_causes.clear();
                }
                self.turn_in_flight = true;
                self.status = if woken { "auto-turn" } else { "working" }.into();
                self.finish_phase();
                self.phase_durations.clear();
                self.phase = "preparing".into();
                self.phase_elapsed = Duration::ZERO;
                self.phase_started_at = Some(Instant::now());
                self.matrix_rain.start();
                self.reasoning.clear();
                self.thinking = false;
                self.current_step = 0;
                self.max_steps = 0;
                self.turn_input_tokens = 0;
                self.turn_output_tokens = 0;
            }
            LiveEvent::StepStarted { step, max_steps } => {
                self.current_step = step;
                self.max_steps = max_steps;
                self.set_phase("waiting for provider");
            }
            LiveEvent::TurnEnded => {
                // El texto ya no crece: los bloques largos se pliegan.
                self.transcript.settle();
                self.turn_in_flight = false;
                self.wake_in_flight = false;
                self.cancel_requested = false;
                self.status = "ready".into();
                self.finish_phase();
                self.matrix_rain.stop();
                self.assistant = None;
                self.thinking = false;
            }
            LiveEvent::Delta { text } => {
                self.set_phase("responding");
                self.thinking = false;
                self.push_assistant(&text)
            }
            LiveEvent::ReasoningDelta { text } => {
                self.set_phase("thinking");
                self.thinking = true;
                self.reasoning.push_str(&text);
            }
            LiveEvent::Usage {
                input_tokens,
                output_tokens,
            } => {
                self.add_usage_counters(input_tokens, output_tokens);
                // Solo el uso de la sesión mostrada fija el medidor; el de los hijos llega por
                // `ChildEvent` y no pasa por aquí.
                if input_tokens > 0 {
                    self.context_tokens = u64::from(input_tokens);
                }
            }
            event @ LiveEvent::UsageProgress { .. } => {
                if let Some(session) = self.session_id {
                    self.apply_usage_progress(session, &event);
                }
            }
            // Solo el desglose de la sesión mostrada: el de los hijos no se reenvía.
            LiveEvent::ContextBreakdown { breakdown, .. } => {
                self.context_breakdown = Some(breakdown);
            }
            // El consumo de resumir cuenta como gasto igual que cualquier llamada, pero no es el
            // contexto del turno: no fija el medidor.
            LiveEvent::CompactionUsage {
                input_tokens,
                output_tokens,
            } => self.add_usage_counters(input_tokens, output_tokens),
            LiveEvent::ReportedCost { usd } => {
                self.set_reported_cost(self.session_id.unwrap_or(SessionId(0)), usd);
            }
            // El resumen ya llega como mensaje (`MessageAdded`), que `parse_notice` reconoce y
            // pinta como aviso propio. Aquí no se añade una segunda línea. Lo que sí cambia es la
            // memoria del medidor: el resumen vigente pasa a contar como memoria.
            LiveEvent::ContextCompacted { summary_bytes, .. } => {
                self.context_memory = estimate_tokens(summary_bytes);
                // La compactación se ha activado: el resumen anticipado deja de estar pendiente.
                self.background_summary = BackgroundSummary::None;
            }
            // El resumen anticipado arranca: la línea de estado cuenta «resumiendo…» desde aquí.
            LiveEvent::BackgroundCompactionStarted { .. } => {
                self.background_summary = BackgroundSummary::Running {
                    since: Instant::now(),
                };
            }
            // El resumen anticipado está listo: el medidor lleva la flecha «ahorro pendiente».
            LiveEvent::BackgroundCompactionReady { .. } => {
                self.background_summary = BackgroundSummary::Ready;
            }
            // El resumen anticipado no llegó a activarse: ni «resumiendo…» ni flecha pendientes.
            LiveEvent::BackgroundCompactionDiscarded => {
                self.background_summary = BackgroundSummary::None;
            }
            // El aviso de presupuesto es de CodeLoom: se pinta como línea de sistema, igual que los
            // avisos de `parse_notice`, y nunca como mensaje de la persona usuaria ni como estado de
            // A2A. Corta la entrada del asistente para que lo que llegue después no se pegue encima.
            LiveEvent::BudgetWarning {
                metric,
                level_percent,
                used,
                limit,
                partial,
            } => {
                let linea = budget_warning_line(metric, level_percent, used, limit, partial);
                self.transcript.push_entry(
                    batten::state::LineKind::System,
                    batten::state::Format::Plain,
                    linea,
                );
                self.assistant = None;
            }
            LiveEvent::ToolStarted { call_id, name } => {
                if name == "lsp" {
                    self.lsp_invoked = true;
                }
                self.set_phase("running tools");
                let tool = self.tool_mut(&call_id, &name);
                tool.start();
                self.sync_tool(&call_id);
            }
            LiveEvent::ToolFinished { call_id, is_error } => {
                if let Some(tool) = self.tools.iter_mut().find(|tool| tool.call_id == call_id) {
                    tool.set_result(is_error);
                }
                self.sync_tool(&call_id);
                if !self
                    .tools
                    .iter()
                    .any(|tool| tool.status() == batten::state::ToolStatus::Running)
                {
                    self.finish_phase_as(if is_error {
                        "tool failed"
                    } else {
                        "tools complete"
                    });
                }
            }
            LiveEvent::MessageAdded(message) => self.apply_message_added(message),
            LiveEvent::SessionMessageAdded { session, message } => {
                if self.session_id == Some(session) {
                    self.apply_message_added(message);
                }
            }
            LiveEvent::Error { message } => {
                self.turn_in_flight = false;
                self.wake_in_flight = false;
                self.cancel_requested = false;
                self.status = format!("error: {message}");
                self.transcript.push_entry(
                    batten::state::LineKind::System,
                    batten::state::Format::Plain,
                    format!("error: {message}"),
                );
                self.finish_phase();
                self.thinking = false;
                self.matrix_rain.stop();
            }
            LiveEvent::RuntimeNotice { message } => {
                if self.status.starts_with("stopping subagent") {
                    self.status = "ready".into();
                }
                // Detectar mensajes de proveedor para spinner en status bar
                let is_provider_start = message.contains("provider request started");
                let is_provider_end = message.contains("provider response received");
                if is_provider_start {
                    self.provider_in_flight = self.provider_in_flight.saturating_add(1);
                    // Estilo nuevo por petición: el composer cambia de indicador.
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as u64)
                        .unwrap_or(0);
                    self.composer
                        .set_activity_style(batten::widgets::activity::pick_activity_style(seed));
                    self.composer.set_working(!self.verbose_provider_notices);
                }
                if is_provider_end && self.provider_in_flight > 0 {
                    self.provider_in_flight = self.provider_in_flight.saturating_sub(1);
                    if self.provider_in_flight == 0 {
                        self.composer.set_working(false);
                    }
                }
                // Solo añadir al transcript si verbose_provider_notices está activado
                if self.verbose_provider_notices || (!is_provider_start && !is_provider_end) {
                    self.transcript.push_entry(
                        batten::state::LineKind::System,
                        batten::state::Format::Plain,
                        message,
                    );
                }
                self.assistant = None;
            }
            LiveEvent::FileDiff {
                path,
                added,
                removed,
            } => {
                if let Some(session) = self.session_id {
                    self.record_file(session.0, &path, added, removed);
                }
                self.changes.upsert(batten::state::FileChange {
                    path,
                    added,
                    removed,
                });
            }
            LiveEvent::JobChanged(snapshot) => {
                if snapshot.kind == "subagent" {
                    if let Some(subagent) = self
                        .subagents
                        .values_mut()
                        .find(|subagent| subagent.job_id == snapshot.id.0)
                    {
                        // Solo el cierre del job fija el estado: mientras sigue en marcha, la tarea
                        // conserva su estado real (`working`, `input-required`, `auth-required`...).
                        // Si el cierre no cambia el estado, se conserva el `TaskStatus` tal cual.
                        let state = TaskState::from(snapshot.status);
                        if state.is_terminal() && subagent.status.state != state {
                            subagent.status = TaskStatus::new(state);
                        }
                    }
                }
                self.sync_job_activity(&snapshot);
                self.jobs.insert(snapshot.id.clone(), snapshot);
            }
            LiveEvent::SubagentChanged(subagent) => {
                if self.in_open_tree(subagent.parent) {
                    let first_seen = !self.subagents.contains_key(&subagent.child.0);
                    if first_seen && !subagent.status.state.is_terminal() {
                        self.expanded_nodes.insert(subagent.child.0);
                    }
                    if self.status.starts_with("stopping subagent")
                        && subagent.status.state.is_terminal()
                    {
                        self.status = "ready".into();
                    }
                    // Primero en el mapa: el camino y la sangría del nodo se derivan de él.
                    self.subagents.insert(subagent.child.0, subagent.clone());
                    // Lo que esperaba a este hijo se entrega o se descarta según su estado.
                    self.on_child_state_changed(subagent.child, subagent.status.state);
                    self.sync_subagent_activity(&subagent);
                    // Una llamada del padre que llegó antes que su hijo puede colgar ahora de él.
                    self.resync_tools();
                }
            }
            LiveEvent::DevelopmentWorkflowChanged(summary) => {
                if Some(summary.parent) == self.session_id {
                    let phase = match summary.phase {
                        codeloom_core::workflow::WorkflowPhase::Specifying => "crear spec+plan",
                        codeloom_core::workflow::WorkflowPhase::AwaitingPlanApproval => {
                            "aprobar spec+plan"
                        }
                        codeloom_core::workflow::WorkflowPhase::Implementing => "implementando",
                        codeloom_core::workflow::WorkflowPhase::Reviewing => "revisando",
                        codeloom_core::workflow::WorkflowPhase::ChangesRequested => "corregir",
                        codeloom_core::workflow::WorkflowPhase::AwaitingFinalAcceptance => {
                            "aceptar resultado"
                        }
                        codeloom_core::workflow::WorkflowPhase::Completed => "completado",
                        codeloom_core::workflow::WorkflowPhase::Cancelled => "cancelado",
                        codeloom_core::workflow::WorkflowPhase::Failed => "fallido",
                    };
                    let agent = if summary.active_child.is_none() {
                        "coordinador"
                    } else {
                        match summary.phase {
                            codeloom_core::workflow::WorkflowPhase::Specifying => &summary.planner,
                            codeloom_core::workflow::WorkflowPhase::Implementing
                            | codeloom_core::workflow::WorkflowPhase::ChangesRequested => {
                                &summary.implementer
                            }
                            codeloom_core::workflow::WorkflowPhase::Reviewing => &summary.reviewer,
                            _ => "coordinador",
                        }
                    };
                    self.transcript.push_entry(
                        batten::state::LineKind::System,
                        batten::state::Format::Plain,
                        format!(
                            "flujo #{} · {} · {} · ronda {}",
                            summary.id.0, phase, agent, summary.correction_round
                        ),
                    );
                }
            }
            LiveEvent::AgentRouted {
                parent,
                agent,
                reason,
                source,
            } => {
                // La elección automática nunca es silenciosa: qué agente, quién decidió y por qué.
                if Some(parent) == self.session_id {
                    self.transcript.push_entry(
                        batten::state::LineKind::System,
                        batten::state::Format::Plain,
                        format!("agente elegido automáticamente: {agent} ({source}) · {reason}"),
                    );
                }
            }
            LiveEvent::A2aServed { task, agent, state } => {
                // Un tercero está dictando trabajo a un modelo local: la persona usuaria ha de verlo.
                self.record_a2a_observation(A2aObservation {
                    direction: A2aDirection::Local,
                    agent: agent.clone(),
                    operation: A2aOperation::TaskStateObserved,
                    rpc_id: None,
                    task_id: Some(task.clone()),
                    context_id: self.session_id.map(|id| id.0.to_string()),
                    state: Some(state.clone()),
                    artifact: None,
                    succeeded: true,
                    elapsed_ms: None,
                });
                self.transcript.push_entry(
                    batten::state::LineKind::System,
                    batten::state::Format::Plain,
                    format!("A2A entrante: tarea {task} · agente {agent} · {state}"),
                );
            }
            LiveEvent::A2aObserved(observation) => self.record_a2a_observation(observation),
            LiveEvent::AgentCardsRefreshed { ok, failed } => {
                let texto = if failed.is_empty() {
                    format!("tarjetas de agentes remotos al día: {ok}")
                } else {
                    // Los nombres vienen del perfil, que es de la persona usuaria, pero se acotan.
                    let nombres: Vec<&str> = failed.iter().take(10).map(String::as_str).collect();
                    format!(
                        "tarjetas de agentes remotos: {ok} al día; fallaron {}: {}",
                        failed.len(),
                        nombres.join(", ")
                    )
                };
                self.transcript.push_entry(
                    batten::state::LineKind::System,
                    batten::state::Format::Plain,
                    texto,
                );
            }
            LiveEvent::TodosChanged { session, items } => {
                if self.session_id == Some(session) {
                    self.todos = items;
                    self.sync_todos();
                }
            }
            LiveEvent::SubagentInputRequested { parent, child, .. } => {
                // La espera la fija el estado de la tarea (`SubagentChanged`, `input-required`);
                // aquí solo se despierta al padre.
                if Some(parent) == self.session_id {
                    // Una pregunta del hijo pide que el padre lea el aviso.
                    self.wake_pending = true;
                    self.wake_causes.push((child.0, "asks input"));
                }
            }
            // La vuelta a `working` llega como `SubagentChanged`: no hay nada que mantener aquí.
            LiveEvent::SubagentInputResolved { .. } => {}
            // La fase fina del hijo (A2A sigue mandando: `status.state` no se toca).
            LiveEvent::SubagentPhaseChanged { child, phase, .. } => {
                if let Some(subagent) = self.subagents.get_mut(&child.0) {
                    subagent.phase = phase;
                }
            }
            LiveEvent::WakeIdle => self.wake_in_flight = false,
            // La compactación manual terminó, bien o mal: se libera la cola y se dice cómo acabó.
            // El resumen en sí ya se pintó con `ContextCompacted`.
            LiveEvent::ManualCompactionEnded { outcome } => {
                use codeloom_core::event::ManualCompaction;
                self.compact_in_flight = false;
                self.slash_feedback = Some(
                    match outcome {
                        ManualCompaction::Compacted => "Older history summarized",
                        ManualCompaction::NothingToSummarize => "Nothing to summarize yet",
                        ManualCompaction::Failed => "Compaction failed; history unchanged",
                    }
                    .into(),
                );
            }
            LiveEvent::ChildEvent {
                parent,
                child,
                event,
            } => {
                if self.in_open_tree(parent) {
                    // El presupuesto es de la familia: el aviso que cruza un hijo es el mismo del
                    // padre y se pinta igual, no como métrica del hijo.
                    if let LiveEvent::BudgetWarning {
                        metric,
                        level_percent,
                        used,
                        limit,
                        partial,
                    } = event.as_ref()
                    {
                        let linea =
                            budget_warning_line(*metric, *level_percent, *used, *limit, *partial);
                        self.transcript.push_entry(
                            batten::state::LineKind::System,
                            batten::state::Format::Plain,
                            linea,
                        );
                        self.assistant = None;
                        return;
                    }
                    // El consumo de un hijo se atribuye al modelo actual de la interfaz, pero
                    // con la sesión del hijo para que su coste se pueda separar.
                    if let LiveEvent::Usage {
                        input_tokens,
                        output_tokens,
                        ..
                    }
                    | LiveEvent::CompactionUsage {
                        input_tokens,
                        output_tokens,
                    } = event.as_ref()
                    {
                        self.record_live_usage(child, *input_tokens, *output_tokens, None);
                    }
                    if matches!(event.as_ref(), LiveEvent::UsageProgress { .. }) {
                        self.apply_usage_progress(child, event.as_ref());
                    }
                    if let LiveEvent::ReportedCost { usd } = event.as_ref() {
                        self.set_reported_cost(child, *usd);
                    }
                    // Un evento que cambia el registro o las métricas del hijo redibuja su nodo.
                    let relevante = event.is_child_relevant();
                    self.apply_child_event(child, *event);
                    if relevante {
                        self.refresh_child_node(child.0);
                    }
                }
            }
            LiveEvent::SubagentSettled {
                parent,
                child,
                status,
                ..
            } => {
                // Un hijo que termina bien o con fallo despierta al padre; uno matado
                // por el usuario o sin terminar no.
                let motivo = match status {
                    codeloom_core::task::TaskState::Completed => Some("completed"),
                    codeloom_core::task::TaskState::Failed => Some("failed"),
                    _ => None,
                };
                if let Some(motivo) = motivo.filter(|_| Some(parent) == self.session_id) {
                    self.wake_pending = true;
                    self.wake_causes.push((child.0, motivo));
                }
            }
            // El registro de agentes cambió. La interfaz aún no lo muestra: mostrarlo no es
            // objetivo del registro (spec del registro de agentes, «Eventos y observabilidad»).
            LiveEvent::AgentRegistryChanged { .. } => {}
            LiveEvent::SkillsLoaded { .. } => {}
        }
    }

    pub(super) fn apply_message_added(&mut self, message: Message) {
        if message.role == Role::User {
            let text = message.text();
            if message.origin == MessageOrigin::Human {
                self.push_user(&text);
            } else {
                self.internal_user_message(&text);
            }
        } else {
            for part in message.parts {
                match part {
                    ContentPart::ToolCall { id, name, input } => {
                        let facts = tool_facts(&name, &input);
                        let tool = self.tool_mut(&id, &name);
                        tool.set_summary(input.to_string());
                        tool.facts = facts;
                        self.sync_tool(&id);
                    }
                    ContentPart::ToolResult {
                        call_id,
                        content,
                        is_error,
                    } => {
                        if let Some(tool) =
                            self.tools.iter_mut().find(|tool| tool.call_id == call_id)
                        {
                            tool.set_result(is_error);
                            tool.set_output(content);
                        }
                        self.sync_tool(&call_id);
                    }
                    _ => {}
                }
            }
        }
    }
}
