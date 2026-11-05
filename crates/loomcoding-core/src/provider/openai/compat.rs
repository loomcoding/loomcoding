//! Proveedor de Chat Completions (OpenAI y compatibles): acumulador, mapeo SSE y mensajes en el formato del cable.

use super::*;

#[derive(Default)]
pub struct ToolCallAcc {
    pub(super) slots: Vec<(String, String, String)>,
}

impl ToolCallAcc {
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn apply(&mut self, tc: &serde_json::Value) {
        let idx = tc.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        while self.slots.len() <= idx {
            self.slots
                .push((String::new(), String::new(), String::new()));
        }
        let slot = &mut self.slots[idx];
        if let Some(id) = tc.get("id").and_then(|x| x.as_str()) {
            if !id.is_empty() {
                slot.0 = id.to_string();
            }
        }
        if let Some(name) = tc.pointer("/function/name").and_then(|x| x.as_str()) {
            if !name.is_empty() {
                slot.1 = name.to_string();
            }
        }
        if let Some(args) = tc.pointer("/function/arguments").and_then(|x| x.as_str()) {
            slot.2.push_str(args);
        }
    }

    pub fn finish(&mut self) -> Vec<StreamChunk> {
        std::mem::take(&mut self.slots)
            .into_iter()
            .filter(|(id, name, _)| !id.is_empty() || !name.is_empty())
            .map(|(id, name, args)| {
                let input = serde_json::from_str(&args).unwrap_or(serde_json::Value::Null);
                StreamChunk::ToolCallComplete { id, name, input }
            })
            .collect()
    }
}

/// Tokens de caché que comunica un objeto `usage` compatible con OpenAI, o `None` si no informa ninguno.
pub(super) fn cache_usage(usage: &serde_json::Value) -> Option<super::super::CacheUsage> {
    let details = &usage["prompt_tokens_details"];
    let read = details["cached_tokens"]
        .as_u64()
        .or_else(|| usage["prompt_cache_hit_tokens"].as_u64());
    let write = details["cache_write_tokens"].as_u64();
    if read.is_none() && write.is_none() {
        return None;
    }
    Some(super::super::CacheUsage {
        read_tokens: read.unwrap_or(0),
        write_tokens: write.unwrap_or(0),
    })
}

pub fn map_openai_event(data: &str, acc: &mut ToolCallAcc) -> Vec<StreamChunk> {
    let mut out = Vec::new();
    let v: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return out,
    };
    if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
        out.push(StreamChunk::Usage(Usage {
            input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            reported_cost_usd: u["cost"]
                .as_f64()
                .filter(|cost| cost.is_finite() && *cost >= 0.0),
        }));
        out.extend(cache_usage(u).map(StreamChunk::CacheUsage));
    }
    // Un servidor puede avisar de un fallo a mitad de flujo con `{"error": ...}` y cerrar con
    // `[DONE]`: sin esto el fallo pasaba por una parada normal y el turno quedaba vacío.
    if let Some(error) = v.get("error").filter(|error| !error.is_null()) {
        out.push(StreamChunk::Error(stream_error_text(error)));
        return out;
    }
    let Some(choices) = v.get("choices").and_then(|c| c.as_array()) else {
        return out;
    };
    for c in choices {
        if let Some(delta) = c.get("delta") {
            if let Some(t) = delta.get("content").and_then(|x| x.as_str()) {
                if !t.is_empty() {
                    out.push(StreamChunk::TextDelta(t.to_string()));
                }
            }
            if let Some(r) = delta.get("reasoning_content").and_then(|x| x.as_str()) {
                if !r.is_empty() {
                    out.push(StreamChunk::ReasoningDelta(r.to_string()));
                }
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|x| x.as_array()) {
                for tc in tcs {
                    acc.apply(tc);
                }
            }
        }
        if let Some(fr) = c.get("finish_reason").and_then(|x| x.as_str()) {
            out.extend(acc.finish());
            let reason = match fr {
                "length" => FinishReason::Length,
                "tool_calls" => FinishReason::ToolCalls,
                _ => FinishReason::Stop,
            };
            out.push(StreamChunk::Done(reason));
        }
    }
    out
}

