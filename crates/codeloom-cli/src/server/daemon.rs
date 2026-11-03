//! Ciclo de vida del daemon: arranque, conexiones, autenticación y servicio de cada cliente.

use super::*;

pub async fn connect_or_start() -> anyhow::Result<ipc::Stream> {
    let token = ipc::load_or_create_token()?;
    let mut child_started = false;
    let mut last_error = None;
    for _ in 0..200 {
        if let Ok(mut stream) = ipc::connect().await {
            ipc::send(
                &mut stream,
                &ClientFrame::Hello {
                    version: ipc::PROTOCOL_VERSION,
                    token: token.clone(),
                },
            )
            .await?;
            match ipc::receive::<ServerFrame>(&mut stream).await? {
                ServerFrame::HelloAccepted { version } if version == ipc::PROTOCOL_VERSION => {
                    return Ok(stream)
                }
                ServerFrame::HelloAccepted { version } => {
                    anyhow::bail!("servidor CodeLoom incompatible (protocolo {version})")
                }
                ServerFrame::HelloRejected { message } => anyhow::bail!("{message}"),
                ServerFrame::Event(_) => {
                    anyhow::bail!("respuesta IPC inesperada durante el saludo")
                }
            }
        }
        if !child_started {
            start_server_process()?;
            child_started = true;
        }
        last_error = Some("el servidor local no está disponible".to_string());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    anyhow::bail!(last_error.unwrap_or_else(|| "no se pudo iniciar el servidor CodeLoom".into()))
}

pub(super) fn start_server_process() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.as_std_mut().pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command.spawn()?;
    Ok(())
}

pub async fn run_server() -> anyhow::Result<()> {
    let _token = ipc::load_or_create_token()?;
    let mut listener = ipc::Listener::bind()?;
    let data_dir = crate::home::user_home()
        .unwrap_or_else(std::env::temp_dir)
        .join(".codeloom");
    std::fs::create_dir_all(&data_dir)?;
    let store = Arc::new(
        codeloom_core::session::sqlite::SqliteStore::open(
            data_dir.join("sessions.db").to_str().unwrap(),
        )
        .await?,
    );
    let runtime_path = data_dir.join("runtime.toml");
    let runtime_text = if runtime_path.exists() {
        std::fs::read_to_string(&runtime_path)?
    } else {
        String::new()
    };
    let a2a = codeloom_a2a::config::A2aConfig::from_toml(&runtime_text)?;
    let supervisor = toml::from_str::<RuntimeUserConfig>(&runtime_text)
        .ok()
        .and_then(|config| config.supervisor)
        .and_then(|settings| {
            let config = codeloom_core::config::CodeLoomConfig {
                provider: settings.provider,
                model: settings.model,
                max_steps: 1,
                effort: None,
            };
            crate::build_provider(&config, &HashMap::new(), None)
                .ok()
                .map(|provider| (provider, config.model))
        });
    let supervisor = if let Some((provider, model)) = supervisor {
        store
            .create_session(".codeloom-supervisor")
            .await
            .ok()
            .map(|session| SupervisorAgent {
                provider,
                model,
                timeout: Duration::from_secs(15),
                session,
                store: store.clone(),
            })
    } else {
        None
    };
    if a2a.serves() {
        if let Some(auth) = &a2a.serve.auth {
            if crate::resolve_credential(&auth.env).is_none() {
                crate::credentials::get_or_create(&auth.env)?;
            }
        }
    }
    let runtime_a2a = resolve_runtime_a2a(&a2a, &crate::resolve_credential)?;
    let (authenticator, interface, auth_scheme) = match runtime_a2a {
        Some((authenticator, interface, scheme)) => {
            (Some(authenticator), Some(interface), Some(scheme))
        }
        None => (None, None, None),
    };
    let cards = Arc::new(StdRwLock::new(HashMap::new()));
    let runtime_generation = generate_agent_id()?;
    let manager = Arc::new(AgentManager::new(
        cards.clone(),
        interface.clone(),
        auth_scheme.clone(),
        runtime_generation,
        data_dir.clone(),
        a2a.serve.auth.as_ref().map(|auth| auth.env.clone()),
        a2a.serve.max_concurrent_tasks,
        Duration::from_secs(a2a.serve.max_task_secs),
        supervisor,
    ));
    if let (Some(auth), Some(interface), Some(scheme)) = (authenticator, interface, auth_scheme) {
        let supervisor = codeloom_core::card::AgentCard::local(
            "CodeLoom Runtime Supervisor",
            "Runtime status and deterministic supervision.",
            env!("CARGO_PKG_VERSION"),
        );
        let card = codeloom_a2a::serve::card::served_card_for_tenant(
            &supervisor,
            &interface,
            Some("supervisor"),
            &scheme,
        );
        cards
            .write()
            .unwrap()
            .insert("supervisor".into(), card.clone());
        let state = Arc::new(codeloom_a2a::serve::rpc::ServeState {
            host: manager.clone(),
            auth,
            card,
            expected_tenant: None,
            agent_cards: cards,
            max_wait: Duration::from_secs(a2a.serve.max_wait_secs),
        });
        codeloom_a2a::serve::serve(state, a2a.serve.listen_addr()?, CancellationToken::new())
            .await?;
        manager.keep_alive.store(true, Ordering::Release);
    }
    let clients = Arc::new(Semaphore::new(32));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let stream = accepted?;
                let permit = clients.clone().try_acquire_owned();
                if let Ok(permit) = permit {
                    tokio::spawn(serve_connection(stream, permit, manager.clone(), store.clone(), data_dir.clone(), _token.clone()));
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if manager.should_stop() { break; }
            }
        }
    }
    drop(store);
    Ok(())
}

