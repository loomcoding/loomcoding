//! Interacción con hijos vivos: tiempos de espera de la cancelación y prompt de los seguimientos.

use super::*;

/// Lo máximo que `cancel` espera a que el hijo se asiente tras la orden de parar.
pub(super) const CANCEL_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Cada cuánto `cancel` relee el estado del hijo mientras espera.
pub(super) const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// Prompt de un seguimiento (D4 de A11): el resultado de la tarea anterior, acotado por la regla
/// 8 del decálogo, antepuesto a la petición. Sin resultado se escribe `(no result)` y, si lo hay,
/// el texto del mensaje de su estado.
pub(super) fn prompt_de_seguimiento(anterior: &SubagentDescriptor, peticion: &str) -> String {
    let resultado = anterior.result_text().unwrap_or_else(|| {
        match anterior
            .status
            .message
            .as_ref()
            .map(|mensaje| mensaje.text())
            .filter(|texto| !texto.trim().is_empty())
        {
            Some(texto) => format!("(no result) {texto}"),
            None => "(no result)".to_string(),
        }
    });
    format!(
        "[Follow-up of subagent {} \"{}\" ({})]\nPrevious result:\n{}\n\n{}",
        anterior.child.0,
        anterior.label,
        anterior.status.state.proto_name(),
        crate::tools::limit_bytes(&resultado, crate::tools::MAX_NOTICE_BYTES),
        peticion
    )
}

impl InProcessSubagentLauncher {
    pub(super) async fn arrancar_seguimiento(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        follow_up_of: SessionId,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        validar_peticion(&label, &prompt)?;
        // Solo se sigue a una tarea terminal de este mismo padre; de otro padre no se revela.
        let anterior = self
            .store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == follow_up_of)
            .ok_or(CoreError::TaskNotFound(follow_up_of.0))?;
        if !anterior.status.state.is_terminal() {
            return Err(CoreError::Tool(format!(
                "subagent {} is still {}; wait for it to finish, or continue it with send_message or answer",
                follow_up_of.0,
                anterior.status.state.proto_name()
            )));
        }
        let prompt = prompt_de_seguimiento(&anterior, &prompt);
        self.arrancar(
            parent,
            label,
            prompt,
            agent,
            vec![follow_up_of],
            false,
            parent_cancel,
        )
        .await
    }

    pub(super) async fn controlar(
        &self,
        parent: SessionId,
        child: SessionId,
        action: &str,
        message: Option<String>,
    ) -> Result<()> {
        if action == "interrupt" {
            return self.cancel(parent, child).await.map(|_| ());
        }
        let descriptor = self
            .store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or_else(|| CoreError::Tool("unknown subagent".into()))?;
        let job = JobId(descriptor.job_id);
        if descriptor.status.state.is_terminal() {
            return Err(CoreError::Tool("subagent has already finished".into()));
        }
        let snapshot = self
            .jobs
            .list(parent)
            .into_iter()
            .find(|snapshot| snapshot.id == job)
            .ok_or_else(|| CoreError::Tool("unknown subagent".into()))?;
        if !matches!(
            snapshot.status,
            crate::job::JobStatus::Running | crate::job::JobStatus::Stopping
        ) {
            return Err(CoreError::Tool("subagent has already finished".into()));
        }
        match action {
            "send_message" => {
                let message = message
                    .filter(|text| !text.trim().is_empty() && text.len() <= 32_768)
                    .ok_or_else(|| CoreError::Tool("message must be 1 to 32768 bytes".into()))?;
                self.store
                    .append_message(child, &crate::message::Message::user(message))
                    .await
            }
            _ => Err(CoreError::Tool(
                "action must be send_message or interrupt".into(),
            )),
        }
    }

    pub(super) async fn cancelar(
        &self,
        parent: SessionId,
        child: SessionId,
    ) -> Result<SubagentDescriptor> {
        let descriptor = self
            .store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or(CoreError::TaskNotFound(child.0))?;
        if descriptor.status.state.is_terminal() {
            return Err(CoreError::TaskNotCancelable {
                child: child.0,
                state: descriptor.status.state,
            });
        }
        // Solo se ordena parar a un trabajo en marcha: si ya se está deteniendo, la orden se dio
        // antes y repetirla no cambia nada (idempotente mientras no se asiente).
        let job = JobId(descriptor.job_id.clone());
        let en_marcha = self.jobs.list(parent).into_iter().any(|snapshot| {
            snapshot.id == job && snapshot.status == crate::job::JobStatus::Running
        });
        if en_marcha {
            self.jobs.kill(parent, &job)?;
        }
        let asentado = async {
            loop {
                let actual = self.descriptor_of(parent, child).await?;
                if actual.status.state.is_terminal() {
                    return Ok::<_, CoreError>(actual);
                }
                tokio::time::sleep(CANCEL_POLL).await;
            }
        };
        match tokio::time::timeout(self.cancel_wait, asentado).await {
            Ok(resultado) => resultado,
            // No se asentó a tiempo: se devuelve lo último que consta, que puede no ser `Canceled`.
            Err(_) => self.descriptor_of(parent, child).await,
        }
    }

    pub(super) async fn avisos_de_resultado(
        &self,
        parent: SessionId,
    ) -> Result<Vec<crate::message::Message>> {
        let mut messages = Vec::new();
        for descriptor in self.store.list_subagents(parent).await? {
            if descriptor.status.state != TaskState::Completed || descriptor.notice_delivered {
                continue;
            }
            if let Some(result) = descriptor.result_text() {
                let mut texto = format!(
                    "[Subagent {} completed]\n{}",
                    descriptor.label,
                    crate::tools::limit_bytes(&result, crate::tools::MAX_NOTICE_BYTES)
                );
                // Los avances no se vuelcan: se nombran con su tamaño y se leen con `artifact_read`.
                let avances: Vec<String> = descriptor
                    .progress_artifacts()
                    .map(|a| {
                        format!(
                            "{} ({} bytes)",
                            a.name.as_deref().unwrap_or("?"),
                            a.text().len()
                        )
                    })
                    .collect();
                if !avances.is_empty() {
                    texto.push_str(&format!(
                        "\n[Progress notes: {}; read with artifact_read]",
                        avances.join(", ")
                    ));
                }
                let message = crate::message::Message::user(texto);
                if self
                    .store
                    .append_subagent_notice(parent, descriptor.child, &message)
                    .await?
                {
                    messages.push(message);
                }
            }
        }
        Ok(messages)
    }

    pub(super) async fn avisos_de_entrada(
        &self,
        parent: SessionId,
    ) -> Result<Vec<crate::message::Message>> {
        Ok(self
            .broker
            .take_unannounced(parent)
            .into_iter()
            .map(|input| {
                let mut lines = vec![format!("[Subagent {} needs input]", input.label)];
                for (index, question) in input.questions.iter().enumerate() {
                    let options: Vec<&str> = question
                        .options
                        .iter()
                        .map(|option| option.label.as_str())
                        .collect();
                    lines.push(format!("{}. {}", index + 1, question.question));
                    lines.push(format!("   options: {}", options.join(" | ")));
                }
                lines.push(format!(
                    "Reply with subagent_control action=answer child_id={} answers=[<one string per question, in order>]",
                    input.child.0
                ));
                crate::message::Message::user(lines.join("
"))
            })
            .collect())
    }
}
