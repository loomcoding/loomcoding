//! Hilo de trabajo de un agente: turnos, cancelación y reenvío de eventos al cliente.

use super::*;

pub(super) async fn run_agent_worker(
    entry: Arc<AgentEntry>,
    agent: Arc<loomcoding_core::agent::AgentLoop>,
    mut commands: mpsc::Receiver<AgentCommand>,
    mut session: SessionId,
) {
    while let Some(command) = commands.recv().await {
        match command {
            AgentCommand::Prompt(prompt) => {
                let mut message = Message::human(prompt.text);
                message.parts.extend(prompt.images);
                if let Err(error) = run_turn_with_message(&entry, &agent, session, message).await {
                    entry
                        .ui
                        .send(ServerEvent::Live(Box::new(LiveEvent::Error {
                            message: error.to_string(),
                        })))
                        .await;
                }
            }
            AgentCommand::Wake => {
                let cancel = CancellationToken::new();
                set_active_cancel(&entry, Some(cancel.clone()));
                entry.working.store(true, Ordering::Release);
                let _ = agent.run_woken(session, cancel).await;
                entry.working.store(false, Ordering::Release);
                set_active_cancel(&entry, None);
            }
            AgentCommand::Compact => {
                agent.compact_now(session, CancellationToken::new()).await;
            }
            AgentCommand::Session(command) => match command {
                loomcoding_tui::run::SessionCommand::Refresh => send_session_list(&entry).await,
                loomcoding_tui::run::SessionCommand::RunAgent(invocation) => {
                    let label = invocation.agent.clone();
                    let request = loomcoding_core::subagent::SubagentRequest {
                        label,
                        prompt: invocation.prompt,
                        agent: Some(invocation.agent),
                        skills: invocation.skills,
                    };
                    // Orden de la persona usuaria: no hay turno del modelo principal. Un rechazo se
                    // muestra como aviso y no deja tarea.
                    if let Err(error) = agent.run_subagent_request(session, request).await {
                        let _ = entry
                            .ui
                            .send(ServerEvent::Live(Box::new(LiveEvent::RuntimeNotice {
                                message: format!("could not start the agent: {error}"),
                            })))
                            .await;
                    }
                }
                loomcoding_tui::run::SessionCommand::KillAll => {
                    // El aviso «emergency stop: N children, M jobs» lo publica el propio agente.
                    agent.kill_everything(session).await;
                }
                loomcoding_tui::run::SessionCommand::CancelSubagent(child) => {
                    match agent.cancel_subagent(session, child).await {
                        Ok(descriptor) if !descriptor.status.state.is_terminal() => {
                            let _ = entry
                                .ui
                                .send(ServerEvent::Live(Box::new(LiveEvent::RuntimeNotice {
                                    message: format!(
                                        "stop requested for subagent {}, current state: {:?}",
                                        child.0, descriptor.status.state
                                    ),
                                })))
                                .await;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            let _ = entry
                                .ui
                                .send(ServerEvent::Live(Box::new(LiveEvent::RuntimeNotice {
                                    message: format!(
                                        "could not stop subagent {}: {error}",
                                        child.0
                                    ),
                                })))
                                .await;
                        }
                    }
                }
                loomcoding_tui::run::SessionCommand::AnswerSubagent(child, text) => {
                    // Respuesta de la persona usuaria a la pregunta pendiente del hijo. Un rechazo
                    // (nada pendiente, número de preguntas distinto) se muestra como aviso.
                    if let Err(error) = agent.answer_subagent(session, child, text).await {
                        let _ = entry
                            .ui
                            .send(ServerEvent::Live(Box::new(LiveEvent::RuntimeNotice {
                                message: format!("could not answer subagent {}: {error}", child.0),
                            })))
                            .await;
                    }
                }
                loomcoding_tui::run::SessionCommand::Select(choice) => {
                    let selected = match choice {
                        loomcoding_tui::app::SessionChoice::New => {
                            entry.store.create_session(&entry.project_key).await
                        }
                        loomcoding_tui::app::SessionChoice::Resume(id) => {
                            match entry
                                .store
                                .session_belongs_to_project(id, &entry.project_key)
                                .await
                            {
                                Ok(true) => Ok(id),
                                Ok(false) => {
                                    Err(CoreError::Store("session is outside this agent".into()))
                                }
                                Err(error) => Err(error),
                            }
                        }
                    };
                    match selected {
                        Ok(id) => {
                            session = id;
                            entry.current_session.store(id.0, Ordering::Release);
                            if let Err(error) = send_loaded_session(&entry, id).await {
                                entry
                                    .ui
                                    .send(ServerEvent::Session(
                                        loomcoding_tui::run::SessionUpdate::Error(
                                            error.to_string(),
                                        ),
                                    ))
                                    .await;
                            }
                        }
                        Err(error) => {
                            entry
                                .ui
                                .send(ServerEvent::Session(
                                    loomcoding_tui::run::SessionUpdate::Error(error.to_string()),
                                ))
                                .await;
                        }
                    }
                }
            },
            AgentCommand::ProviderSettings {
                id,
                settings,
                api_key,
            } => {
                let current = entry.config.read().unwrap().clone();
                let result = crate::apply_provider_settings(
                    settings,
                    api_key.as_deref(),
                    &current,
                    &entry.config_path,
                    &agent,
                    &entry.project_env,
                );
                if let Ok((config, configured)) = &result {
                    *entry.model.write().unwrap() = config.model.clone();
                    *entry.settings.write().unwrap() = config.into();
                    *entry.config.write().unwrap() = config.clone();
                    entry
                        .api_key_configured
                        .store(*configured, Ordering::Release);
                }
                entry
                    .ui
                    .send(ServerEvent::ProviderSettingsResult {
                        id,
                        result: result
                            .map(|(_, configured)| configured)
                            .map_err(|error| error.to_string()),
                    })
                    .await;
            }
            AgentCommand::A2aPrompt {
                task,
                session,
                prompt,
                timeout,
                _permit,
            } => {
                if task.cancel.is_cancelled() {
                    task.update(loomcoding_core::task::TaskState::Canceled, None);
                    continue;
                }
                task.update(loomcoding_core::task::TaskState::Working, None);
                let cancel = task.cancel.clone();
                let turn = run_turn_with_cancel(&entry, &agent, session, prompt, cancel.clone());
                tokio::pin!(turn);
                let result = tokio::select! {
                    result = &mut turn => Some(result),
                    _ = tokio::time::sleep(timeout) => {
                        cancel.cancel();
                        set_active_cancel(&entry, None);
                        entry.working.store(false, Ordering::Release);
                        None
                    }
                };
                match result {
                    None => {
                        task.update(
                            loomcoding_core::task::TaskState::Failed,
                            Some("agent task timed out"),
                        );
                    }
                    Some(Ok(_)) if task.cancel.is_cancelled() => {
                        task.update(loomcoding_core::task::TaskState::Canceled, None);
                    }
                    Some(Ok(answer)) => {
                        task.update(loomcoding_core::task::TaskState::Completed, Some(&answer));
                    }
                    Some(Err(_)) if task.cancel.is_cancelled() => {
                        task.update(loomcoding_core::task::TaskState::Canceled, None);
                    }
                    Some(Err(_)) => {
                        task.update(
                            loomcoding_core::task::TaskState::Failed,
                            Some("agent task failed"),
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn set_active_cancel(entry: &AgentEntry, token: Option<CancellationToken>) {
    *entry.active_cancel.lock().unwrap() = token;
}

pub(super) async fn run_turn_with_cancel(
    entry: &AgentEntry,
    agent: &loomcoding_core::agent::AgentLoop,
    session: SessionId,
    prompt: String,
    cancel: CancellationToken,
) -> loomcoding_core::error::Result<String> {
    set_active_cancel(entry, Some(cancel.clone()));
    entry.working.store(true, Ordering::Release);
    let outcome = agent.run(session, prompt, cancel).await;
    set_active_cancel(entry, None);
    entry.working.store(false, Ordering::Release);
    outcome.map(|outcome| outcome.final_text)
}

pub(super) async fn run_turn_with_message(
    entry: &AgentEntry,
    agent: &loomcoding_core::agent::AgentLoop,
    session: SessionId,
    message: Message,
) -> loomcoding_core::error::Result<String> {
    let cancel = CancellationToken::new();
    set_active_cancel(entry, Some(cancel.clone()));
    entry.working.store(true, Ordering::Release);
    let outcome = agent.run_message(session, message, cancel).await;
    set_active_cancel(entry, None);
    entry.working.store(false, Ordering::Release);
    outcome.map(|outcome| outcome.final_text)
}

pub(super) async fn forward_agent_events(
    _manager: Arc<AgentManager>,
    entry: Arc<AgentEntry>,
    mut events: tokio::sync::broadcast::Receiver<LiveEvent>,
) {
    while let Ok(event) = events.recv().await {
        if matches!(event, LiveEvent::AgentRegistryChanged { .. }) {
            let previous = entry.runtime_capabilities.read().unwrap().clone();
            let updated = runtime_capabilities(
                &entry.registry.snapshot(),
                previous.mcp_config,
                previous.mcp_servers,
                previous.rust_analyzer_available,
            );
            *entry.runtime_capabilities.write().unwrap() = updated.clone();
            entry.ui.send(ServerEvent::Capabilities(updated)).await;
        }
        match &event {
            LiveEvent::TurnStarted => {
                *entry.status.write().unwrap() = "working".into();
                entry.activity.write().unwrap().take();
            }
            LiveEvent::TurnEnded | LiveEvent::WakeIdle => {
                *entry.status.write().unwrap() = "idle".into();
                entry.activity.write().unwrap().take();
            }
            LiveEvent::ToolStarted { name, .. } => {
                *entry.activity.write().unwrap() =
                    Some(loomcoding_core::tools::limit_bytes(name, 64).into_owned());
            }
            LiveEvent::ToolFinished { .. } => {
                entry.activity.write().unwrap().take();
            }
            LiveEvent::JobChanged(job) => {
                let mut jobs = entry.jobs.lock().unwrap();
                if matches!(
                    job.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Killed
                ) {
                    jobs.remove(&job.id);
                } else {
                    jobs.insert(job.id.clone());
                }
            }
            LiveEvent::Error { .. } => {
                *entry.status.write().unwrap() = "error".into();
            }
            _ => {}
        }
        let bounded = bounded_wire(event);
        entry.ui.send(ServerEvent::Live(Box::new(bounded))).await;
    }
}