pub(super) fn resolve_runtime_a2a(
    config: &codeloom_a2a::config::A2aConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> anyhow::Result<
    Option<(
        codeloom_a2a::serve::auth::Authenticator,
        String,
        codeloom_a2a::serve::config::ServeAuthScheme,
    )>,
> {
    if !matches!(
        config.role,
        codeloom_a2a::config::A2aRole::Serve | codeloom_a2a::config::A2aRole::Both
    ) {
        return Ok(None);
    }
    let resolved = config.serve.validate(env)?;
    let scheme = resolved.scheme.clone();
    Ok(Some((
        codeloom_a2a::serve::auth::Authenticator::new(resolved),
        config.serve.interface_url()?,
        scheme,
    )))
}

pub(super) async fn serve_connection(
    stream: ipc::Stream,
    _permit: tokio::sync::OwnedSemaphorePermit,
    manager: Arc<AgentManager>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    data_dir: PathBuf,
    token: String,
) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let hello = tokio::time::timeout(
        Duration::from_secs(5),
        ipc::receive::<ClientFrame>(&mut reader),
    )
    .await;
    let Ok(Ok(ClientFrame::Hello {
        version,
        token: supplied,
    })) = hello
    else {
        return;
    };
    if version != ipc::PROTOCOL_VERSION || !constant_time_eq(&token, &supplied) {
        let message = if version != ipc::PROTOCOL_VERSION {
            "protocolo local incompatible"
        } else {
            "autenticación local rechazada"
        };
        let _ = ipc::send(
            &mut writer,
            &ServerFrame::HelloRejected {
                message: message.into(),
            },
        )
        .await;
        return;
    }
    if ipc::send(&mut writer, &ServerFrame::HelloAccepted { version })
        .await
        .is_err()
    {
        return;
    }
    manager.client_connected();
    let result =
        serve_authenticated(&mut reader, &mut writer, manager.clone(), store, &data_dir).await;
    if let Some(agent_id) = result {
        disconnect_agent(&manager, &agent_id).await;
    }
    manager.client_disconnected();
}

pub(super) async fn disconnect_agent(manager: &AgentManager, id: &str) {
    if let Some(agent) = manager.get(id).await {
        agent.connected.store(false, Ordering::Release);
        agent.ui.disconnect().await;
    }
}

