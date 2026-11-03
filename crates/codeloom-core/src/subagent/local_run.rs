//! Lanzar y ejecutar un hijo local: arranque, bucle del agente y asiento del resultado.

use super::*;

impl InProcessSubagentLauncher {
    /// Arranque común de `start` y `start_follow_up`. La petición ya viene validada.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn arrancar(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        referencias: Vec<SessionId>,
        read_only: bool,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        self.arrancar_con(
            parent,
            label,
            prompt,
            agent,
            referencias,
            read_only,
            None,
            parent_cancel,
        )
        .await
    }

    /// Como `arrancar`, con una selección opcional de las skills del perfil.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn arrancar_con(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        referencias: Vec<SessionId>,
        read_only: bool,
        skills: Option<Vec<String>>,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        // El agente se valida antes de reservar puesto o crear tarea: un rechazo no deja rastro.
        let perfil = self.resolver_agente(agent.as_deref())?;
        let skills_elegidas = elegir_skills(perfil.as_ref(), skills)?;
        if read_only
            && perfil
                .as_ref()
                .is_some_and(|entry| !matches!(entry.execution, Execution::Local(_)))
        {
            return Err(CoreError::Tool(
                "workflow reviewer must be a local agent".into(),
            ));
        }
        let cliente_remoto = self.cliente_remoto(perfil.as_ref())?;
        let profile_provider = match &perfil {
            Some(AgentEntry {
                execution: Execution::Local(spec),
                ..
            }) => match &spec.provider {
                Some(config) => {
                    let resolver = self.profile_provider_resolver.as_ref().ok_or_else(|| {
                        CoreError::Tool("no hay proveedor configurado para el perfil".into())
                    })?;
                    Some((resolver(config)?, config.clone()))
                }
                None => None,
            },
            _ => None,
        };
        let (agente, origen) = match &perfil {
            Some(entrada) => (Some(entrada.name.clone()), Some(entrada.origin.as_text())),
            None => (None, None),
        };
        let project = self.cwd.to_string_lossy().into_owned();
        let permit = match self.active.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                // La validación ya pasó: la capacidad agotada sí deja constancia como tarea `rejected`.
                self.registrar_rechazo(parent, &label, &project, agente.clone(), origen.clone())
                    .await?;
                return Err(CoreError::Tool(mensaje_capacidad(self.max_active)));
            }
        };
        let child = self.store.create_session(&project).await?;
        let tracker = Arc::new(TaskTracker::new(self.store.clone(), self.bus.clone()));
        let child_store = self.store.clone();
        let scope = perfil
            .as_ref()
            .and_then(|entry| match &entry.execution {
                Execution::Local(spec) => Some(spec.scope.mcp.as_slice()),
                Execution::Remote(_) => None,
            })
            .unwrap_or_default();
        for server_id in scope {
            if !self.tools.has_mcp_server(server_id) {
                self.bus.publish(crate::event::LiveEvent::RuntimeNotice {
                    message: format!(
                        "Agent `{}` allows MCP server `{server_id}`, but it is not configured or connected",
                        agente.as_deref().unwrap_or(AGENTE_POR_DEFECTO)
                    ),
                });
            }
        }
        // Niveles restantes del hijo y si puede delegar (véase `child_levels`). Un agente remoto o de
        // revisión de solo lectura nunca delega.
        let profile_depth = match &perfil {
            Some(AgentEntry {
                execution: Execution::Local(spec),
                ..
            }) => spec.max_depth,
            _ => None,
        };
        let child_left = child_levels(self.levels_left, profile_depth);
        let delegates = !read_only && can_delegate(child_left);
        if depth_clipped(self.levels_left, profile_depth) {
            let nombre = agente.clone().unwrap_or_else(|| AGENTE_POR_DEFECTO.into());
            if self.clipped_notified.lock().unwrap().insert(nombre.clone()) {
                self.bus.publish(crate::event::LiveEvent::RuntimeNotice {
                    message: format!(
                        "Agent `{nombre}` asks for max_depth {} but the depth ceiling leaves it {child_left}",
                        profile_depth.unwrap_or_default()
                    ),
                });
            }
        }
        let mut scoped_tools = self.tools.with_mcp_scope(scope);
        if delegates {
            for tool in &self.delegation_tools {
                scoped_tools.register(tool.clone());
            }
        }
        let mut child_tools = if read_only {
            scoped_tools.only(&["view", "glob", "grep"])
        } else {
            scoped_tools
        };
        // Solo los hijos pueden dejar avances; la sesión raíz no tiene tarea a la que adjuntarlos.
        child_tools.register(Arc::new(
            crate::tools::artifact_publish::ArtifactPublishTool,
        ));
        let child_tools = Arc::new(child_tools);
        let child_provider = profile_provider
            .as_ref()
            .map(|(provider, _)| provider.clone())
            .unwrap_or_else(|| self.provider.read().unwrap().clone());
        // Cada hijo tiene su propio servicio de permisos: delega en el del padre y
        // encamina sus preguntas (`ask_user`) al buzón de este lanzador.
        let child_permission = Arc::new(PermissionService::with_responder(Box::new(
            ChildResponder::new(
                self.permission.clone(),
                self.broker.clone(),
                parent,
                child,
                label.clone(),
                self.bus.clone(),
                tracker.clone(),
            ),
        )));
        let child_broker = self.broker.clone();
        // Configuración del hijo: la del padre, con lo que declare el perfil. Lo que el perfil
        // no declara se hereda. Se copia aquí, al arrancar, y la tarea ya no depende del registro.
        let mut child_config = self.config.read().unwrap().clone();
        if let Some((_, provider)) = &profile_provider {
            child_config.provider = provider.clone();
        }
        let mut child_system = None;
        let mut skill_statuses = Vec::new();
        let mut skill_usage_components = Vec::new();
        if let Some(AgentEntry {
            execution: Execution::Local(spec),
            origin,
            ..
        }) = &perfil
        {
            if let Some(modelo) = &spec.model {
                child_config.model = modelo.clone();
            }
            if let Some(esfuerzo) = &spec.effort {
                child_config.effort = Some(esfuerzo.clone());
            }
            // `max_turns` limita los pasos del bucle del hijo: es el límite que ya lleva `max_steps`.
            if let Some(turnos) = spec.max_turns {
                child_config.max_steps = usize::try_from(turnos).unwrap_or(usize::MAX);
            }
            // `max_depth` se aplicará al construir el hijo (véase `child_levels`).
            let sistema = spec.system_prompt();
            if !sistema.trim().is_empty() {
                child_system = Some(sistema);
            }
            let home = std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from);
            let mut raices = crate::skills::SkillRoots::new(self.cwd.clone(), home);
            // Las skills de un catálogo externo solo las ve el agente que viene de ese catálogo.
            if let AgentOrigin::File {
                level: crate::registry::ProfileLevel::Catalog,
                path,
            } = origin
            {
                if let Some(raiz) = crate::registry::catalog_root(path) {
                    raices = raices.with_catalog(raiz.join(".agents").join("skills"));
                }
            }
            let ids = skills_elegidas.as_deref().unwrap_or(&spec.scope.skills);
            let loaded = crate::skills::load_scoped_skills(ids, &raices);
            skill_statuses = loaded.statuses;
            skill_usage_components = skill_statuses
                .iter()
                .filter(|skill| skill.state == crate::skills::SkillLoadState::Loaded)
                .map(|skill| crate::session::UsageComponent {
                    kind: crate::session::UsageComponentKind::Skill,
                    id: skill.id.clone(),
                    input_tokens: skill.input_tokens_per_request,
                })
                .collect();
            if loaded.shared_input_tokens_per_request > 0 {
                skill_usage_components.push(crate::session::UsageComponent {
                    kind: crate::session::UsageComponentKind::Skill,
                    id: "local-skills-preamble".into(),
                    input_tokens: loaded.shared_input_tokens_per_request,
                });
            }
            child_system = compose_child_system(child_system.take(), &loaded.prompt);
        }
        // El hijo que puede delegar recibe un lanzador anidado que comparte con este el tope de activos, el
        // buzón y el presupuesto de toda la familia; los demás no reciben ninguno.
        let child_family = self.familia(parent);
        let child_nested = delegates.then(|| -> Arc<dyn SubagentLauncher> {
            Arc::new(self.anidado(
                child_family,
                child_provider.clone(),
                child_config.clone(),
                child_permission.clone(),
                &child_tools,
                child_left,
            ))
        });
        let child_registry = if delegates {
            self.registry.clone()
        } else {
            None
        };
        let child_confirm = self.confirm;
        let child_delegation = self.delegation;
        let child_max_steps = child_config.max_steps;
        let child_cwd = self.cwd.clone();
        let child_jobs = self.jobs.clone();
        let child_catalog = self.catalog.clone();
        let child_spill = self.spill.clone();
        let child_budget = self.budget.clone();
        let descriptor_label = label.clone();
        // El bus del hijo se crea y se suscribe antes de lanzarlo, para no perder ningún
        // evento de su arranque. Su canal se cierra cuando se sueltan todos sus emisores.
        let child_bus = EventBus::new(256);
        let mut child_rx = child_bus.subscribe();
        if !skill_statuses.is_empty() {
            child_bus.publish(LiveEvent::SkillsLoaded {
                skills: skill_statuses,
            });
        }
        // La compuerta retiene al hijo hasta que su tarea `submitted` está guardada.
        let (abrir_puerta, puerta) = tokio::sync::oneshot::channel::<()>();
        let cuerpo_tracker = tracker.clone();
        let run: JobBody = match cliente_remoto {
            Some(cliente) => cuerpo_remoto(EjecucionRemota {
                cliente,
                parent,
                child,
                prompt,
                tracker: cuerpo_tracker,
                store: self.store.clone(),
                permit,
                puerta,
                parent_cancel,
                poll: self.remote_poll,
                max_follow: self.remote_max_follow,
                broker: child_broker.clone(),
                bus: self.bus.clone(),
                label: label.clone(),
                input_wait: self.remote_input_wait,
            }),
            None => Box::new(move |handle, job_cancel| {
                Box::pin(async move {
                    let _permit = permit;
                    let _discard = DiscardOnExit {
                        broker: child_broker,
                        child,
                    };
                    // Si el lanzador no llega a abrir la compuerta, el hijo no arranca.
                    if puerta.await.is_err() || job_cancel.is_cancelled() {
                        return Ok(None);
                    }
                    if let Err(error) = cuerpo_tracker
                        .transition(parent, child, TaskStatus::new(TaskState::Working))
                        .await
                    {
                        tracing::warn!("persist subagent status: {error}");
                    }
                    let mut child_agent = AgentLoop::new(
                        child_provider,
                        child_store.clone(),
                        child_tools,
                        child_bus,
                        child_permission,
                        child_config,
                        child_cwd,
                    )
                    .with_jobs(child_jobs)
                    .with_catalog(child_catalog)
                    .with_system_extra(child_system)
                    .with_system_usage_components(skill_usage_components)
                    .with_budget(child_budget)
                    .with_budget_family(child_family);
                    if let Some(spill) = child_spill {
                        child_agent = child_agent.with_spill(spill);
                    }
                    if let Some(registro) = child_registry {
                        child_agent = child_agent
                            .with_agent_registry(registro)
                            .with_delegation_confirm(child_confirm);
                    }
                    // Al final: los demás `with_*` reconstruyen el lanzador por defecto del agente.
                    if let Some(lanzador) = child_nested {
                        child_agent = child_agent
                            .with_delegation_mode(child_delegation)
                            .with_subagent_launcher(lanzador);
                    }
                    let child_cancel = CancellationToken::new();
                    let outcome = tokio::select! {
                        result = child_agent.run_internal(child, prompt, child_cancel.clone()) => result?,
                        _ = job_cancel.cancelled() => {
                            child_cancel.cancel();
                            return Ok(None);
                        }
                        _ = parent_cancel.cancelled() => {
                            child_cancel.cancel();
                            // Cancelar al padre cancela al hijo: el trabajo se mata para que la
                            // tarea acabe en `canceled` y no en `completed` sin resultado.
                            job_cancel.cancel();
                            return Ok(None);
                        }
                    };
                    // El agente ya no emite: soltarlo cierra el bus del hijo y permite
                    // que la tarea de reenvío termine.
                    drop(child_agent);
                    let steps = outcome.steps;
                    let text = outcome.final_text;
                    if text.trim().is_empty() {
                        // El motivo importa: antes todo se reportaba como "sin respuesta
                        // final tras N pasos", que sonaba a presupuesto agotado incluso
                        // cuando el corte había sido por repetición de tool calls.
                        let reason = if steps >= child_max_steps {
                            format!(
                                "subagent reached its step limit ({steps}/{child_max_steps}) without a final response"
                            )
                        } else {
                            format!(
                                "subagent produced no final response after {steps} steps \
                                 (the turn was cut early: repeated tool calls or cancellation)"
                            )
                        };
                        return Err(CoreError::Tool(reason));
                    }
                    child_store
                        .complete_subagent(parent, child, &[Artifact::text_artifact(text.as_str())])
                        .await?;
                    if let Err(error) = cuerpo_tracker.publish(parent, child).await {
                        tracing::warn!("publish subagent status: {error}");
                    }
                    handle.append(&text);
                    Ok(Some(JobResult { content: text }))
                })
            }),
        };
        let job = self.jobs.start(JobSpec {
            kind: "subagent".into(),
            label,
            owner: parent,
            output_limit: 64 * 1024,
            run,
        })?;
        // Reenvío filtrado: solo los eventos relevantes del hijo llegan al padre,
        // etiquetados con su identidad. La tarea termina cuando el bus del hijo se cierra.
        let forward_bus = self.bus.clone();
        tokio::spawn(async move {
            loop {
                match child_rx.recv().await {
                    Ok(event) => {
                        // El avance que publica el propio hijo cambia su descriptor: sube tal cual, sin
                        // envolver, como el resto de cambios de tarea que publica el lanzador.
                        if matches!(&event, LiveEvent::SubagentChanged(d) if d.parent == parent && d.child == child)
                        {
                            forward_bus.publish(event);
                        } else if event.is_child_relevant() {
                            forward_bus.publish(LiveEvent::ChildEvent {
                                parent,
                                child,
                                event: Box::new(event),
                            });
                        }
                    }
                    // Un hijo muy verboso puede adelantar el búfer: se pierden eventos, no el reenvío.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        // La tarea nace `submitted` al reconocerla; el hijo empieza cuando queda guardada.
        let descriptor = SubagentDescriptor {
            parent,
            child,
            job_id: job.id().0.clone(),
            label: descriptor_label,
            status: TaskStatus::new(TaskState::Submitted),
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: agente,
            agent_origin: origen,
            reference_task_ids: referencias,
        };
        if let Err(error) = self.store.save_subagent(&descriptor).await {
            let _ = self.jobs.kill(parent, job.id());
            return Err(error);
        }
        let mut updates = self.jobs.subscribe(parent, job.id())?;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        let _ = abrir_puerta.send(());
        let store = self.store.clone();
        let settle_bus = self.bus.clone();
        tokio::spawn(async move {
            loop {
                let snapshot = updates.borrow_and_update().clone();
                let state = TaskState::from(snapshot.status);
                if state.is_terminal() {
                    // `complete_subagent` ya fijó `Completed` junto con el resultado: aquí solo se
                    // escribe lo que el almacén aún no tiene asentado (muerte, fallo o cierre sin resultado).
                    let asentado = match store.list_subagents(parent).await {
                        Ok(hijos) => hijos
                            .iter()
                            .find(|hijo| hijo.child == child && hijo.status.state.is_terminal())
                            .map(|hijo| hijo.status.clone()),
                        Err(error) => {
                            tracing::warn!("read subagent status: {error}");
                            None
                        }
                    };
                    // Manda el estado ya persistido: un espejo remoto fija el suyo (p. ej. `rejected`)
                    // antes de que el trabajo acabe, y el aviso no debe contradecirlo.
                    let estado_final = match asentado {
                        Some(persistido) => persistido.state,
                        None => {
                            let mut status = TaskStatus::new(state);
                            if state == TaskState::Failed {
                                if let Some(detail) = snapshot.detail.as_deref() {
                                    status.message = Some(Message::agent_text(
                                        crate::tools::limit_bytes(
                                            detail,
                                            crate::tools::MAX_NOTICE_BYTES,
                                        )
                                        .into_owned(),
                                    ));
                                }
                            }
                            if let Err(error) = tracker.transition(parent, child, status).await {
                                tracing::warn!("persist subagent status: {error}");
                            }
                            state
                        }
                    };
                    // Se publica tras el intento de persistir, también si falló: así el
                    // padre nunca lee el aviso antes del estado final.
                    settle_bus.publish(LiveEvent::SubagentSettled {
                        parent,
                        child,
                        status: estado_final,
                    });
                    break;
                }
                if updates.changed().await.is_err() {
                    break;
                }
            }
        });
        Ok((child, job.id().clone()))
    }
}
