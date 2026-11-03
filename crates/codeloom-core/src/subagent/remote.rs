//! Hijos que son agentes A2A remotos: el trabajo espejo que sigue la tarea remota y copia su estado a la local.

use super::*;

/// Cada cuánto se consulta una tarea remota.
pub(super) const REMOTE_POLL: std::time::Duration = std::time::Duration::from_secs(1);

/// Lo máximo que se sigue una tarea remota antes de darla por perdida (decálogo: todo bucle tiene cota).
pub(super) const REMOTE_MAX_FOLLOW: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Cuánto se espera a que el remoto abra el flujo de la tarea (A8c) antes de sondear.
pub(super) const REMOTE_WATCH_OPEN_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(10);

/// Rondas de pregunta (`input-required`) que se atienden como máximo en una tarea remota.
pub(super) const REMOTE_MAX_RONDAS_ENTRADA: u32 = 5;

/// Lo máximo que se espera a que la persona conteste a una pregunta de un remoto.
pub(super) const REMOTE_ESPERA_RESPUESTA: std::time::Duration =
    std::time::Duration::from_secs(30 * 60);

/// Bytes de la pregunta de un remoto que se muestran (el texto es de un tercero).
pub(super) const REMOTE_MAX_BYTES_PREGUNTA: usize = 2_000;

/// Lo máximo que se espera a que un remoto conteste a `CancelTask`; menor que `CANCEL_WAIT`.
pub(super) const REMOTE_CANCEL_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(1500);

/// Consultas fallidas seguidas tras las que se da por perdido el contacto con el remoto.
pub(super) const REMOTE_MAX_FALLOS_SEGUIDOS: u32 = 3;

/// Todo lo que el trabajo espejo de un agente remoto necesita para seguir su tarea.
pub(super) struct EjecucionRemota {
    pub(super) cliente: Arc<dyn RemoteTaskClient>,
    pub(super) parent: SessionId,
    pub(super) child: SessionId,
    pub(super) prompt: String,
    pub(super) tracker: Arc<TaskTracker>,
    pub(super) store: Arc<dyn SessionStore>,
    pub(super) permit: OwnedSemaphorePermit,
    pub(super) puerta: tokio::sync::oneshot::Receiver<()>,
    pub(super) parent_cancel: CancellationToken,
    pub(super) poll: std::time::Duration,
    pub(super) max_follow: std::time::Duration,
    /// Buzón de preguntas, bus y etiqueta: para preguntar a la persona lo que el remoto pida.
    pub(super) broker: Arc<InputBroker>,
    pub(super) bus: EventBus,
    pub(super) label: String,
    pub(super) input_wait: std::time::Duration,
}

pub(super) fn cuerpo_remoto(ejecucion: EjecucionRemota) -> JobBody {
    Box::new(move |handle, job_cancel| Box::pin(seguir_remoto(ejecucion, handle, job_cancel)))
}

/// Estado local con el mensaje que explica la razón, con la marca de tiempo de este proceso
/// (la del remoto no es de fiar: es de un tercero).
pub(super) fn estado_con_mensaje(state: TaskState, mensaje: Option<Message>) -> TaskStatus {
    TaskStatus {
        message: mensaje,
        ..TaskStatus::new(state)
    }
}

/// Los pasos del espejo que cambian la tarea local.
pub(super) struct Espejo {
    pub(super) cliente: Arc<dyn RemoteTaskClient>,
    pub(super) parent: SessionId,
    pub(super) child: SessionId,
    pub(super) tracker: Arc<TaskTracker>,
    pub(super) store: Arc<dyn SessionStore>,
    pub(super) broker: Arc<InputBroker>,
    pub(super) bus: EventBus,
    pub(super) label: String,
}

/// Cómo acaba la espera de la respuesta a una pregunta del remoto.
pub(super) enum Entrada {
    Respuesta(String),
    /// La persona no contestó a tiempo.
    Expirada,
    /// La petición se descartó sin respuesta.
    Descartada,
    /// Se canceló el hijo o el padre mientras se esperaba.
    Cancelada,
}

/// Texto de la pregunta de un remoto, listo para mostrarse: sin caracteres de control (salvo saltos
/// y tabuladores), acotado y nunca vacío. Es dato de un tercero: se muestra, no se interpreta.
pub(super) fn pregunta_del_remoto(mensaje: Option<&Message>) -> String {
    let plano: String = mensaje
        .map(Message::text)
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let plano = plano.trim();
    if plano.is_empty() {
        return "The remote agent asks for input but gave no text.".to_string();
    }
    crate::tools::limit_bytes(plano, REMOTE_MAX_BYTES_PREGUNTA).into_owned()
}

