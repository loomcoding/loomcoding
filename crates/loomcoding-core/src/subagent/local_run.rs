//! Lanzar un hijo local: la secuencia de arranque, paso a paso.
//!
//! Cabe señalar que `arrancar_con` solo orquesta; cada paso tiene su método con nombre. El orden
//! importa y se conserva: primero se valida todo (un rechazo no deja rastro), después se reserva
//! puesto y se crea la tarea, y solo al final se lanza el trabajo y se abre su compuerta.

use super::local_body::{cuerpo_local, EjecucionLocal};
use super::local_watch::{reenviar_eventos_del_hijo, vigilar_asiento};
use super::*;

/// Lo que el perfil (o su ausencia) decide sobre el hijo, ya validado.
pub(super) struct PerfilResuelto {
    pub(super) perfil: Option<AgentEntry>,
    pub(super) skills_elegidas: Option<Vec<String>>,
    pub(super) cliente_remoto: Option<Arc<dyn RemoteTaskClient>>,
    pub(super) profile_provider: Option<(Arc<dyn Provider>, crate::config::ProviderConfig)>,
    pub(super) agente: Option<String>,
    pub(super) origen: Option<String>,
}

/// Configuración, prompt y skills con que arranca el hijo.
pub(super) struct ConfigDelHijo {
    pub(super) config: LoomcodingConfig,
    pub(super) system: Option<String>,
    pub(super) skill_statuses: Vec<crate::skills::SkillLoadStatus>,
    pub(super) skill_usage: Vec<crate::session::UsageComponent>,
}

