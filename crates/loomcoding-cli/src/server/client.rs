//! Cliente: validación de la petición, órdenes del usuario y bucle de la TUI contra el daemon.

use super::*;

pub(super) fn validate_cwd(value: String) -> anyhow::Result<PathBuf> {
    if value.is_empty() || value.len() > 32_000 {
        anyhow::bail!("invalid project path");
    }
    let path = std::fs::canonicalize(value)?;
    if !path.is_dir() {
        anyhow::bail!("project path is not a directory");
    }
    Ok(path)
}

pub(super) async fn handle_client_command(
    entry: &AgentEntry,
    command: ClientCommand,
) -> anyhow::Result<()> {
    match command {
        ClientCommand::Prompt(prompt) => {
            validate_user_prompt(&prompt)?;
            entry.commands.send(AgentCommand::Prompt(prompt)).await?;
        }
        ClientCommand::Wake => entry.commands.send(AgentCommand::Wake).await?,
        ClientCommand::Compact => entry.commands.send(AgentCommand::Compact).await?,
        ClientCommand::CancelTurn => {
            if let Some(cancel) = entry.active_cancel.lock().unwrap().as_ref() {
                cancel.cancel();
            }
        }
        ClientCommand::RefreshAgents => {
            if let Some(refresh) = &entry.refresher {
                let _ = refresh.send(());
            }
        }
        ClientCommand::Session(command) => {
            // Los comandos de sesión se encolan tras el turno en curso. La parada de emergencia no
            // puede esperar: se cancela el turno ya y el trabajador cancela hijos y jobs al soltarlo.
            if matches!(command, loomcoding_tui::run::SessionCommand::KillAll) {
                if let Some(cancel) = entry.active_cancel.lock().unwrap().as_ref() {
                    cancel.cancel();
                }
            }
            entry.commands.send(AgentCommand::Session(command)).await?
        }
        ClientCommand::ProviderSettings {
            id,
            settings,
            api_key,
        } => {
            entry
                .commands
                .send(AgentCommand::ProviderSettings {
                    id,
                    settings,
                    api_key,
                })
                .await?
        }
        ClientCommand::PermissionReply { id, decision } => {
            entry.ui.resolve_decision(id, decision).await
        }
        ClientCommand::QuestionReply { id, answers } => {
            entry.ui.resolve_questions(id, answers).await
        }
        ClientCommand::Disconnect => anyhow::bail!("client disconnected"),
    }
    Ok(())
}