pub struct OpenAiCompatProvider {
    pub(super) base_url: String,
    pub(super) api_key: String,
    pub(super) client: reqwest::Client,
    pub(super) request_timeout: Duration,
}

impl OpenAiCompatProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            request_timeout: Duration::from_secs(120),
        }
    }

    /// Sustituye el plazo total de cada petición (por defecto, 120 s). Cabe señalar que quien lo
    /// configura debe acotarlo antes (`MAX_REQUEST_TIMEOUT_SECS`); aquí se acepta tal cual.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub(super) fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
        let mut messages = vec![serde_json::json!({ "role": "system", "content": req.system })];
        messages.extend(wire_messages(&req.messages));
        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema
                    }
                })
            })
            .collect();
        let mut cuerpo = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "tools": tools,
            "stream": true,
            // Sin esta petición expresa, los servidores de tipo OpenAI no envían el
            // uso de tokens en el flujo y las métricas quedarían siempre a cero.
            "stream_options": { "include_usage": true },
            "temperature": req.temperature,
            "max_tokens": req.max_tokens
        });
        // Sin esfuerzo, el cuerpo queda exactamente como antes de existir este campo.
        if let Some(nivel) = req.effort.as_deref() {
            // OpenRouter no entiende `reasoning_effort`: su forma nativa anida el nivel.
            if self.base_url.contains("openrouter.ai") {
                cuerpo["reasoning"] = serde_json::json!({ "effort": nivel });
            } else {
                cuerpo["reasoning_effort"] = serde_json::json!(nivel);
            }
        }
        cuerpo
    }
}

/// Cota del cuerpo de una respuesta de error que viaja en el mensaje (decálogo de contexto y coste).
const MAX_ERROR_BODY_CHARS: usize = 512;

/// Texto de un error recibido dentro del flujo, acotado por caracteres.
pub(crate) fn stream_error_text(error: &serde_json::Value) -> String {
    let text = match error {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Object(map) => {
            let field = |name: &str| map.get(name).and_then(|value| value.as_str());
            match (field("type"), field("message")) {
                (Some(kind), Some(message)) => format!("{kind}: {message}"),
                (None, Some(message)) => message.to_string(),
                _ => error.to_string(),
            }
        }
        other => other.to_string(),
    };
    format!(
        "provider stream error: {}",
        text.chars().take(MAX_ERROR_BODY_CHARS).collect::<String>()
    )
}

/// `http {estado}: {cuerpo}`, con el cuerpo acotado por caracteres; sin cuerpo, solo `http {estado}`.
pub(super) fn http_error_text(status: reqwest::StatusCode, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return format!("http {status}");
    }
    format!(
        "http {status}: {}",
        body.chars().take(MAX_ERROR_BODY_CHARS).collect::<String>()
    )
}

pub(super) fn role_str(r: crate::message::Role) -> &'static str {
    use crate::message::Role::{Assistant, System, Tool, User};
    match r {
        System => "system",
        User => "user",
        Assistant => "assistant",
        Tool => "tool",
    }
}