/// Servidores MCP que el perfil permite; sin perfil local, ninguno.
fn alcance_mcp(perfil: &Option<AgentEntry>) -> &[String] {
    perfil
        .as_ref()
        .and_then(|entry| match &entry.execution {
            Execution::Local(spec) => Some(spec.scope.mcp.as_slice()),
            Execution::Remote(_) => None,
        })
        .unwrap_or_default()
}

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
        let perfil = self.resolver_perfil(agent.as_deref(), skills, read_only)?;
        let project = self.cwd.to_string_lossy().into_owned();
        let permit = match self.active.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                // La validación ya pasó: la capacidad agotada sí deja constancia como tarea `rejected`.
                self.registrar_rechazo(
                    parent,
                    &label,
                    &project,
                    perfil.agente.clone(),
                    perfil.origen.clone(),
                )
                .await?;
                return Err(CoreError::Tool(mensaje_capacidad(self.max_active)));
            }
        };
        let child = self.store.create_session(&project).await?;
        let tracker = Arc::new(TaskTracker::new(self.store.clone(), self.bus.clone()));
        let scope = alcance_mcp(&perfil.perfil);
        self.avisar_mcp_ausente(scope, perfil.agente.as_deref());
        let (child_left, delegates) = self.profundidad_del_hijo(&perfil, read_only);
        let child_tools = self.herramientas_del_hijo(scope, delegates, read_only);
        let child_provider = perfil
            .profile_provider
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
        let ConfigDelHijo {
            config: child_config,
            system: child_system,
            skill_statuses,
            skill_usage: skill_usage_components,
        } = self.configuracion_del_hijo(&perfil);
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
        let descriptor_label = label.clone();
        // El bus del hijo se crea y se suscribe antes de lanzarlo, para no perder ningún
        // evento de su arranque. Su canal se cierra cuando se sueltan todos sus emisores.
        let child_bus = EventBus::new(256);
        let child_rx = child_bus.subscribe();
        if !skill_statuses.is_empty() {
            child_bus.publish(LiveEvent::SkillsLoaded {
                skills: skill_statuses,
            });
        }
        // La compuerta retiene al hijo hasta que su tarea `submitted` está guardada.
        let (abrir_puerta, puerta) = tokio::sync::oneshot::channel::<()>();
        let run: JobBody = match perfil.cliente_remoto {
            Some(cliente) => cuerpo_remoto(EjecucionRemota {
                cliente,
                parent,
                child,
                prompt,
                tracker: tracker.clone(),
                store: self.store.clone(),
                permit,
                puerta,
                parent_cancel,
                poll: self.remote_poll,
                max_follow: self.remote_max_follow,
                broker: self.broker.clone(),
                bus: self.bus.clone(),
                label: label.clone(),
                input_wait: self.remote_input_wait,
            }),
            None => cuerpo_local(EjecucionLocal {
                permit,
                broker: self.broker.clone(),
                parent,
                child,
                puerta,
                tracker: tracker.clone(),
                prompt,
                parent_cancel,
                provider: child_provider,
                store: self.store.clone(),
                tools: child_tools,
                bus: child_bus,
                permission: child_permission,
                config: child_config,
                cwd: self.cwd.clone(),
                jobs: self.jobs.clone(),
                catalog: self.catalog.clone(),
                system: child_system,
                skill_usage: skill_usage_components,
                budget: self.budget.clone(),
                family: child_family,
                spill: self.spill.clone(),
                registry: child_registry,
                confirm: self.confirm,
                delegation: self.delegation,
                nested: child_nested,
            }),
        };
        let job = self.jobs.start(JobSpec {
            kind: "subagent".into(),
            label,
            owner: parent,
            output_limit: 64 * 1024,
            run,
        })?;
        reenviar_eventos_del_hijo(self.bus.clone(), child_rx, parent, child);
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
            agent: perfil.agente,
            agent_origin: perfil.origen,
            reference_task_ids: referencias,
        };
        if let Err(error) = self.store.save_subagent(&descriptor).await {
            let _ = self.jobs.kill(parent, job.id());
            return Err(error);
        }
        let updates = self.jobs.subscribe(parent, job.id())?;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        let _ = abrir_puerta.send(());
        vigilar_asiento(
            self.store.clone(),
            self.bus.clone(),
            tracker,
            updates,
            parent,
            child,
        );
        Ok((child, job.id().clone()))
    }

    /// Valida y resuelve todo lo que el perfil decide: agente, skills elegidas, cliente remoto y
    /// proveedor propio. No reserva nada: un fallo aquí no deja rastro.
    fn resolver_perfil(
        &self,
        agent: Option<&str>,
        skills: Option<Vec<String>>,
        read_only: bool,
    ) -> Result<PerfilResuelto> {
        let perfil = self.resolver_agente(agent)?;
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
        Ok(PerfilResuelto {
            perfil,
            skills_elegidas,
            cliente_remoto,
            profile_provider,
            agente,
            origen,
        })
    }

    /// Avisa de los servidores MCP que el perfil permite pero no están configurados o conectados.
    fn avisar_mcp_ausente(&self, scope: &[String], agente: Option<&str>) {
        for server_id in scope {
            if !self.tools.has_mcp_server(server_id) {
                self.bus.publish(crate::event::LiveEvent::RuntimeNotice {
                    message: format!(
                        "Agent `{}` allows MCP server `{server_id}`, but it is not configured or connected",
                        agente.unwrap_or(AGENTE_POR_DEFECTO)
                    ),
                });
            }
        }
    }

    /// Niveles restantes del hijo y si puede delegar (véase `child_levels`). Un agente remoto o de
    /// revisión de solo lectura nunca delega. Avisa una sola vez por nombre si el techo recorta lo
    /// que pide el perfil.
    fn profundidad_del_hijo(&self, perfil: &PerfilResuelto, read_only: bool) -> (u32, bool) {
        let profile_depth = match &perfil.perfil {
            Some(AgentEntry {
                execution: Execution::Local(spec),
                ..
            }) => spec.max_depth,
            _ => None,
        };
        let child_left = child_levels(self.levels_left, profile_depth);
        let delegates = !read_only && can_delegate(child_left);
        if depth_clipped(self.levels_left, profile_depth) {
            let nombre = perfil
                .agente
                .clone()
                .unwrap_or_else(|| AGENTE_POR_DEFECTO.into());
            if self.clipped_notified.lock().unwrap().insert(nombre.clone()) {
                self.bus.publish(crate::event::LiveEvent::RuntimeNotice {
                    message: format!(
                        "Agent `{nombre}` asks for max_depth {} but the depth ceiling leaves it {child_left}",
                        profile_depth.unwrap_or_default()
                    ),
                });
            }
        }
        (child_left, delegates)
    }

    /// Herramientas del hijo: las del padre acotadas a su alcance MCP, más las de delegación si puede
    /// delegar; un revisor de solo lectura solo ve `view`, `glob` y `grep`.
    fn herramientas_del_hijo(
        &self,
        scope: &[String],
        delegates: bool,
        read_only: bool,
    ) -> Arc<ToolRegistry> {
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
        Arc::new(child_tools)
    }

    /// Configuración del hijo: la del padre, con lo que declare el perfil. Lo que el perfil
    /// no declara se hereda. Se copia aquí, al arrancar, y la tarea ya no depende del registro.
    fn configuracion_del_hijo(&self, perfil: &PerfilResuelto) -> ConfigDelHijo {
        let mut child_config = self.config.read().unwrap().clone();
        if let Some((_, provider)) = &perfil.profile_provider {
            child_config.provider = provider.clone();
        }
        let mut child_system = None;
        let mut skill_statuses = Vec::new();
        let mut skill_usage_components = Vec::new();
        if let Some(AgentEntry {
            execution: Execution::Local(spec),
            origin,
            ..
        }) = &perfil.perfil
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
            let ids = perfil
                .skills_elegidas
                .as_deref()
                .unwrap_or(&spec.scope.skills);
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
        ConfigDelHijo {
            config: child_config,
            system: child_system,
            skill_statuses,
            skill_usage: skill_usage_components,
        }
    }
}
