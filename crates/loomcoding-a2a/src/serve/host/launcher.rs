//! Operaciones del anfitrión sobre el lanzador: arranque, flujo, observación y vista.

use super::*;

impl LauncherHost {
    /// Crea la sesión padre del servidor (una por proceso) y devuelve el anfitrión.
    pub async fn new(
        launcher: Arc<dyn SubagentLauncher>,
        store: &dyn SessionStore,
        cwd: &str,
        agent: String,
        limits: HostLimits,
    ) -> loomcoding_core::error::Result<Self> {
        let parent = store.create_session(cwd).await?;
        Ok(LauncherHost {
            launcher,
            parent,
            agent,
            limits,
            bus: None,
            flujos: Arc::new(tokio::sync::Semaphore::new(MAX_STREAMS)),
            arranque: tokio::sync::Mutex::new(()),
        })
    }

    /// Publica en el bus cada cambio de estado de una tarea entrante, para que la persona usuaria
    /// la vea en la conversación.
    pub fn with_bus(mut self, bus: EventBus) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Sesión padre bajo la que viven las tareas entrantes.
    pub fn parent(&self) -> SessionId {
        self.parent
    }

    pub(super) async fn descriptor(&self, id: &str) -> Result<SubagentDescriptor, HostError> {
        // Un id que no es un número es, para el cliente, una tarea que no existe.
        let hijo = id.parse::<i64>().map_err(|_| HostError::NotFound)?;
        self.launcher
            .list(self.parent)
            .await
            .map_err(otro)?
            .into_iter()
            .find(|descriptor| descriptor.child.0 == hijo)
            .ok_or(HostError::NotFound)
    }

    /// Flujo de la tarea `hijo` ligado a un hueco de flujo: al soltarlo, el hueco se libera.
    /// Se suscribe al bus **antes** de leer la tarea, para no perder cambios entre ambas cosas.
    pub(super) async fn flujo(
        &self,
        hijo: i64,
        permiso: tokio::sync::OwnedSemaphorePermit,
        permitir_terminal: bool,
    ) -> Result<EventStream, HostError> {
        let bus = self.bus.as_ref().ok_or(HostError::Unsupported)?;
        let rx = bus.subscribe();
        let actual = self.descriptor(&hijo.to_string()).await?;
        if !permitir_terminal && actual.status.state.is_terminal() {
            return Err(HostError::Unsupported);
        }
        let (lanzador, padre, id) = (self.launcher.clone(), self.parent, actual.child);
        let refrescar: Refrescar = Box::new(move || {
            let lanzador = lanzador.clone();
            Box::pin(async move {
                lanzador
                    .list(padre)
                    .await
                    .ok()?
                    .into_iter()
                    .find(|descriptor| descriptor.child == id)
            })
        });
        let maximo = self.limits.max_result_bytes;
        let (bus, agent) = (self.bus.clone(), self.agent.clone());
        Ok(flujo_de_tarea(rx, actual, refrescar)
            .map(move |evento| {
                // El hueco viaja dentro del cierre: se libera cuando el flujo se suelta.
                let _permiso = &permiso;
                if let Some(bus) = &bus {
                    let observacion = match &evento {
                        loomcoding_core::task_events::StreamResponse::StatusUpdate(evento) => {
                            Some(A2aObservation {
                                direction: A2aDirection::Outbound,
                                agent: agent.clone(),
                                operation: A2aOperation::TaskStatusUpdateEvent,
                                rpc_id: None,
                                task_id: Some(evento.task_id.clone()),
                                context_id: Some(evento.context_id.clone()),
                                state: Some(evento.status.state.proto_name().to_string()),
                                artifact: None,
                                succeeded: true,
                                elapsed_ms: None,
                            })
                        }
                        loomcoding_core::task_events::StreamResponse::ArtifactUpdate(evento) => {
                            Some(A2aObservation {
                                direction: A2aDirection::Outbound,
                                agent: agent.clone(),
                                operation: A2aOperation::TaskArtifactUpdateEvent,
                                rpc_id: None,
                                task_id: Some(evento.task_id.clone()),
                                context_id: Some(evento.context_id.clone()),
                                state: None,
                                artifact: Some(
                                    evento
                                        .artifact
                                        .name
                                        .clone()
                                        .unwrap_or_else(|| evento.artifact.artifact_id.clone()),
                                ),
                                succeeded: true,
                                elapsed_ms: None,
                            })
                        }
                        loomcoding_core::task_events::StreamResponse::Task(_)
                        | loomcoding_core::task_events::StreamResponse::Message(_) => None,
                    };
                    if let Some(observacion) = observacion {
                        bus.publish(LiveEvent::A2aObserved(observacion));
                    }
                }
                let mut cable = WireStreamResponse::from(&evento);
                acotar_evento(&mut cable, maximo);
                serde_json::to_value(&cable).unwrap_or(serde_json::Value::Null)
            })
            .boxed())
    }