pub(super) fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != 64 || right.len() != 64 {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

pub(super) async fn serve_authenticated<R, W>(
    reader: &mut R,
    writer: &mut W,
    manager: Arc<AgentManager>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    data_dir: &std::path::Path,
) -> Option<String>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let frame =
        match tokio::time::timeout(Duration::from_secs(5), ipc::receive::<ClientFrame>(reader))
            .await
        {
            Ok(Ok(frame)) => frame,
            _ => return None,
        };
    match frame {
        ClientFrame::Print { cwd, prompt, yolo } => {
            let result = async {
                if prompt.len() > 32_000 {
                    anyhow::bail!("prompt exceeds 32000 bytes");
                }
                let cwd = validate_cwd(cwd)?;
                let ui = Arc::new(AgentUi::new(mpsc::channel(1).0));
                let setup =
                    build_agent(&manager, cwd, ui, store.clone(), data_dir, yolo, false).await?;
                let session = store.create_session(&setup.project).await?;
                let outcome = setup
                    .agent
                    .run(session, prompt, CancellationToken::new())
                    .await?;
                Ok::<String, anyhow::Error>(outcome.final_text)
            }
            .await;
            let response = result
                .map(|text| codeloom_core::tools::limit_bytes(&text, 32_000).into_owned())
                .map_err(|error| error.to_string());
            let _ = ipc::send(
                writer,
                &ServerFrame::event(ServerEvent::PrintResult { result: response }),
            )
            .await;
            None
        }
        ClientFrame::Open {
            cwd,
            attach_agent,
            yolo,
        } => {
            let event_capacity = mpsc::channel::<ServerFrame>(128);
            let (event_tx, mut event_rx) = event_capacity;
            let entry = match validate_cwd(cwd).map(|cwd| (cwd, attach_agent)) {
                Ok((cwd, attach)) => match open_agent(
                    &manager,
                    store,
                    data_dir,
                    cwd,
                    attach.as_deref(),
                    yolo,
                    event_tx,
                )
                .await
                {
                    Ok(entry) => entry,
                    Err(error) => {
                        let _ = ipc::send(
                            writer,
                            &ServerFrame::event(ServerEvent::Error {
                                message: error.to_string(),
                            }),
                        )
                        .await;
                        return None;
                    }
                },
                Err(error) => {
                    let _ = ipc::send(
                        writer,
                        &ServerFrame::event(ServerEvent::Error {
                            message: error.to_string(),
                        }),
                    )
                    .await;
                    return None;
                }
            };
            let ready = ServerFrame::event(ServerEvent::Ready {
                agent_id: entry.id.clone(),
                runtime_generation: manager.runtime_generation.clone(),
                cwd: entry.cwd.display().to_string(),
                model: entry.model.read().unwrap().clone(),
                settings: entry.settings.read().unwrap().clone(),
                api_key_configured: entry.api_key_configured.load(Ordering::Acquire),
            });
            if ipc::send(writer, &ready).await.is_err() {
                entry.connected.store(false, Ordering::Release);
                entry.ui.disconnect().await;
                return Some(entry.id.clone());
            }
            let capabilities = ServerFrame::event(ServerEvent::Capabilities(
                entry.runtime_capabilities.read().unwrap().clone(),
            ));
            if ipc::send(writer, &capabilities).await.is_err() {
                entry.connected.store(false, Ordering::Release);
                entry.ui.disconnect().await;
                return Some(entry.id.clone());
            }
            let session = SessionId(entry.current_session.load(Ordering::Acquire));
            if entry
                .commands
                .send(AgentCommand::Session(
                    codeloom_tui::run::SessionCommand::Select(
                        codeloom_tui::app::SessionChoice::Resume(session),
                    ),
                ))
                .await
                .is_err()
            {
                return Some(entry.id.clone());
            }
            loop {
                tokio::select! {
                    event = event_rx.recv() => match event {
                        Some(frame) => if ipc::send(writer, &frame).await.is_err() { break; },
                        None => break,
                    },
                    frame = ipc::receive::<ClientFrame>(reader) => match frame {
                        Ok(ClientFrame::Command(command)) => {
                            if handle_client_command(&entry, command).await.is_err() { break; }
                        }
                        Ok(ClientFrame::Disconnect) | Err(_) => break,
                        _ => break,
                    },
                }
            }
            entry.connected.store(false, Ordering::Release);
            entry.ui.disconnect().await;
            Some(entry.id.clone())
        }
        _ => {
            let _ = ipc::send(
                writer,
                &ServerFrame::HelloRejected {
                    message: "se esperaba Open o Print".into(),
                },
            )
            .await;
            None
        }
    }
}