pub(super) fn validate_user_prompt(
    prompt: &loomcoding_tui::app::PendingPrompt,
) -> anyhow::Result<()> {
    if prompt.text.is_empty() || prompt.text.len() > 32_000 {
        anyhow::bail!("prompt must be 1 to 32000 bytes");
    }
    let images = prompt
        .images
        .iter()
        .filter_map(|part| match part {
            loomcoding_core::message::ContentPart::Image {
                mime_type,
                data,
                filename,
            } => Some((mime_type.as_str(), data.as_str(), filename.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if images.len() != prompt.images.len() || images.len() > loomcoding_tui::app::MAX_PROMPT_IMAGES
    {
        anyhow::bail!("prompt contains unsupported image parts");
    }
    let total_encoded = images.iter().map(|(_, data, _)| data.len()).sum::<usize>();
    if total_encoded
        > loomcoding_tui::app::MAX_PROMPT_IMAGES * loomcoding_tui::app::MAX_IMAGE_BASE64_BYTES
        || images.iter().any(|(mime, data, name)| {
            !matches!(
                *mime,
                "image/png" | "image/jpeg" | "image/gif" | "image/webp"
            ) || data.len() > 2_796_204
                || name.len() > 255
                || name.contains('/')
                || name.contains('\\')
        })
    {
        anyhow::bail!("prompt images exceed supported limits");
    }
    for (mime, data, _) in &images {
        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| anyhow::anyhow!("prompt image is not valid Base64"))?;
        let valid = match *mime {
            "image/png" => decoded.starts_with(b"\x89PNG\r\n\x1a\n"),
            "image/jpeg" => decoded.starts_with(b"\xff\xd8\xff"),
            "image/gif" => decoded.starts_with(b"GIF87a") || decoded.starts_with(b"GIF89a"),
            "image/webp" => {
                decoded.len() >= 12 && &decoded[..4] == b"RIFF" && &decoded[8..12] == b"WEBP"
            }
            _ => false,
        };
        if !valid || decoded.len() > loomcoding_tui::app::MAX_IMAGE_BYTES {
            anyhow::bail!("prompt image does not match its media type or size limit");
        }
    }
    Ok(())
}

pub async fn run_client(cli: Cli) -> anyhow::Result<()> {
    let mut stream = connect_or_start().await?;
    if let Some(prompt) = cli.print {
        ipc::send(
            &mut stream,
            &ClientFrame::Print {
                cwd: client_cwd()?,
                prompt,
                yolo: cli.yolo,
            },
        )
        .await?;
        return match ipc::receive::<ServerFrame>(&mut stream).await? {
            ServerFrame::Event(event) => match *event {
                ServerEvent::PrintResult { result: Ok(text) } => {
                    println!("{text}");
                    Ok(())
                }
                ServerEvent::PrintResult { result: Err(error) } => anyhow::bail!("{error}"),
                ServerEvent::Error { message } => anyhow::bail!("{message}"),
                _ => anyhow::bail!("respuesta inesperada del servidor local"),
            },
            _ => anyhow::bail!("respuesta inesperada del servidor local"),
        };
    }
    ipc::send(
        &mut stream,
        &ClientFrame::Open {
            cwd: client_cwd()?,
            attach_agent: cli.attach,
            yolo: cli.yolo,
        },
    )
    .await?;
    run_client_tui(stream).await
}

pub(super) fn client_cwd() -> anyhow::Result<String> {
    let herdr = loomcoding_herdr::HerdrContext::from_env();
    let cwd = herdr
        .as_ref()
        .and_then(|context| context.workspace_cwd.as_ref())
        .filter(|path| path.is_dir())
        .cloned()
        .unwrap_or(std::env::current_dir()?);
    Ok(cwd.to_string_lossy().into_owned())
}

pub(super) type PendingSettings =
    Arc<StdMutex<HashMap<u64, oneshot::Sender<Result<bool, String>>>>>;

pub(super) async fn run_client_tui(stream: ipc::Stream) -> anyhow::Result<()> {
    use loomcoding_tui::run::{ProviderSettingsCommand, SessionCommand, SessionUpdate};
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (frame_tx, mut frame_rx) = mpsc::channel::<ClientFrame>(128);
    let writer_task = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            if ipc::send(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });

    let (prompt_tx, mut prompt_rx) =
        mpsc::unbounded_channel::<loomcoding_tui::app::PendingPrompt>();
    let (wake_tx, mut wake_rx) = mpsc::unbounded_channel::<()>();
    let (compact_tx, mut compact_rx) = mpsc::unbounded_channel::<()>();
    let (settings_tx, mut settings_rx) = mpsc::unbounded_channel::<ProviderSettingsCommand>();
    let (session_command_tx, mut session_command_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (session_update_tx, session_update_rx) = mpsc::unbounded_channel::<SessionUpdate>();
    let (perm_tx, perm_rx) = mpsc::channel(8);
    let (ask_tx, ask_rx) = mpsc::channel(4);
    let (catalog_tx, catalog_rx) = mpsc::unbounded_channel();
    let pending_settings: PendingSettings = Arc::new(StdMutex::new(HashMap::new()));
    let settings_pending = pending_settings.clone();
    let frame_commands = frame_tx.clone();
    tokio::spawn(async move {
        let mut next_settings_id = 1u64;
        loop {
            tokio::select! {
                value = prompt_rx.recv() => match value {
                    Some(prompt) => if frame_commands.send(ClientFrame::Command(ClientCommand::Prompt(prompt))).await.is_err() { break; },
                    None => break,
                },
                value = wake_rx.recv() => match value {
                    Some(()) => if frame_commands.send(ClientFrame::Command(ClientCommand::Wake)).await.is_err() { break; },
                    None => break,
                },
                value = compact_rx.recv() => match value {
                    Some(()) => if frame_commands.send(ClientFrame::Command(ClientCommand::Compact)).await.is_err() { break; },
                    None => break,
                },
                value = session_command_rx.recv() => match value {
                    Some(command) => if frame_commands.send(ClientFrame::Command(ClientCommand::Session(command))).await.is_err() { break; },
                    None => break,
                },
                value = settings_rx.recv() => match value {
                    Some(command) => {
                        let id = next_settings_id;
                        next_settings_id = next_settings_id.wrapping_add(1).max(1);
                        settings_pending.lock().unwrap().insert(id, command.reply);
                        if frame_commands.send(ClientFrame::Command(ClientCommand::ProviderSettings { id, settings: command.settings, api_key: command.api_key })).await.is_err() { break; }
                    }
                    None => break,
                },
            }
        }
    });

    let (ready_tx, ready_rx) = oneshot::channel();
    let reader_frames = frame_tx.clone();
    let active_turn: loomcoding_tui::run::ActiveTurn = Arc::new(std::sync::Mutex::new(None));
    let reader_active = active_turn.clone();
    let reader_pending_settings = pending_settings.clone();
    let bus = loomcoding_core::event::EventBus::new(1024);
    let bus_rx = bus.subscribe();
    let reader_bus = bus.clone();
    let reader_task = tokio::spawn(async move {
        let mut ready_tx = Some(ready_tx);
        let mut session_items = Vec::new();
        let mut loaded: Option<LoadedSession> = None;
        let mut turn_end: Option<oneshot::Sender<()>> = None;
        while let Ok(frame) = ipc::receive::<ServerFrame>(&mut reader).await {
            let ServerFrame::Event(event) = frame else {
                continue;
            };
            match *event {
                ServerEvent::Ready {
                    agent_id,
                    runtime_generation,
                    cwd,
                    model,
                    settings,
                    api_key_configured,
                } => {
                    if let Some(reply) = ready_tx.take() {
                        let _ = reply.send(Ok((
                            agent_id,
                            runtime_generation,
                            cwd,
                            model,
                            settings,
                            api_key_configured,
                        )));
                    }
                }
                ServerEvent::Capabilities(capabilities) => {
                    let _ = session_update_tx.send(SessionUpdate::Capabilities(capabilities));
                }
                ServerEvent::Error { message } if ready_tx.is_some() => {
                    if let Some(reply) = ready_tx.take() {
                        let _ = reply.send(Err(message));
                    }
                    break;
                }
                ServerEvent::Error { message } => {
                    let _ = session_update_tx.send(SessionUpdate::Error(message));
                }
                ServerEvent::Live(event) => {
                    match &*event {
                        LiveEvent::TurnStarted => {
                            if let Some(end) = turn_end.take() {
                                let _ = end.send(());
                            }
                            let cancel = CancellationToken::new();
                            *reader_active.lock().unwrap() = Some(cancel.clone());
                            let (end_tx, end_rx) = oneshot::channel();
                            let frame_tx = reader_frames.clone();
                            tokio::spawn(async move {
                                tokio::select! {
                                    _ = cancel.cancelled() => { let _ = frame_tx.send(ClientFrame::Command(ClientCommand::CancelTurn)).await; }
                                    _ = end_rx => {}
                                }
                            });
                            turn_end = Some(end_tx);
                        }
                        LiveEvent::TurnEnded => {
                            if let Some(end) = turn_end.take() {
                                let _ = end.send(());
                            }
                            *reader_active.lock().unwrap() = None;
                        }
                        _ => {}
                    }
                    reader_bus.publish(*event);
                }
                ServerEvent::Session(update) => {
                    let _ = session_update_tx.send(update);
                }
                ServerEvent::SessionListPage { items, done } => {
                    session_items.extend(items);
                    if done {
                        let _ = session_update_tx
                            .send(SessionUpdate::List(std::mem::take(&mut session_items)));
                    }
                }
                ServerEvent::SessionLoadedPage {
                    session,
                    messages,
                    usage,
                    subagents,
                    done,
                } => {
                    let state =
                        loaded.get_or_insert_with(|| (session, Vec::new(), Vec::new(), Vec::new()));
                    if state.0 != session {
                        loaded = Some((session, Vec::new(), Vec::new(), Vec::new()));
                    }
                    let state = loaded.as_mut().unwrap();
                    state.1.extend(messages);
                    state.2.extend(usage);
                    state.3.extend(subagents);
                    if done {
                        if let Some((id, messages, usage, subagents)) = loaded.take() {
                            let _ = session_update_tx.send(SessionUpdate::Loaded(id, messages));
                            let _ = session_update_tx.send(SessionUpdate::Usage(id, usage));
                            let _ = session_update_tx.send(SessionUpdate::Subagents(id, subagents));
                        }
                    }
                }
                ServerEvent::PermissionRequest { id, request } => {
                    let (reply_tx, reply_rx) = oneshot::channel();
                    if perm_tx.send((request, reply_tx)).await.is_ok() {
                        let frame_tx = reader_frames.clone();
                        tokio::spawn(async move {
                            if let Ok(decision) = reply_rx.await {
                                let _ = frame_tx
                                    .send(ClientFrame::Command(ClientCommand::PermissionReply {
                                        id,
                                        decision,
                                    }))
                                    .await;
                            }
                        });
                    }
                }
                ServerEvent::Questions { id, questions } => {
                    let (reply_tx, reply_rx) = oneshot::channel();
                    if ask_tx.send((questions, reply_tx)).await.is_ok() {
                        let frame_tx = reader_frames.clone();
                        tokio::spawn(async move {
                            if let Ok(answers) = reply_rx.await {
                                let _ = frame_tx
                                    .send(ClientFrame::Command(ClientCommand::QuestionReply {
                                        id,
                                        answers,
                                    }))
                                    .await;
                            }
                        });
                    }
                }
                ServerEvent::ProviderSettingsResult { id, result } => {
                    if let Some(reply) = reader_pending_settings.lock().unwrap().remove(&id) {
                        let _ = reply.send(result);
                    }
                }
                ServerEvent::PrintResult { .. } | ServerEvent::Catalog(_) => {}
            }
        }
    });

    let (agent_id, runtime_generation, cwd, model, settings, api_key_configured) = ready_rx
        .await
        .map_err(|_| anyhow::anyhow!("local server closed before opening the agent"))?
        .map_err(anyhow::Error::msg)?;
    let cwd_path = PathBuf::from(&cwd);
    let config_path = cwd_path.join("loomcoding.toml");
    let agents_config = loomcoding_core::config::AgentsConfig::load(&config_path)?;
    let a2a_config = loomcoding_a2a::config::A2aConfig::load(&config_path)?;
    // Preferencias de UI del proyecto (verbose_provider_notices, etc.).
    let ui_config = loomcoding_core::config::UiConfig::load(&config_path).unwrap_or_default();
    // `[flows.<kind>] enabled` para la cabecera: el mismo valor con que el daemon arranca cada modo.
    let flows_config = loomcoding_core::config::FlowsConfig::load(&config_path).unwrap_or_default();
    let flow_modes: Vec<loomcoding_core::flow::FlowMode> = loomcoding_workflows::flow_modes()
        .into_iter()
        .map(|modo| loomcoding_core::flow::FlowMode {
            enabled: flows_config.enabled(&modo.kind, modo.enabled),
            ..modo
        })
        .collect();
    let data_dir = crate::home::user_home()
        .unwrap_or_else(std::env::temp_dir)
        .join(".loomcoding");
    let runtime_peers = a2a_config
        .consumes()
        .then(|| crate::runtime_peer_dir(&data_dir, &runtime_generation, &cwd_path));
    let startup = crate::arrancar_agentes_con_runtime(
        crate::home::user_home().as_deref(),
        &cwd_path,
        &agents_config,
        runtime_peers,
    );
    let registry = Arc::new(startup.registro);
    let registry_bus = loomcoding_core::event::EventBus::new(32);
    let _watcher = registry.spawn_watcher(startup.directorios, registry_bus.clone());
    let _remote_clients = crate::arrancar_remotos(&a2a_config, &registry, &registry_bus)?;
    let local_refresher = crate::arrancar_refresco_manual(&a2a_config, &registry, &registry_bus)?;
    if let Some(refresher) = local_refresher.clone() {
        let mut events = registry_bus.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                if matches!(event, LiveEvent::AgentRegistryChanged { .. }) {
                    let _ = refresher.send(());
                }
            }
        });
    }
    // La ventana declarada depende del proveedor y del modelo, que solo se conocen ya abierto el agente.
    let limits =
        loomcoding_core::config::ProviderLimitsConfig::load(&config_path).unwrap_or_default();
    let declared = crate::declared_window(&limits, &settings.base_url, &settings.model);
    let catalog = Arc::new(std::sync::RwLock::new(
        loomcoding_core::catalog::Catalog::default(),
    ));
    // Los servidores locales del registro global se sondean en segundo plano (véase `sondeo_local`).
    let locales = crate::registro_local::cargar_registro_global()
        .map(|registro| registro.providers)
        .unwrap_or_default();
    let publicador = crate::sondeo_local::Publicador::new(catalog_tx, catalog, declared, locales);
    tokio::spawn(crate::load_catalog(publicador.clone()));
    let refresco_proveedores = crate::sondeo_local::arrancar_sondeo(publicador);
    let mut app = loomcoding_tui::app::App::with_context(cwd, model)
        .with_ui_config(ui_config)
        .with_flow_modes(flow_modes);
    app.use_product_default_view();
    app.set_provider_settings(settings);
    app.set_declared_efforts(limits.efforts.clone());
    app.settings_api_key_configured = api_key_configured;
    if !api_key_configured {
        app.status = "API key missing · /settings".into();
    }
    app.set_prefs_path(data_dir.join("ui.toml"));
    app.set_agent_registry(registry);
    if matches!(
        a2a_config.role,
        loomcoding_a2a::config::A2aRole::Consume | loomcoding_a2a::config::A2aRole::Both
    ) {
        let refresh = frame_tx.clone();
        let local_refresh = local_refresher.clone();
        app.set_refresh_agents_hook(Arc::new(move || {
            let _ = refresh.try_send(ClientFrame::Command(ClientCommand::RefreshAgents));
            if let Some(local_refresh) = &local_refresh {
                let _ = local_refresh.send(());
            }
        }));
    }
    // Abrir `/model`, `/settings` o el selector de proveedor pide un sondeo; el envío no espera.
    app.set_refresh_providers_hook(Arc::new(move || {
        let _ = refresco_proveedores.send(());
    }));
    loomcoding_tui::run::run(
        app,
        prompt_tx,
        wake_tx,
        compact_tx,
        settings_tx,
        bus_rx,
        perm_rx,
        ask_rx,
        active_turn,
        session_command_tx,
        session_update_rx,
        catalog_rx,
    )
    .await?;
    let _ = frame_tx.send(ClientFrame::Disconnect).await;
    drop(frame_tx);
    let _ = reader_task.await;
    let _ = writer_task.await;
    let _ = agent_id;
    Ok(())
}