    pub(super) fn observe(
        &self,
        direction: A2aDirection,
        operation: A2aOperation,
        task_id: Option<String>,
        context_id: Option<String>,
        state: Option<String>,
        elapsed_ms: Option<u64>,
    ) {
        if let Some(bus) = &self.bus {
            bus.publish(LiveEvent::A2aObserved(A2aObservation {
                direction,
                agent: self.agent.clone(),
                operation,
                rpc_id: None,
                task_id,
                context_id,
                state,
                artifact: None,
                succeeded: true,
                elapsed_ms,
            }));
        }
    }

    pub(super) async fn start_with_operation(
        &self,
        prompt: String,
        operation: A2aOperation,
    ) -> Result<TaskView, HostError> {
        let started = Instant::now();
        let _turno = self.arranque.lock().await;
        let vivas = self
            .launcher
            .list(self.parent)
            .await
            .map_err(otro)?
            .iter()
            .filter(|descriptor| !descriptor.status.state.is_terminal())
            .count();
        if vivas >= self.limits.max_concurrent {
            return Err(HostError::Busy);
        }
        let (hijo, _) = self
            .launcher
            .start(
                self.parent,
                ETIQUETA.to_string(),
                prompt,
                Some(self.agent.clone()),
                CancellationToken::new(),
            )
            .await
            .map_err(otro)?;

        let (lanzador, padre, maximo) = (self.launcher.clone(), self.parent, self.limits.max_task);
        let (bus, agente) = (self.bus.clone(), self.agent.clone());
        let aviso = move |d: &SubagentDescriptor| {
            if let Some(bus) = &bus {
                bus.publish(LiveEvent::A2aServed {
                    task: d.child.0.to_string(),
                    agent: agente.clone(),
                    state: d.status.state.proto_name().to_string(),
                });
            }
        };
        tokio::spawn(async move {
            let limite = Instant::now() + maximo;
            let mut ultimo = None;
            loop {
                tokio::time::sleep(SONDEO.max(Duration::from_millis(100))).await;
                let actual = lanzador
                    .list(padre)
                    .await
                    .ok()
                    .and_then(|lista| lista.into_iter().find(|d| d.child == hijo));
                if let Some(descriptor) = &actual {
                    if ultimo != Some(descriptor.status.state) {
                        ultimo = Some(descriptor.status.state);
                        aviso(descriptor);
                    }
                }
                if actual.is_none_or(|d| d.status.state.is_terminal()) {
                    return;
                }
                if Instant::now() >= limite {
                    if let Ok(cancelada) = lanzador.cancel(padre, hijo).await {
                        aviso(&cancelada);
                    }
                    return;
                }
            }
        });

        let descriptor = self.descriptor(&hijo.0.to_string()).await?;
        self.observe(
            A2aDirection::Inbound,
            operation,
            Some(descriptor.child.0.to_string()),
            Some(descriptor.context_id()),
            Some(descriptor.status.state.proto_name().to_string()),
            Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        );
        Ok(self.vista(&descriptor))
    }

    pub(super) fn vista(&self, descriptor: &SubagentDescriptor) -> TaskView {
        let mut tarea = WireTask::from(descriptor);
        acotar_texto(&mut tarea, self.limits.max_result_bytes);
        TaskView {
            json: serde_json::to_value(&tarea).unwrap_or(serde_json::Value::Null),
            terminal: descriptor.status.state.is_terminal(),
        }
    }
}

pub(super) fn otro(error: CoreError) -> HostError {
    HostError::Other(error.to_string())
}
