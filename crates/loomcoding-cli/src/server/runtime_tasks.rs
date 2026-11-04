//! Tareas del runtime y supervisor: vista de tarea, configuración del usuario y ejecución del supervisor.

use super::*;

pub(super) struct RuntimeTask {
    pub(super) id: String,
    pub(super) tenant: String,
    pub(super) context_id: String,
    pub(super) cancel: CancellationToken,
    pub(super) current: StdRwLock<TaskView>,
    pub(super) updates: broadcast::Sender<TaskView>,
}

#[derive(Deserialize)]
pub(super) struct RuntimeUserConfig {
    pub(super) supervisor: Option<SupervisorSettings>,
}

#[derive(Deserialize)]
pub(super) struct SupervisorSettings {
    pub(super) provider: loomcoding_core::config::ProviderConfig,
    pub(super) model: String,
}

#[derive(Clone)]
pub(super) struct SupervisorAgent {
    pub(super) provider: Arc<dyn Provider>,
    pub(super) model: String,
    pub(super) timeout: Duration,
    pub(super) session: SessionId,
    pub(super) store: Arc<loomcoding_core::session::sqlite::SqliteStore>,
}

pub(super) type LoadedSession = (
    SessionId,
    Vec<loomcoding_core::message::Message>,
    Vec<loomcoding_core::session::UsageRecord>,
    Vec<loomcoding_core::session::SubagentDescriptor>,
);

pub(super) const MAX_RUNTIME_TASK_RESULT_BYTES: usize = 32 * 1024;

impl RuntimeTask {
    pub(super) fn new(id: String, tenant: String, context_id: String) -> Self {
        let current = runtime_task_view(
            &id,
            &context_id,
            loomcoding_core::task::TaskState::Submitted,
            None,
        );
        Self {
            id,
            tenant,
            context_id,
            cancel: CancellationToken::new(),
            current: StdRwLock::new(current),
            updates: broadcast::channel(16).0,
        }
    }

    pub(super) fn update(
        &self,
        state: loomcoding_core::task::TaskState,
        result: Option<&str>,
    ) -> TaskView {
        let view = runtime_task_view(&self.id, &self.context_id, state, result);
        *self.current.write().unwrap() = view.clone();
        let _ = self.updates.send(view.clone());
        view
    }

    pub(super) fn view(&self) -> TaskView {
        self.current.read().unwrap().clone()
    }
}

pub(super) fn runtime_task_view(
    id: &str,
    context_id: &str,
    state: loomcoding_core::task::TaskState,
    result: Option<&str>,
) -> TaskView {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let mut task = json!({
        "id": id,
        "contextId": context_id,
        "status": {
            "state": state.proto_name(),
            "timestamp": loomcoding_a2a::wire::timestamp_to_wire(now),
        },
    });
    if let Some(result) = result {
        let result =
            loomcoding_core::tools::limit_bytes(result, MAX_RUNTIME_TASK_RESULT_BYTES - 64);
        task["artifacts"] = json!([{
            "artifactId": "final-result",
            "parts": [{"text": result.as_ref()}],
        }]);
    }
    TaskView {
        json: task,
        terminal: state.is_terminal(),
    }
}

pub(super) async fn ejecutar_supervisor(
    task: Arc<RuntimeTask>,
    supervisor: SupervisorAgent,
    prompt: String,
    snapshot: String,
    fallback: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let input = format!("Request: {prompt}\n\nCurrent runtime snapshot:\n{snapshot}");
    let user_message = Message::user(input.clone());
    let _ = supervisor
        .store
        .append_message(supervisor.session, &user_message)
        .await;
    let request = ProviderRequest {
        model: supervisor.model.clone(),
        system: "You are the Loomcoding runtime supervisor. Answer using only the supplied runtime snapshot. You may explain status and suggest actions, but you cannot execute tools, alter permissions, or approve operations. If the snapshot lacks information, say so.".into(),
        messages: vec![user_message],
        tools: Vec::new(),
        temperature: 0.1,
        max_tokens: 512,
        effort: None,
        programmatic_tool_calling: false,
    };
    let cancel = task.cancel.clone();
    let result = tokio::select! {
        _ = cancel.cancelled() => {
            task.update(loomcoding_core::task::TaskState::Canceled, None);
            return;
        }
        result = tokio::time::timeout(supervisor.timeout, async {
            let mut stream = supervisor.provider.stream(request, cancel.clone()).await?;
            let mut text = String::new();
            let mut usage = None;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    StreamChunk::TextDelta(delta) if text.len() < 8 * 1024 => {
                        text.push_str(&delta);
                    }
                    StreamChunk::Usage(value) => usage = Some(value),
                    _ => {}
                }
            }
            Ok::<_, loomcoding_core::error::CoreError>((text, usage))
        }) => result.ok().and_then(|result| result.ok()),
    };
    let answer = match result {
        Some((text, usage)) if !text.trim().is_empty() => {
            let text = loomcoding_core::tools::limit_bytes(&text, 8 * 1024).into_owned();
            let assistant = Message::assistant(vec![loomcoding_core::message::ContentPart::Text {
                text: text.clone(),
            }]);
            let _ = supervisor
                .store
                .append_message(supervisor.session, &assistant)
                .await;
            if let Some(usage) = usage {
                let created_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let _ = supervisor
                    .store
                    .record_usage(
                        supervisor.session,
                        &loomcoding_core::session::UsageRecord {
                            session: supervisor.session,
                            model: supervisor.model.clone(),
                            input_tokens: usage.input_tokens.into(),
                            output_tokens: usage.output_tokens.into(),
                            input_price: None,
                            output_price: None,
                            reported_cost_usd: usage.reported_cost_usd,
                            source: loomcoding_core::message::UsageSource::Provider,
                            is_final: true,
                            components: Vec::new(),
                            cache_read_tokens: 0,
                            cache_write_tokens: 0,
                            created_at_ms,
                        },
                    )
                    .await;
            }
            text
        }
        _ => {
            let assistant = Message::assistant(vec![loomcoding_core::message::ContentPart::Text {
                text: fallback.clone(),
            }]);
            let _ = supervisor
                .store
                .append_message(supervisor.session, &assistant)
                .await;
            fallback
        }
    };
    task.update(loomcoding_core::task::TaskState::Completed, Some(&answer));
}