impl Espejo {
    /// Pregunta a la persona (por el buzón de los hijos) lo que el remoto pide y espera su respuesta,
    /// con tope de tiempo. Cancelar el hijo o el padre retira la pregunta.
    pub(super) async fn preguntar(
        &self,
        texto: String,
        espera: std::time::Duration,
        job_cancel: &CancellationToken,
        parent_cancel: &CancellationToken,
    ) -> Entrada {
        let pregunta = crate::question::Question {
            header: "Remote agent".to_string(),
            question: texto,
            options: Vec::new(),
            multi_select: false,
        };
        let (parent, child) = (self.parent, self.child);
        let (bus, label) = (self.bus.clone(), self.label.clone());
        let peticion = self.broker.request_with(
            parent,
            child,
            self.label.clone(),
            vec![pregunta],
            move || {
                bus.publish(LiveEvent::SubagentInputRequested {
                    parent,
                    child,
                    label,
                });
            },
        );
        let resultado = tokio::select! {
            respuestas = peticion => match respuestas {
                Some(respuestas) => Entrada::Respuesta(
                    respuestas
                        .iter()
                        .flat_map(|r| r.selected.iter().cloned())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                None => Entrada::Descartada,
            },
            _ = tokio::time::sleep(espera) => Entrada::Expirada,
            _ = job_cancel.cancelled() => Entrada::Cancelada,
            _ = parent_cancel.cancelled() => {
                job_cancel.cancel();
                Entrada::Cancelada
            }
        };
        // La petición se retira al soltarse; la marca de espera de la interfaz se quita aquí.
        self.bus
            .publish(LiveEvent::SubagentInputResolved { parent, child });
        resultado
    }

    pub(super) async fn transicion(&self, status: TaskStatus) {
        if let Err(error) = self
            .tracker
            .transition(self.parent, self.child, status)
            .await
        {
            tracing::warn!("persist subagent status: {error}");
        }
    }

    /// Cierra la tarea como fallida con la razón en su mensaje y devuelve el error del trabajo.
    pub(super) async fn fallar(&self, razon: String) -> Result<Option<JobResult>> {
        self.transicion(estado_con_mensaje(
            TaskState::Failed,
            Some(Message::agent_text(razon.as_str())),
        ))
        .await;
        Err(CoreError::Tool(razon))
    }

    /// Pide al remoto cancelar y deja la tarea local `canceled` con lo que el remoto haya dicho:
    /// sin su confirmación no se afirma que quedó cancelada allí.
    pub(super) async fn cancelar(&self, id: Option<&str>) -> Result<Option<JobResult>> {
        let nota = match id {
            None => "cancelled before the remote assigned a task id".to_string(),
            Some(id) => {
                match tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, self.cliente.cancel(id)).await {
                    Ok(Ok(tarea)) if tarea.status.state == TaskState::Canceled => {
                        "cancellation confirmed by the remote".to_string()
                    }
                    Ok(Ok(tarea)) => format!(
                        "cancellation requested; the remote reports {}",
                        tarea.status.state.proto_name()
                    ),
                    Ok(Err(RemoteTaskError::NotCancelable)) => {
                        "cancellation requested; the remote says the task is not cancelable"
                            .to_string()
                    }
                    Ok(Err(error)) => {
                        format!("cancellation requested; the remote did not confirm it: {error}")
                    }
                    Err(_) => {
                        "cancellation requested; the remote did not answer in time".to_string()
                    }
                }
            }
        };
        self.transicion(estado_con_mensaje(
            TaskState::Canceled,
            Some(Message::agent_text(nota.as_str())),
        ))
        .await;
        Ok(None)
    }

    /// Cierra la tarea con lo que el remoto dice que acabó (estado terminal).
    pub(super) async fn terminar(
        &self,
        tarea: RemoteTask,
        handle: &JobHandle,
    ) -> Result<Option<JobResult>> {
        if tarea.status.state != TaskState::Completed {
            let razon = tarea
                .status
                .message
                .as_ref()
                .map(Message::text)
                .filter(|texto| !texto.trim().is_empty())
                .unwrap_or_else(|| format!("remote task {}", tarea.status.state.proto_name()));
            self.transicion(estado_con_mensaje(
                tarea.status.state,
                tarea.status.message.clone(),
            ))
            .await;
            return Err(CoreError::Tool(razon));
        }
        let mut artifacts = tarea.artifacts;
        if artifacts.is_empty() {
            if let Some(texto) = tarea
                .status
                .message
                .as_ref()
                .map(Message::text)
                .filter(|texto| !texto.trim().is_empty())
            {
                artifacts.push(Artifact::text_artifact(texto));
            }
        }
        self.store
            .complete_subagent(self.parent, self.child, &artifacts)
            .await?;
        if let Err(error) = self.tracker.publish(self.parent, self.child).await {
            tracing::warn!("publish subagent status: {error}");
        }
        let texto = crate::a2a::artifacts_text(&artifacts);
        handle.append(&texto);
        Ok(Some(JobResult { content: texto }))
    }
}

