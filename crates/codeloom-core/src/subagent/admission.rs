//! Antes de lanzar un hijo: resolver el agente, pedir aprobación y registrar rechazos por capacidad.

use super::*;

impl InProcessSubagentLauncher {
    /// Guarda un hijo rechazado por capacidad agotada: tarea `rejected` sin trabajo.
    pub(super) async fn registrar_rechazo(
        &self,
        parent: SessionId,
        label: &str,
        project: &str,
        agent: Option<String>,
        agent_origin: Option<String>,
    ) -> Result<()> {
        let child = self.store.create_session(project).await?;
        let descriptor = SubagentDescriptor {
            parent,
            child,
            job_id: String::new(),
            label: label.to_string(),
            status: TaskStatus::new(TaskState::Rejected)
                .with_message(mensaje_capacidad(self.max_active)),
            phase: SubagentPhase::Rejected,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            artifacts: Vec::new(),
            notice_delivered: false,
            agent,
            agent_origin,
            reference_task_ids: Vec::new(),
        };
        self.store.save_subagent(&descriptor).await?;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        Ok(())
    }

    /// Resuelve el agente pedido (`None` = `default`) en la instantánea vigente y devuelve su
    /// entrada copiada por valor: una recarga posterior no la altera. Sin registro, solo
    /// `default` es válido y se devuelve `None` (sin perfil que aplicar). Todo rechazo es un
    /// error de validación: quien llama no ha creado aún tarea ni puesto.
    pub(super) fn resolver_agente(&self, agent: Option<&str>) -> Result<Option<AgentEntry>> {
        let nombre = agent.unwrap_or(AGENTE_POR_DEFECTO);
        let Some(registro) = &self.registry else {
            if nombre == AGENTE_POR_DEFECTO {
                return Ok(None);
            }
            return Err(CoreError::Tool(format!(
                "no hay registro de agentes configurado: no se puede usar el agente «{nombre}»"
            )));
        };
        let instantanea = registro.snapshot();
        let entrada = instantanea.get(nombre).ok_or_else(|| {
            CoreError::Tool(format!("agente «{nombre}» inexistente en el registro"))
        })?;
        // Sin fábrica de clientes no hay forma de ejecutar un remoto: se dice antes que lo demás.
        if matches!(entrada.execution, Execution::Remote(_)) && self.remote_clients.is_none() {
            return Err(CoreError::Tool(format!(
                "agente remoto no disponible: «{nombre}»"
            )));
        }
        if instantanea.approval(nombre) == Some(Approval::Pending) {
            return Err(CoreError::Tool(format!(
                "agente «{nombre}» pendiente de aprobación: apruébalo antes de usarlo"
            )));
        }
        if let (Execution::Remote(spec), Some(fabrica)) = (&entrada.execution, &self.remote_clients)
        {
            if let Some(motivo) = fabrica.unavailable_reason(nombre, spec) {
                return Err(CoreError::Tool(format!(
                    "el agente remoto «{nombre}» no se puede usar: {motivo}"
                )));
            }
        }
        if let Execution::Local(spec) = &entrada.execution {
            if let Some(modelo) = &spec.model {
                let base_url = spec
                    .provider
                    .as_ref()
                    .map(|provider| provider.base_url.clone())
                    .unwrap_or_else(|| self.config.read().unwrap().provider.base_url.clone());
                let conocido = self.catalog.read().unwrap().knows_model(&base_url, modelo);
                if conocido == Some(false) {
                    return Err(CoreError::Tool(format!(
                        "el modelo «{modelo}» del agente «{nombre}» no figura en el catálogo"
                    )));
                }
            }
        }
        Ok(Some(entrada.clone()))
    }

    /// Cliente del agente remoto elegido; `None` si el agente es local. Un remoto sin cliente
    /// construible (p. ej. sin tarjeta descargada) es un error: no se crea tarea.
    pub(super) fn cliente_remoto(
        &self,
        perfil: Option<&AgentEntry>,
    ) -> Result<Option<Arc<dyn RemoteTaskClient>>> {
        let Some(AgentEntry {
            name,
            execution: Execution::Remote(spec),
            ..
        }) = perfil
        else {
            return Ok(None);
        };
        self.remote_clients
            .as_ref()
            .and_then(|fabrica| fabrica.client_for(name, spec))
            .map(Some)
            .ok_or_else(|| CoreError::Tool(format!("agente remoto no disponible: «{name}»")))
    }

    /// Con `confirm_delegation = always`, pregunta a la persona usuaria antes de crear el hijo y solo
    /// continúa si responde exactamente «Approve». Sin nadie que responda (sin usuario interactivo,
    /// `--yolo`, pregunta descartada) se deniega: nada se aprueba por omisión. Es el control por código
    /// de esa regla, que antes solo existía como frase del prompt.
    pub(super) async fn pedir_aprobacion(
        &self,
        label: &str,
        prompt: &str,
        agent: Option<&str>,
    ) -> Result<()> {
        if self.confirm != ConfirmDelegation::Always {
            return Ok(());
        }
        let extracto = crate::tools::limit_bytes(prompt, 600);
        let pregunta = crate::question::Question {
            header: "Delegation".into(),
            question: format!(
                "Start a subagent?
Agent: {}
Task: {label}

{extracto}",
                agent.unwrap_or("default")
            ),
            options: vec![
                crate::question::QuestionOption {
                    label: "Approve".into(),
                    description: String::new(),
                },
                crate::question::QuestionOption {
                    label: "Cancel".into(),
                    description: String::new(),
                },
            ],
            multi_select: false,
        };
        let aprobada = self
            .permission
            .ask_user(&[pregunta])
            .await
            .and_then(|respuestas| respuestas.into_iter().next())
            .is_some_and(|respuesta| respuesta.selected == ["Approve"]);
        if aprobada {
            Ok(())
        } else {
            Err(CoreError::Tool(
                "delegation was not approved by the user".into(),
            ))
        }
    }
}

/// Mensaje de la tarea rechazada por capacidad agotada. Es el mismo error que recibe el padre.
pub(super) fn mensaje_capacidad(maximo: usize) -> String {
    format!("maximum of {maximo} active subagents reached")
}

/// Resumen de una petición de permiso para el mensaje de `auth-required`: herramienta, acción
/// y detalle recortado, de modo que el padre vea qué se pide sin volcar argumentos enteros.
pub(crate) fn resumen_de_permiso(tool: &str, action: &str, detail: &str) -> String {
    const LIMITE: usize = 160;
    let detalle = detail.trim();
    let resumen = if detalle.is_empty() {
        format!("{tool} {action}")
    } else {
        format!("{tool} {action}: {detalle}")
    };
    if resumen.chars().count() <= LIMITE {
        resumen
    } else {
        let mut recortado: String = resumen.chars().take(LIMITE - 1).collect();
        recortado.push('…');
        recortado
    }
}

/// Validación común del prompt y la etiqueta de un hijo nuevo.
pub(super) fn validar_peticion(label: &str, prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() || prompt.len() > 32_768 || label.len() > 128 {
        return Err(CoreError::Tool("invalid subagent prompt or label".into()));
    }
    Ok(())
}