/// Traduce los mensajes al formato Chat Completions, incluyendo `tool_calls`
/// en el asistente y `tool_call_id` en los resultados (uno por ToolResult).
pub(super) fn wire_messages(messages: &[crate::message::Message]) -> Vec<serde_json::Value> {
    use crate::message::{ContentPart, Role};
    let mut out = Vec::new();
    for m in messages {
        match m.role {
            Role::Assistant => {
                let calls: Vec<serde_json::Value> = m
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::ToolCall { id, name, input } => Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": { "name": name, "arguments": input.to_string() }
                        })),
                        _ => None,
                    })
                    .collect();
                let mut obj = serde_json::json!({ "role": "assistant", "content": m.text() });
                if !calls.is_empty() {
                    obj["tool_calls"] = serde_json::Value::Array(calls);
                }
                out.push(obj);
            }
            Role::Tool => {
                for p in &m.parts {
                    if let ContentPart::ToolResult {
                        call_id, content, ..
                    } = p
                    {
                        out.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call_id,
                            "content": crate::tools::limit_output(content)
                        }));
                    }
                }
            }
            other => {
                let has_image = m.parts.iter().any(|part| {
                    matches!(
                        part,
                        ContentPart::Image { .. } | ContentPart::ImageUrl { .. }
                    )
                });
                let content = if has_image {
                    serde_json::Value::Array(
                        m.parts
                            .iter()
                            .filter_map(|part| match part {
                                ContentPart::Text { text } => Some(serde_json::json!({
                                    "type": "text",
                                    "text": text
                                })),
                                ContentPart::Image { mime_type, data, .. } => Some(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": { "url": format!("data:{mime_type};base64,{data}") }
                                })),
                                ContentPart::ImageUrl { url } => Some(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": { "url": url }
                                })),
                                _ => None,
                            })
                            .collect(),
                    )
                } else {
                    serde_json::Value::String(m.text())
                };
                out.push(serde_json::json!({ "role": role_str(other), "content": content }));
            }
        }
    }
    out
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: true,
            streaming: true,
        }
    }

    async fn stream(
        &self,
        req: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .client
            .post(url)
            .timeout(self.request_timeout)
            .bearer_auth(&self.api_key)
            .json(&self.build_body(&req))
            .send()
            .await
            .map_err(|error| {
                let phase = if error.is_timeout() {
                    "timed out before response headers"
                } else if error.is_connect() {
                    "could not connect before response headers"
                } else {
                    "failed before response headers"
                };
                CoreError::Provider(format!("OpenAI-compatible request {phase}"))
            })?;
        if !resp.status().is_success() {
            let status = resp.status();
            // Sin el cuerpo no se puede reconocer el rechazo por exceso de contexto (`is_context_overflow`).
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Provider(http_error_text(status, &body)));
        }

        struct State<S> {
            stream: S,
            sse: SseBuffer,
            acc: ToolCallAcc,
            queue: std::collections::VecDeque<StreamChunk>,
            cancel: CancellationToken,
            done: bool,
        }

        let init = State {
            stream: resp.bytes_stream(),
            sse: SseBuffer::default(),
            acc: ToolCallAcc::default(),
            queue: std::collections::VecDeque::new(),
            cancel,
            done: false,
        };

        let s = futures::stream::unfold(init, |mut st| async move {
            loop {
                if st.cancel.is_cancelled() {
                    return None;
                }
                if let Some(chunk) = st.queue.pop_front() {
                    if matches!(chunk, StreamChunk::Done(_)) {
                        st.done = true;
                    }
                    return Some((chunk, st));
                }
                if st.done {
                    return None;
                }
                match st.stream.next().await {
                    Some(Ok(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for data in st.sse.push(&text) {
                            if data == "[DONE]" {
                                st.queue.extend(st.acc.finish());
                                st.queue.push_back(StreamChunk::Done(FinishReason::Stop));
                            } else {
                                st.queue.extend(map_openai_event(&data, &mut st.acc));
                            }
                        }
                    }
                    Some(Err(error)) => {
                        st.done = true;
                        st.queue
                            .push_back(StreamChunk::Error(if error.is_timeout() {
                                "OpenAI-compatible response stream timed out".into()
                            } else {
                                "OpenAI-compatible response stream failed".into()
                            }));
                    }
                    None => {
                        st.queue.extend(st.acc.finish());
                        if !st.queue.iter().any(|c| matches!(c, StreamChunk::Done(_))) {
                            st.queue.push_back(StreamChunk::Done(FinishReason::Stop));
                        }
                    }
                }
            }
        });

        let stream: ProviderStream = Box::pin(s);
        if req.programmatic_tool_calling && req.tools.iter().any(|tool| tool.programmatic_allowed) {
            let notice = format!(
                "OpenAI-compatible model `{}` uses direct calls; programmatic calling is available only through Responses",
                req.model
            );
            Ok(Box::pin(
                futures::stream::once(async move { StreamChunk::ProviderNotice(notice) })
                    .chain(stream),
            ))
        } else {
            Ok(stream)
        }
    }
}