/// Espejo de una tarea remota: envía el mensaje, la sigue por sondeo y copia su estado y sus
/// artefactos al descriptor local, de modo que el árbol, la cancelación y el presupuesto de tareas
/// la tratan como a cualquier subagente.
pub(super) async fn seguir_remoto(
    e: EjecucionRemota,
    handle: JobHandle,
    job_cancel: CancellationToken,
) -> Result<Option<JobResult>> {
    let EjecucionRemota {
        cliente,
        parent,
        child,
        prompt,
        tracker,
        store,
        permit,
        puerta,
        parent_cancel,
        poll,
        max_follow,
        broker,
        bus,
        label,
        input_wait,
    } = e;
    let _permit = permit;
    // La compuerta retiene el trabajo hasta que la tarea `submitted` está guardada.
    if puerta.await.is_err() || job_cancel.is_cancelled() {
        return Ok(None);
    }
    let espejo = Espejo {
        cliente: cliente.clone(),
        parent,
        child,
        tracker,
        store,
        broker,
        bus,
        label,
    };
    espejo.transicion(TaskStatus::new(TaskState::Working)).await;

    let enviado = tokio::select! {
        respuesta = cliente.send(Message::user_text(prompt)) => respuesta,
        _ = job_cancel.cancelled() => return espejo.cancelar(None).await,
        _ = parent_cancel.cancelled() => {
            job_cancel.cancel();
            return espejo.cancelar(None).await;
        }
    };
    let mut tarea = match enviado {
        Ok(RemoteReply::Task(tarea)) => tarea,
        Ok(RemoteReply::Message(mensaje)) => {
            // El remoto contestó sin crear tarea: su mensaje es el resultado.
            let tarea = RemoteTask {
                id: String::new(),
                context_id: None,
                status: TaskStatus::new(TaskState::Completed),
                artifacts: vec![Artifact::text_artifact(mensaje.text())],
            };
            return espejo.terminar(tarea, &handle).await;
        }
        Err(error) => return espejo.fallar(format!("remote send failed: {error}")).await,
    };

    let id = tarea.id.clone();
    let mut inicio = tokio::time::Instant::now();
    let mut ultimo = TaskState::Working;
    let mut fallos = 0u32;
    let mut rondas = 0u32;
    // Cada ronda sigue la tarea hasta un estado terminal o hasta que el remoto pide entrada; en este
    // último caso se pregunta a la persona, se responde a la misma tarea y se empieza otra ronda.
    'ronda: loop {
        // A8c: si el remoto declara streaming se sigue el flujo en lugar de sondear. Cualquier fallo
        // al abrirlo, un error a mitad, un cierre antes del estado terminal o una tarea ajena
        // devuelven el seguimiento al sondeo de siempre: el flujo es una mejora, nunca la única vía.
        if !tarea.status.state.is_terminal() && tarea.status.state != TaskState::InputRequired {
            let apertura = tokio::select! {
                apertura = tokio::time::timeout(REMOTE_WATCH_OPEN_TIMEOUT, cliente.watch(&id)) => apertura,
                _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                _ = parent_cancel.cancelled() => {
                    job_cancel.cancel();
                    return espejo.cancelar(Some(&id)).await;
                }
            };
            if let Ok(Ok(mut flujo)) = apertura {
                let limite = inicio + max_follow;
                loop {
                    tokio::select! {
                        siguiente = futures::StreamExt::next(&mut flujo) => match siguiente {
                            Some(Ok(nueva)) if nueva.id == id => {
                                let estado = nueva.status.state;
                                tarea = nueva;
                                if estado.is_terminal() {
                                    return espejo.terminar(tarea, &handle).await;
                                }
                                if estado != ultimo {
                                    espejo
                                        .transicion(estado_con_mensaje(
                                            estado,
                                            tarea.status.message.clone(),
                                        ))
                                        .await;
                                    ultimo = estado;
                                }
                                // El remoto pide entrada: se atiende fuera del flujo.
                                if estado == TaskState::InputRequired {
                                    break;
                                }
                            }
                            _ => break,
                        },
                        _ = tokio::time::sleep_until(limite) => break,
                        _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                        _ = parent_cancel.cancelled() => {
                            job_cancel.cancel();
                            return espejo.cancelar(Some(&id)).await;
                        }
                    }
                }
            }
        }
        loop {
            let estado = tarea.status.state;
            if estado.is_terminal() {
                return espejo.terminar(tarea, &handle).await;
            }
            if estado != ultimo {
                espejo
                    .transicion(estado_con_mensaje(estado, tarea.status.message.clone()))
                    .await;
                ultimo = estado;
            }
            if estado == TaskState::InputRequired {
                rondas += 1;
                if rondas > REMOTE_MAX_RONDAS_ENTRADA {
                    let _ = tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, cliente.cancel(&id)).await;
                    return espejo
                        .fallar(format!(
                            "the remote asked for input more than {REMOTE_MAX_RONDAS_ENTRADA} times"
                        ))
                        .await;
                }
                let texto = pregunta_del_remoto(tarea.status.message.as_ref());
                match espejo
                    .preguntar(texto, input_wait, &job_cancel, &parent_cancel)
                    .await
                {
                    Entrada::Respuesta(respuesta) => {
                        let mut mensaje = Message::user_text(respuesta);
                        mensaje.task_id = Some(id.clone());
                        mensaje.context_id = tarea.context_id.clone();
                        let enviado = tokio::select! {
                            respuesta = cliente.send(mensaje) => respuesta,
                            _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                            _ = parent_cancel.cancelled() => {
                                job_cancel.cancel();
                                return espejo.cancelar(Some(&id)).await;
                            }
                        };
                        match enviado {
                            Ok(RemoteReply::Task(nueva)) if nueva.id == id => tarea = nueva,
                            Ok(RemoteReply::Task(_)) => {
                                return espejo
                                    .fallar("the remote answered about a different task".into())
                                    .await
                            }
                            Ok(RemoteReply::Message(mensaje)) => {
                                // Contestó con un mensaje y sin tarea: ese mensaje es el resultado.
                                let cierre = RemoteTask {
                                    id: id.clone(),
                                    context_id: tarea.context_id.clone(),
                                    status: TaskStatus::new(TaskState::Completed),
                                    artifacts: vec![Artifact::text_artifact(mensaje.text())],
                                };
                                return espejo.terminar(cierre, &handle).await;
                            }
                            Err(error) => {
                                return espejo.fallar(format!("remote send failed: {error}")).await
                            }
                        }
                        if !tarea.status.state.is_terminal() {
                            espejo.transicion(TaskStatus::new(TaskState::Working)).await;
                        }
                        ultimo = TaskState::Working;
                        // Esperar a la persona no cuenta contra el límite de seguimiento.
                        inicio = tokio::time::Instant::now();
                        fallos = 0;
                        continue 'ronda;
                    }
                    Entrada::Expirada => {
                        let _ =
                            tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, cliente.cancel(&id)).await;
                        return espejo
                            .fallar("nobody answered the remote agent's question in time".into())
                            .await;
                    }
                    Entrada::Descartada | Entrada::Cancelada => {
                        return espejo.cancelar(Some(&id)).await
                    }
                }
            }
            if inicio.elapsed() >= max_follow {
                // Se avisa al remoto de que ya no interesa, sin esperar su confirmación.
                let _ = tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, cliente.cancel(&id)).await;
                return espejo
                    .fallar(format!(
                        "gave up following the remote task after {} s",
                        max_follow.as_secs()
                    ))
                    .await;
            }
            tokio::select! {
                _ = tokio::time::sleep(poll) => {}
                _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                _ = parent_cancel.cancelled() => {
                    job_cancel.cancel();
                    return espejo.cancelar(Some(&id)).await;
                }
            }
            let consulta = tokio::select! {
                respuesta = cliente.get(&id) => respuesta,
                _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                _ = parent_cancel.cancelled() => {
                    job_cancel.cancel();
                    return espejo.cancelar(Some(&id)).await;
                }
            };
            match consulta {
                Ok(nueva) if nueva.id == id => {
                    fallos = 0;
                    tarea = nueva;
                }
                Ok(_) => {
                    return espejo
                        .fallar("the remote answered about a different task".into())
                        .await
                }
                Err(RemoteTaskError::NotFound) => {
                    return espejo
                        .fallar("the remote no longer knows the task".into())
                        .await
                }
                Err(error) => {
                    fallos += 1;
                    if fallos >= REMOTE_MAX_FALLOS_SEGUIDOS {
                        return espejo
                            .fallar(format!("lost contact with the remote: {error}"))
                            .await;
                    }
                }
            }
        }
    }
}
