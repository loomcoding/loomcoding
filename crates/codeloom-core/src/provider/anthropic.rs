use super::sse::SseBuffer;
use super::{Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk};
use crate::error::{CoreError, Result};
use crate::message::FinishReason;
use async_trait::async_trait;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

const OPENROUTER_HAIKU_55: &str = "anthropic/claude-haiku-5.5";
const OPENROUTER_PTC_UNSUPPORTED: &str = "OpenRouter rejected Anthropic programmatic tool calling";

/// Tope del estado de continuación; el agente rechaza los que superen 64 KiB, así que
/// uno mayor se descarta aquí en lugar de abortar el turno.
const MAX_CONTINUATION_STATE_BYTES: usize = 60 * 1024;

#[derive(Default)]
pub struct AnthropicAcc {
    blocks: Vec<(String, String, String)>, // (id, name, json) por index
    /// Bloques de contenido tal como los emitió el modelo, por índice.
    content: Vec<serde_json::Value>,
    has_thinking: bool,
    /// Modelo y `base_url` con los que se firma el estado de continuación.
    identity: Option<(String, String)>,
}

impl AnthropicAcc {
    /// Acumulador que, si la respuesta trae razonamiento, emite su estado de continuación.
    pub fn for_provider(model: &str, base_url: &str) -> Self {
        Self {
            identity: Some((model.to_string(), base_url.to_string())),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    fn content_slot(&mut self, idx: usize) -> &mut serde_json::Value {
        while self.content.len() <= idx {
            self.content.push(serde_json::Value::Null);
        }
        &mut self.content[idx]
    }

    pub fn finish(&mut self) -> Vec<StreamChunk> {
        let state = self.continuation_state();
        let mut out: Vec<StreamChunk> = std::mem::take(&mut self.blocks)
            .into_iter()
            .filter(|(_, name, _)| !name.is_empty())
            .map(|(id, name, json)| StreamChunk::ToolCallComplete {
                id,
                name,
                input: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            })
            .collect();
        out.extend(state.map(StreamChunk::ProviderState));
        out
    }

    /// Cabe señalar que con `thinking` activo la API exige reenviar, junto a cada `tool_use`,
    /// el bloque de razonamiento con su firma; sin él responde 400. Se conserva solo si lo hubo.
    fn continuation_state(&mut self) -> Option<crate::message::ProviderState> {
        let (model, base_url) = self.identity.clone().filter(|_| self.has_thinking)?;
        let mut content = Vec::new();
        for (idx, block) in std::mem::take(&mut self.content).into_iter().enumerate() {
            let mut block = block;
            if block["type"] == "tool_use" {
                let json = self
                    .blocks
                    .get(idx)
                    .map_or("", |(_, _, json)| json.as_str());
                block["input"] =
                    serde_json::from_str(json).unwrap_or_else(|_| serde_json::json!({}));
            }
            if !block.is_null() {
                content.push(block);
            }
        }
        let state = crate::message::ProviderState {
            provider: "anthropic".into(),
            data: serde_json::json!({
                "model": model,
                "base_url": base_url,
                "assistant_content": content,
            }),
        };
        let size = serde_json::to_vec(&state).map_or(usize::MAX, |bytes| bytes.len());
        (size <= MAX_CONTINUATION_STATE_BYTES).then_some(state)
    }
}

/// Añade `extra` al campo de texto `key` de un bloque, si el bloque ya existe.
fn append_str(block: &mut serde_json::Value, key: &str, extra: &str) {
    if let Some(serde_json::Value::String(current)) = block.get_mut(key) {
        current.push_str(extra);
    }
}

pub fn map_anthropic_event(event: &str, data: &str, acc: &mut AnthropicAcc) -> Vec<StreamChunk> {
    let mut out = Vec::new();
    let v: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return out,
    };
    match event {
        "content_block_start" => {
            let idx = v["index"].as_u64().unwrap_or(0) as usize;
            while acc.blocks.len() <= idx {
                acc.blocks
                    .push((String::new(), String::new(), String::new()));
            }
            let cb = &v["content_block"];
            match cb["type"].as_str().unwrap_or("") {
                "text" => *acc.content_slot(idx) = serde_json::json!({"type":"text","text":""}),
                "thinking" => {
                    acc.has_thinking = true;
                    *acc.content_slot(idx) =
                        serde_json::json!({"type":"thinking","thinking":"","signature":""});
                }
                "redacted_thinking" => {
                    acc.has_thinking = true;
                    *acc.content_slot(idx) = cb.clone();
                }
                "tool_use" => {
                    *acc.content_slot(idx) = serde_json::json!({
                        "type":"tool_use","id":cb["id"],"name":cb["name"],"input":{}
                    });
                }
                _ => {}
            }
            if cb["type"] == "tool_use" {
                acc.blocks[idx].0 = cb["id"].as_str().unwrap_or_default().to_string();
                acc.blocks[idx].1 = cb["name"].as_str().unwrap_or_default().to_string();
            }
        }
        "content_block_delta" => {
            let idx = v["index"].as_u64().unwrap_or(0) as usize;
            let d = &v["delta"];
            match d["type"].as_str().unwrap_or("") {
                "text_delta" => {
                    let text = d["text"].as_str().unwrap_or_default();
                    append_str(acc.content_slot(idx), "text", text);
                    out.push(StreamChunk::TextDelta(text.to_string()));
                }
                "thinking_delta" => {
                    let thinking = d["thinking"].as_str().unwrap_or_default();
                    append_str(acc.content_slot(idx), "thinking", thinking);
                    out.push(StreamChunk::ReasoningDelta(thinking.to_string()));
                }
                "signature_delta" => append_str(
                    acc.content_slot(idx),
                    "signature",
                    d["signature"].as_str().unwrap_or_default(),
                ),
                "input_json_delta" => {
                    while acc.blocks.len() <= idx {
                        acc.blocks
                            .push((String::new(), String::new(), String::new()));
                    }
                    acc.blocks[idx]
                        .2
                        .push_str(d["partial_json"].as_str().unwrap_or_default());
                }
                _ => {}
            }
        }
        "message_start" => {
            if let Some(u) = v.pointer("/message/usage") {
                out.push(StreamChunk::UsageUpdate(super::UsageUpdate {
                    input_tokens: u["input_tokens"].as_u64().map(|value| value as u32),
                    output_tokens: u["output_tokens"].as_u64().map(|value| value as u32),
                    reported_cost_usd: u["cost"]
                        .as_f64()
                        .filter(|cost| cost.is_finite() && *cost >= 0.0),
                    final_update: false,
                }));
            }
        }
        "message_delta" => {
            if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                out.push(StreamChunk::UsageUpdate(super::UsageUpdate {
                    input_tokens: None,
                    output_tokens: u["output_tokens"].as_u64().map(|value| value as u32),
                    reported_cost_usd: u["cost"]
                        .as_f64()
                        .filter(|cost| cost.is_finite() && *cost >= 0.0),
                    final_update: true,
                }));
            }
            out.extend(acc.finish());
            let reason = match v
                .pointer("/delta/stop_reason")
                .and_then(|x| x.as_str())
                .unwrap_or("")
            {
                "tool_use" => FinishReason::ToolCalls,
                "max_tokens" => FinishReason::Length,
                _ => FinishReason::Stop,
            };
            out.push(StreamChunk::Done(reason));
        }
        _ => {}
    }
    out
}

/// Traduce un nivel de esfuerzo al presupuesto de razonamiento de Anthropic.
/// Cabe señalar que esta tabla es una suposición pendiente de contrastar con el
/// servicio real; un nivel fuera de ella no envía nada.
fn anthropic_budget(effort: &str) -> Option<u32> {
    match effort {
        "minimal" => Some(1024),
        "low" => Some(2048),
        "medium" => Some(8192),
        "high" => Some(16384),
        "max" => Some(32000),
        _ => None,
    }
}

fn is_ptc_unsupported(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("code_execution")
        || body.contains("allowed_callers")
        || body.contains("programmatic tool calling")
}

fn is_openrouter_ptc_rejection(status: reqwest::StatusCode, body: &str) -> bool {
    status == reqwest::StatusCode::BAD_REQUEST && is_ptc_unsupported(body)
}

fn http_error(status: reqwest::StatusCode, body: &str) -> String {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.pointer("/error/message")?.as_str().map(str::to_owned));
    match message.filter(|message| !message.trim().is_empty()) {
        Some(message) => format!(
            "Anthropic API returned HTTP {}: {}",
            status.as_u16(),
            message.chars().take(512).collect::<String>()
        ),
        None => format!("Anthropic API returned HTTP {}", status.as_u16()),
    }
}

pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    capability_cache: Mutex<HashMap<String, bool>>,
}

impl AnthropicProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            capability_cache: Mutex::new(HashMap::new()),
        }
    }

    fn is_openrouter(&self) -> bool {
        reqwest::Url::parse(&self.base_url)
            .is_ok_and(|url| url.host_str() == Some("openrouter.ai") && url.path() == "/api")
    }

    fn is_openrouter_haiku_55(&self, model: &str) -> bool {
        self.is_openrouter() && model == OPENROUTER_HAIKU_55
    }

    fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
        let messages = wire_messages(&req.messages, &req.model, &self.base_url);
        let mut system = req.system.clone();
        for message in req.messages.iter().filter(|message| {
            message.role == crate::message::Role::System && !message.text().is_empty()
        }) {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(&message.text());
        }
        let mut tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                let mut tool = serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema
                });
                if req.programmatic_tool_calling && t.programmatic_allowed {
                    tool["allowed_callers"] =
                        serde_json::json!(["direct", "code_execution_20260120"]);
                }
                tool
            })
            .collect();
        if req.programmatic_tool_calling && req.tools.iter().any(|tool| tool.programmatic_allowed) {
            tools.push(serde_json::json!({
                "type": "code_execution_20260521",
                "name": "code_execution"
            }));
        }
        let mut cuerpo = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
            "stream": !req.programmatic_tool_calling,
            "temperature": req.temperature
        });
        if self.is_openrouter_haiku_55(&req.model) {
            cuerpo.as_object_mut().unwrap().remove("temperature");
            cuerpo["provider"] = serde_json::json!({
                "order": ["anthropic"],
                "allow_fallbacks": false
            });
        // Sin esfuerzo (o con un nivel fuera de la tabla), el cuerpo no cambia.
        } else if let Some(presupuesto) = req.effort.as_deref().and_then(anthropic_budget) {
            cuerpo["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": presupuesto
            });
            // La respuesta visible va detrás del razonamiento: max_tokens debe superar
            // el presupuesto, así que se amplía en vez de recortar el presupuesto.
            cuerpo["max_tokens"] =
                serde_json::json!(presupuesto.saturating_add(req.max_tokens.max(1)));
            // Con `thinking` la API no admite una temperatura personalizada: se omite.
            if let Some(obj) = cuerpo.as_object_mut() {
                obj.remove("temperature");
            }
        }
        if let Some(container) = req.messages.iter().rev().find_map(|message| {
            message
                .provider_state
                .as_ref()
                .filter(|state| {
                    state.provider == "anthropic"
                        && state.data["model"] == req.model
                        && state.data["base_url"] == self.base_url
                })
                .and_then(|state| state.data.get("container_id"))
                .and_then(serde_json::Value::as_str)
        }) {
            cuerpo["container"] = serde_json::json!({ "id": container });
        }
        cuerpo
    }

    async fn supports_code_execution(
        &self,
        model: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<bool>> {
        // OpenRouter's public model record does not expose Anthropic's PTC capability field.
        // Try only the documented Haiku 5.5 model and pin its upstream to Anthropic.
        if self.is_openrouter_haiku_55(model) {
            return Ok(Some(true));
        }
        if let Ok(cache) = self.capability_cache.lock() {
            if let Some(supported) = cache.get(model).copied() {
                return Ok(Some(supported));
            }
        }
        let Ok(mut url) = reqwest::Url::parse(&format!("{}/v1/models/", self.base_url)) else {
            return Ok(None);
        };
        let Ok(mut segments) = url.path_segments_mut() else {
            return Ok(None);
        };
        segments.pop_if_empty().push(model);
        drop(segments);
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            response = self.client.get(url)
                .timeout(std::time::Duration::from_secs(10))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .send() => response,
        };
        let Ok(response) = response else {
            return Ok(None);
        };
        if !response.status().is_success() {
            return Ok(None);
        }
        let Ok(value) = response.json::<serde_json::Value>().await else {
            return Ok(None);
        };
        let Some(supported) = value
            .pointer("/capabilities/code_execution/supported")
            .and_then(serde_json::Value::as_bool)
        else {
            return Ok(None);
        };
        if let Ok(mut cache) = self.capability_cache.lock() {
            cache.insert(model.to_string(), supported);
        }
        Ok(Some(supported))
    }

    async fn stream_programmatic(
        &self,
        req: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        let url = format!("{}/v1/messages", self.base_url);
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            response = self.client.post(url)
                .timeout(std::time::Duration::from_secs(120))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&self.build_body(&req))
                .send() => response.map_err(|error| {
                    CoreError::Provider(if error.is_timeout() {
                        "Anthropic request timed out before response headers".into()
                    } else if error.is_connect() {
                        "Anthropic could not connect before response headers".into()
                    } else {
                        "Anthropic request failed before response headers".into()
                    })
                })?,
        };
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if self.is_openrouter_haiku_55(&req.model) && is_openrouter_ptc_rejection(status, &body)
            {
                return Err(CoreError::Provider(OPENROUTER_PTC_UNSUPPORTED.into()));
            }
            return Err(CoreError::Provider(http_error(status, &body)));
        }
        let response: serde_json::Value = response
            .json()
            .await
            .map_err(|error| CoreError::Provider(format!("invalid Anthropic response: {error}")))?;
        let content = response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .ok_or_else(|| CoreError::Provider("Anthropic response has no content".into()))?;
        if serde_json::to_vec(&content).map_or(true, |bytes| bytes.len() > 64 * 1024) {
            return Err(CoreError::Provider(
                "Anthropic continuation state exceeds 64 KiB".into(),
            ));
        }
        let has_pending_programmatic_call = content.iter().any(|block| {
            block["type"] == "tool_use"
                && block
                    .pointer("/caller/type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|caller| caller.starts_with("code_execution_"))
        });
        if has_pending_programmatic_call && response.pointer("/container/id").is_none() {
            return Err(CoreError::Provider(
                "Anthropic paused programmatic calls without a container ID".into(),
            ));
        }
        let state = crate::message::ProviderState {
            provider: "anthropic".into(),
            data: serde_json::json!({
                "model": req.model,
                "base_url": self.base_url,
                "container_id": response.pointer("/container/id"),
                "assistant_content": content,
            }),
        };
        let mut chunks = vec![StreamChunk::Started];
        for block in &content {
            match block.get("type").and_then(serde_json::Value::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(serde_json::Value::as_str) {
                        chunks.push(StreamChunk::TextDelta(text.to_string()));
                    }
                }
                Some("tool_use") => {
                    if let (Some(id), Some(name)) = (
                        block.get("id").and_then(serde_json::Value::as_str),
                        block.get("name").and_then(serde_json::Value::as_str),
                    ) {
                        let caller = block
                            .pointer("/caller/type")
                            .and_then(serde_json::Value::as_str);
                        match caller {
                            Some("code_execution_20260120" | "code_execution_20260521")
                                if !req
                                    .tools
                                    .iter()
                                    .any(|tool| tool.name == name && tool.programmatic_allowed)
                                    || matches!(
                                        caller,
                                        Some("code_execution_20260120" | "code_execution_20260521")
                                    ) && block
                                        .pointer("/caller/tool_id")
                                        .and_then(serde_json::Value::as_str)
                                        .is_none_or(str::is_empty) =>
                            {
                                return Err(CoreError::Provider(format!(
                                    "Anthropic returned a programmatic call for ineligible tool `{name}`"
                                )));
                            }
                            Some(
                                "direct" | "code_execution_20260120" | "code_execution_20260521",
                            )
                            | None => {}
                            Some(_) => {
                                return Err(CoreError::Provider(
                                    "Anthropic returned a tool call with an unknown caller".into(),
                                ));
                            }
                        }
                        if matches!(
                            caller,
                            Some("code_execution_20260120" | "code_execution_20260521")
                        ) {
                            chunks.push(StreamChunk::ProviderNotice(format!(
                                "Anthropic programmatic call {id} → {name}"
                            )));
                        }
                        chunks.push(StreamChunk::ToolCallComplete {
                            id: id.to_string(),
                            name: name.to_string(),
                            input: block
                                .get("input")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null),
                        });
                    }
                }
                _ => {}
            }
        }
        if let Some(usage) = response.get("usage") {
            chunks.push(StreamChunk::Usage(crate::message::Usage {
                input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
                reported_cost_usd: usage["cost"]
                    .as_f64()
                    .filter(|cost| cost.is_finite() && *cost >= 0.0),
            }));
        }
        chunks.push(StreamChunk::ProviderState(state));
        let finish = if response["stop_reason"] == "tool_use" {
            FinishReason::ToolCalls
        } else {
            FinishReason::Stop
        };
        chunks.push(StreamChunk::Done(finish));
        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}

fn role_str(r: crate::message::Role) -> &'static str {
    use crate::message::Role::{Assistant, System, Tool, User};
    match r {
        System => "system",
        User => "user",
        Assistant => "assistant",
        Tool => "user",
    }
}

/// Traduce los mensajes al formato Messages: bloques `tool_use` en el
/// asistente y `tool_result` en un mensaje de usuario (uno por ToolResult).
fn wire_messages(
    messages: &[crate::message::Message],
    model: &str,
    base_url: &str,
) -> Vec<serde_json::Value> {
    use crate::message::{ContentPart, Role};
    let mut out = Vec::new();
    for m in messages {
        match m.role {
            Role::System => {}
            Role::Assistant => {
                if let Some(content) = m
                    .provider_state
                    .as_ref()
                    .filter(|state| {
                        state.provider == "anthropic"
                            && state.data["model"] == model
                            && state.data["base_url"] == base_url
                    })
                    .and_then(|state| state.data.get("assistant_content"))
                {
                    out.push(serde_json::json!({ "role": "assistant", "content": content }));
                    continue;
                }
                let mut blocks: Vec<serde_json::Value> = Vec::new();
                let text = m.text();
                if !text.is_empty() {
                    blocks.push(serde_json::json!({ "type": "text", "text": text }));
                }
                for p in &m.parts {
                    if let ContentPart::ToolCall { id, name, input } = p {
                        blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }));
                    }
                }
                if blocks.is_empty() {
                    blocks.push(serde_json::json!({ "type": "text", "text": "" }));
                }
                out.push(serde_json::json!({ "role": "assistant", "content": blocks }));
            }
            Role::Tool => {
                let mut blocks: Vec<serde_json::Value> = Vec::new();
                for p in &m.parts {
                    if let ContentPart::ToolResult {
                        call_id,
                        content,
                        is_error,
                    } = p
                    {
                        blocks.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": call_id,
                            "content": crate::tools::limit_output(content),
                            "is_error": is_error,
                        }));
                    }
                }
                if !blocks.is_empty() {
                    out.push(serde_json::json!({ "role": "user", "content": blocks }));
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
                                ContentPart::Image {
                                    mime_type, data, ..
                                } => Some(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": mime_type,
                                        "data": data
                                    }
                                })),
                                ContentPart::ImageUrl { url } => Some(serde_json::json!({
                                    "type": "image",
                                    "source": { "type": "url", "url": url }
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
impl Provider for AnthropicProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: true,
            streaming: true,
        }
    }

    async fn stream(
        &self,
        mut req: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        let mut fallback_notice = None;
        if req.programmatic_tool_calling && req.tools.iter().any(|tool| tool.programmatic_allowed) {
            match self.supports_code_execution(&req.model, &cancel).await? {
                Some(true) => match self.stream_programmatic(req.clone(), cancel.clone()).await {
                    Ok(stream) => return Ok(stream),
                    Err(CoreError::Provider(message)) if message == OPENROUTER_PTC_UNSUPPORTED => {
                        fallback_notice = Some(format!(
                            "OpenRouter rejected programmatic tools for `{}`; using direct calls",
                            req.model
                        ));
                    }
                    Err(error) => return Err(error),
                },
                Some(false) => {
                    tracing::info!(model = %req.model, "Anthropic model lacks code_execution; using direct tool calls");
                    fallback_notice = Some(format!(
                        "Anthropic model `{}` does not support code execution; using direct tool calls",
                        req.model
                    ));
                }
                None => {
                    tracing::info!(model = %req.model, "Anthropic code_execution capability is unknown; using direct tool calls");
                    fallback_notice = Some(format!(
                        "Could not verify code execution for Anthropic model `{}`; using direct tool calls",
                        req.model
                    ));
                }
            }
        }
        if fallback_notice.is_some() || !req.tools.iter().any(|tool| tool.programmatic_allowed) {
            req.programmatic_tool_calling = false;
        }
        let url = format!("{}/v1/messages", self.base_url);
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            response = self.client.post(url)
                .timeout(std::time::Duration::from_secs(120))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&self.build_body(&req))
                .send() => response.map_err(|error| {
                    CoreError::Provider(if error.is_timeout() {
                        "Anthropic request timed out before response headers".into()
                    } else if error.is_connect() {
                        "Anthropic could not connect before response headers".into()
                    } else {
                        "Anthropic request failed before response headers".into()
                    })
                })?,
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Provider(http_error(status, &body)));
        }

        struct State<S> {
            stream: S,
            sse: SseBuffer,
            acc: AnthropicAcc,
            queue: std::collections::VecDeque<StreamChunk>,
            cancel: CancellationToken,
            done: bool,
        }

        let init = State {
            stream: resp.bytes_stream(),
            sse: SseBuffer::default(),
            acc: AnthropicAcc::for_provider(&req.model, &self.base_url),
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
                        for (name, data) in st.sse.push_events(&text) {
                            st.queue
                                .extend(map_anthropic_event(&name, &data, &mut st.acc));
                        }
                    }
                    Some(Err(error)) => {
                        st.done = true;
                        st.queue
                            .push_back(StreamChunk::Error(if error.is_timeout() {
                                "Anthropic response stream timed out".into()
                            } else {
                                "Anthropic response stream failed".into()
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
        if let Some(message) = fallback_notice {
            Ok(Box::pin(
                futures::stream::once(async move { StreamChunk::ProviderNotice(message) })
                    .chain(stream),
            ))
        } else {
            Ok(stream)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(name: &str, v: serde_json::Value) -> Vec<StreamChunk> {
        let mut acc = AnthropicAcc::default();
        map_anthropic_event(name, &v.to_string(), &mut acc)
    }

    #[test]
    fn maps_text_delta() {
        let chunks = ev(
            "content_block_delta",
            json!({"index":0,"delta":{"type":"text_delta","text":"hi"}}),
        );
        assert!(matches!(chunks.as_slice(), [StreamChunk::TextDelta(t)] if t == "hi"));
    }

    #[test]
    fn encodes_local_image_parts_as_anthropic_base64_blocks() {
        use crate::message::{ContentPart, Message};
        let mut message = Message::human("What is this?");
        message.parts.push(ContentPart::Image {
            mime_type: "image/png".into(),
            data: "cG5n".into(),
            filename: "sample.png".into(),
        });
        let wire = super::wire_messages(&[message], "model", "https://api.example");
        assert_eq!(wire[0]["content"][0]["text"], "What is this?");
        assert_eq!(wire[0]["content"][1]["source"]["type"], "base64");
        assert_eq!(wire[0]["content"][1]["source"]["media_type"], "image/png");
        assert_eq!(wire[0]["content"][1]["source"]["data"], "cG5n");
    }

    #[test]
    fn maps_provider_reported_cost_from_usage() {
        let chunks = ev(
            "message_delta",
            json!({"usage":{"output_tokens":2,"cost":0.002}}),
        );
        assert!(
            matches!(&chunks[0], StreamChunk::UsageUpdate(usage) if usage.reported_cost_usd == Some(0.002) && usage.final_update)
        );
    }

    #[test]
    fn assembles_tool_use() {
        let mut acc = AnthropicAcc::default();
        let start = json!({"index":0,"content_block":{"type":"tool_use","id":"t1","name":"view"}});
        let d1 = json!({"index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}});
        let d2 = json!({"index":0,"delta":{"type":"input_json_delta","partial_json":"\"a.rs\"}"}});
        map_anthropic_event("content_block_start", &start.to_string(), &mut acc);
        map_anthropic_event("content_block_delta", &d1.to_string(), &mut acc);
        map_anthropic_event("content_block_delta", &d2.to_string(), &mut acc);
        let stop = json!({"delta":{"stop_reason":"tool_use"}});
        let chunks = map_anthropic_event("message_delta", &stop.to_string(), &mut acc);
        assert!(
            matches!(chunks.first(), Some(StreamChunk::ToolCallComplete { name, input, .. })
            if name == "view" && input["path"] == "a.rs")
        );
        assert!(matches!(
            chunks.last(),
            Some(StreamChunk::Done(FinishReason::ToolCalls))
        ));
    }

    #[test]
    fn maps_message_start_usage() {
        let chunks = ev(
            "message_start",
            json!({"message":{"usage":{"input_tokens":12,"output_tokens":0}}}),
        );
        assert!(
            matches!(chunks.as_slice(), [StreamChunk::UsageUpdate(u)] if u.input_tokens == Some(12) && !u.final_update)
        );
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::message::{ContentPart, Message, Role};
    use serde_json::json;

    #[test]
    fn serializes_tool_use_and_tool_result_blocks() {
        let p = AnthropicProvider::new("http://x", "k");
        let req = ProviderRequest {
            model: "m".into(),
            system: "sys".into(),
            messages: vec![
                Message::assistant(vec![ContentPart::ToolCall {
                    id: "t1".into(),
                    name: "view".into(),
                    input: json!({"path":"a"}),
                }]),
                Message::new(
                    Role::Tool,
                    vec![ContentPart::ToolResult {
                        call_id: "t1".into(),
                        content: "ok".into(),
                        is_error: false,
                    }],
                ),
            ],
            tools: vec![],
            temperature: 0.0,
            max_tokens: 16,
            effort: None,
            programmatic_tool_calling: false,
        };
        let body = p.build_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        let assistant = msgs.iter().find(|m| m["role"] == "assistant").unwrap();
        let blocks = assistant["content"].as_array().unwrap();
        assert!(blocks
            .iter()
            .any(|b| b["type"] == "tool_use" && b["id"] == "t1" && b["name"] == "view"));
        let user = msgs
            .iter()
            .find(|m| {
                m["role"] == "user"
                    && m["content"]
                        .as_array()
                        .map(|a| a.iter().any(|b| b["type"] == "tool_result"))
                        .unwrap_or(false)
            })
            .unwrap();
        let result = user["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["type"] == "tool_result")
            .unwrap();
        assert_eq!(result["tool_use_id"], "t1");
        assert_eq!(result["content"], "ok");
    }

    #[test]
    fn moves_system_history_into_the_top_level_system_field() {
        let p = AnthropicProvider::new("http://x", "k");
        let mut req = ProviderRequest {
            model: "m".into(),
            system: "base instructions".into(),
            messages: vec![
                Message::new(
                    Role::System,
                    vec![ContentPart::Text {
                        text: "later instruction".into(),
                    }],
                ),
                Message::user("hello"),
            ],
            tools: vec![],
            temperature: 0.0,
            max_tokens: 16,
            effort: None,
            programmatic_tool_calling: false,
        };
        let body = p.build_body(&req);
        assert_eq!(body["system"], "base instructions\n\nlater instruction");
        assert_eq!(
            body["messages"],
            json!([{ "role": "user", "content": "hello" }])
        );
        req.system.clear();
        assert_eq!(p.build_body(&req)["system"], "later instruction");
    }

    #[test]
    fn anthropic_http_error_includes_a_bounded_provider_message() {
        let message = http_error(
            reqwest::StatusCode::BAD_REQUEST,
            &json!({"error":{"message":"invalid messages[0].role"}}).to_string(),
        );
        assert!(message.contains("HTTP 400: invalid messages[0].role"));
        let long = "x".repeat(600);
        assert!(
            http_error(
                reqwest::StatusCode::BAD_REQUEST,
                &json!({"error":{"message":long}}).to_string()
            )
            .chars()
            .count()
                < 560
        );
    }
}

#[cfg(test)]
mod effort_tests {
    use super::*;
    use crate::provider::ToolSpec;
    use serde_json::json;

    /// Petición de prueba con una herramienta, para que el cuerpo sea representativo.
    fn peticion(effort: Option<&str>, max_tokens: u32) -> ProviderRequest {
        ProviderRequest {
            model: "m".into(),
            system: "sys".into(),
            messages: vec![],
            tools: vec![ToolSpec {
                name: "view".into(),
                description: "Ve un fichero".into(),
                input_schema: json!({ "type": "object" }),
                programmatic_allowed: true,
            }],
            temperature: 0.5,
            max_tokens,
            effort: effort.map(String::from),
            programmatic_tool_calling: false,
        }
    }

    #[test]
    fn anthropic_body_without_effort_is_unchanged() {
        // Cuerpo exacto que enviaba `build_body` antes de existir el esfuerzo.
        let previo = json!({
            "model": "m",
            "max_tokens": 16,
            "system": "sys",
            "messages": [],
            "tools": [{
                "name": "view",
                "description": "Ve un fichero",
                "input_schema": { "type": "object" }
            }],
            "stream": true,
            "temperature": 0.5
        });
        let p = AnthropicProvider::new("http://x", "k");
        assert_eq!(p.build_body(&peticion(None, 16)), previo);
    }

    #[test]
    fn anthropic_effort_enables_thinking_and_raises_max_tokens() {
        let p = AnthropicProvider::new("http://x", "k");
        let cuerpo = p.build_body(&peticion(Some("high"), 1000));
        assert_eq!(cuerpo["thinking"]["type"], "enabled");
        assert_eq!(cuerpo["thinking"]["budget_tokens"], 16384);
        let max = cuerpo["max_tokens"].as_u64().unwrap();
        assert!(
            max > 16384,
            "max_tokens ({max}) debe superar el presupuesto"
        );
        assert_eq!(max, 16384 + 1000);
    }

    #[test]
    fn anthropic_effort_omits_temperature() {
        let p = AnthropicProvider::new("http://x", "k");
        let cuerpo = p.build_body(&peticion(Some("low"), 16));
        assert!(cuerpo.get("temperature").is_none());
    }

    #[test]
    fn anthropic_budget_table() {
        assert_eq!(anthropic_budget("minimal"), Some(1024));
        assert_eq!(anthropic_budget("low"), Some(2048));
        assert_eq!(anthropic_budget("medium"), Some(8192));
        assert_eq!(anthropic_budget("high"), Some(16384));
        assert_eq!(anthropic_budget("max"), Some(32000));
        assert_eq!(anthropic_budget("xhigh"), None);
    }

    #[test]
    fn unknown_effort_sends_nothing() {
        // Un nivel fuera de la tabla deja la petición exactamente como la de «default».
        let p = AnthropicProvider::new("http://x", "k");
        assert_eq!(
            p.build_body(&peticion(Some("turbo"), 16)),
            p.build_body(&peticion(None, 16))
        );
    }
}

#[cfg(test)]
mod programmatic_tests {
    use super::*;
    use crate::message::Usage;
    use crate::provider::ToolSpec;
    use serde_json::json;

    fn request() -> ProviderRequest {
        ProviderRequest {
            model: "claude-test".into(),
            system: "system".into(),
            messages: vec![],
            tools: vec![
                ToolSpec {
                    name: "view".into(),
                    description: "read a file".into(),
                    input_schema: json!({"type":"object"}),
                    programmatic_allowed: true,
                },
                ToolSpec {
                    name: "write".into(),
                    description: "write a file".into(),
                    input_schema: json!({"type":"object"}),
                    programmatic_allowed: false,
                },
            ],
            temperature: 0.0,
            max_tokens: 128,
            effort: None,
            programmatic_tool_calling: true,
        }
    }

    #[tokio::test]
    async fn stream_error_preserves_anthropic_400_message() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error":{"message":"messages.0.content: invalid"}
            })))
            .mount(&server)
            .await;
        let provider = AnthropicProvider::new(server.uri(), "test-key");
        let mut request = request();
        request.programmatic_tool_calling = false;
        let error = match provider.stream(request, CancellationToken::new()).await {
            Err(error) => error.to_string(),
            Ok(_) => panic!("400 response must be an error"),
        };
        assert!(error.contains("HTTP 400: messages.0.content: invalid"));
    }

    #[tokio::test]
    #[ignore = "live OpenRouter calls; incurs provider usage"]
    async fn openrouter_haiku_live_direct_vs_ptc() {
        use crate::message::{ContentPart, Message, Role};
        use futures::StreamExt;
        use std::time::Instant;

        let key = std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY");
        let provider = AnthropicProvider::new("https://openrouter.ai/api", key);
        let mut total_cost = 0.0_f64;
        for ptc in [false, true] {
            let started = Instant::now();
            let mut req = request();
            req.model = OPENROUTER_HAIKU_55.into();
            req.system = "Read the fixture with the view tool and return only its VALUE.".into();
            req.messages = vec![Message::user(
                "Use view exactly once to read the local fixture.",
            )];
            req.max_tokens = 128;
            req.programmatic_tool_calling = ptc;
            let mut turns = 0;
            let mut usage = Usage {
                input_tokens: 0,
                output_tokens: 0,
                reported_cost_usd: None,
            };
            let mut called_programmatically = false;
            let mut final_text = String::new();
            let mut provider_rejected = false;
            for _ in 0..2 {
                turns += 1;
                let mut stream = match provider.stream(req.clone(), CancellationToken::new()).await
                {
                    Ok(stream) => stream,
                    Err(_) => {
                        provider_rejected = true;
                        break;
                    }
                };
                let mut parts = Vec::new();
                let mut state = None;
                let mut tool_call = None;
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        StreamChunk::TextDelta(text) => {
                            final_text.push_str(&text);
                            parts.push(ContentPart::Text { text });
                        }
                        StreamChunk::ToolCallComplete { id, name, input } => {
                            tool_call = Some((id.clone(), name.clone(), input.clone()));
                            parts.push(ContentPart::ToolCall { id, name, input });
                        }
                        StreamChunk::Usage(u) => {
                            usage.input_tokens = usage.input_tokens.max(u.input_tokens);
                            usage.output_tokens = usage.output_tokens.max(u.output_tokens);
                        }
                        StreamChunk::UsageUpdate(u) => {
                            if let Some(input) = u.input_tokens {
                                usage.input_tokens = usage.input_tokens.max(input);
                            }
                            if let Some(output) = u.output_tokens {
                                usage.output_tokens = usage.output_tokens.max(output);
                            }
                        }
                        StreamChunk::ProviderState(s) => state = Some(s),
                        StreamChunk::ProviderNotice(n) if n.contains("programmatic call") => {
                            called_programmatically = true
                        }
                        _ => {}
                    }
                }
                let Some((id, name, _)) = tool_call else {
                    break;
                };
                assert_eq!(name, "view");
                let mut assistant = Message::assistant(parts);
                assistant.provider_state = state;
                req.messages.push(assistant);
                req.messages.push(Message::new(
                    Role::Tool,
                    vec![ContentPart::ToolResult {
                        call_id: id,
                        content: "VALUE=cedar-7".into(),
                        is_error: false,
                    }],
                ));
            }
            let cost = usage.input_tokens as f64 * 0.10 / 1_000_000.0
                + usage.output_tokens as f64 * 0.50 / 1_000_000.0;
            total_cost += cost;
            println!("mode={} tokens_in={} tokens_out={} turns={} latency_ms={} tool_ptc={} cost_usd={:.6} exact={} provider_rejected={}",
                if ptc { "ptc" } else { "direct" }, usage.input_tokens, usage.output_tokens,
                turns, started.elapsed().as_millis(), called_programmatically, cost,
                final_text.trim() == "cedar-7", provider_rejected);
            assert!(total_cost < 1.0, "USD 1 benchmark cap reached");
        }
    }

    #[test]
    fn body_enables_code_execution_only_when_a_tool_is_eligible() {
        let provider = AnthropicProvider::new("http://localhost", "test-key");
        let body = provider.build_body(&request());
        assert!(body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["type"] == "code_execution_20260521"));
        assert_eq!(
            body["tools"][0]["allowed_callers"],
            json!(["direct", "code_execution_20260120"])
        );

        let mut direct = request();
        direct
            .tools
            .iter_mut()
            .for_each(|tool| tool.programmatic_allowed = false);
        assert!(
            AnthropicProvider::new("http://localhost", "test-key").build_body(&direct)["tools"]
                .as_array()
                .unwrap()
                .iter()
                .all(|tool| tool["type"] != "code_execution_20260521")
        );
    }

    #[test]
    fn openrouter_haiku_ptc_is_pinned_and_direct_anthropic_body_is_unchanged() {
        let mut req = request();
        req.model = OPENROUTER_HAIKU_55.into();
        let openrouter = AnthropicProvider::new("https://openrouter.ai/api", "key");
        let body = openrouter.build_body(&req);
        assert_eq!(body["provider"]["order"], json!(["anthropic"]));
        assert_eq!(body["provider"]["allow_fallbacks"], false);
        assert!(body.get("temperature").is_none());
        assert!(body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["type"] == "code_execution_20260521"));

        let direct = AnthropicProvider::new("https://api.anthropic.com", "key").build_body(&req);
        assert!(direct.get("provider").is_none());
    }

    #[test]
    fn openrouter_falls_back_only_on_explicit_bad_request_about_ptc() {
        use reqwest::StatusCode;

        assert!(is_openrouter_ptc_rejection(
            StatusCode::BAD_REQUEST,
            "code_execution is unsupported"
        ));
        assert!(!is_openrouter_ptc_rejection(
            StatusCode::BAD_REQUEST,
            "invalid model"
        ));
        assert!(!is_openrouter_ptc_rejection(
            StatusCode::INTERNAL_SERVER_ERROR,
            "code_execution unavailable"
        ));
        assert!(!is_openrouter_ptc_rejection(
            StatusCode::UNAUTHORIZED,
            "code_execution unsupported"
        ));
    }

    #[test]
    fn programmatic_continuation_replays_provider_blocks_and_container_in_memory() {
        use crate::message::{ContentPart, Message, ProviderState, Role};
        let provider = AnthropicProvider::new("http://localhost", "test-key");
        let mut assistant = Message::assistant(vec![ContentPart::ToolCall {
            id: "tool-1".into(),
            name: "view".into(),
            input: json!({"path":"README.md"}),
        }]);
        let content = json!([
            {"type":"server_tool_use","id":"exec-1","name":"code_execution","input":{"code":"x = 1"}},
            {"type":"tool_use","id":"tool-1","name":"view","input":{"path":"README.md"},
                "caller":{"type":"code_execution_20260120","tool_id":"exec-1"}}
        ]);
        assistant.provider_state = Some(ProviderState {
            provider: "anthropic".into(),
            data: json!({"model":"claude-test","base_url":"http://localhost",
                "container_id":"container-1","assistant_content":content.clone()}),
        });
        let result = Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "tool-1".into(),
                content: "file contents".into(),
                is_error: false,
            }],
        );
        let mut req = request();
        req.messages = vec![assistant, result];
        let body = provider.build_body(&req);
        assert_eq!(body["container"]["id"], "container-1");
        assert_eq!(body["messages"][0]["content"], content);
        assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "tool-1");
    }

    #[tokio::test]
    async fn rejects_programmatic_calls_to_ineligible_tools() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models/claude-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "capabilities":{"code_execution":{"supported":true}}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "stop_reason":"tool_use","container":{"id":"container-1"},
                "content":[{"type":"tool_use","id":"call-1","name":"write",
                    "input":{},"caller":{"type":"code_execution_20260120","tool_id":"exec-1"}}]
            })))
            .mount(&server)
            .await;
        let provider = AnthropicProvider::new(server.uri(), "test-key");
        assert!(provider
            .stream(request(), CancellationToken::new())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn unverified_model_uses_direct_calls_and_reports_fallback() {
        use futures::StreamExt;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models/claude-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "capabilities":{"code_execution":{"supported":false}}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "data: {\"type\":\"message_start\",\"message\":{\"usage\":{}}}\n\n",
                "text/event-stream",
            ))
            .mount(&server)
            .await;
        let provider = AnthropicProvider::new(server.uri(), "test-key");
        let mut stream = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap();
        let mut notice = false;
        while let Some(chunk) = stream.next().await {
            if matches!(chunk, StreamChunk::ProviderNotice(ref text) if text.contains("using direct tool calls"))
            {
                notice = true;
            }
        }
        assert!(notice);
        let sent = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&sent[1].body).unwrap();
        assert!(body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "code_execution_20260521"));
    }

    #[tokio::test]
    async fn expired_programmatic_container_is_not_replayed_as_direct_calls() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models/claude-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "capabilities":{"code_execution":{"supported":true}}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error":{"message":"container expired"}
            })))
            .mount(&server)
            .await;

        let provider = AnthropicProvider::new(server.uri(), "test-key");
        assert!(provider
            .stream(request(), CancellationToken::new())
            .await
            .is_err());
        let sent = server.received_requests().await.unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(
            sent.iter()
                .filter(|request| request.method == "POST")
                .count(),
            1
        );
    }
}

#[cfg(test)]
mod thinking_replay_tests {
    use super::*;
    use serde_json::json;

    fn feed(acc: &mut AnthropicAcc, events: &[(&str, serde_json::Value)]) -> Vec<StreamChunk> {
        events
            .iter()
            .flat_map(|(name, data)| map_anthropic_event(name, &data.to_string(), acc))
            .collect()
    }

    /// Respuesta en streaming con razonamiento y una llamada a herramienta.
    fn thinking_then_tool_use() -> Vec<(&'static str, serde_json::Value)> {
        vec![
            (
                "content_block_start",
                json!({"index":0,"content_block":{"type":"thinking","thinking":""}}),
            ),
            (
                "content_block_delta",
                json!({"index":0,"delta":{"type":"thinking_delta","thinking":"Voy a "}}),
            ),
            (
                "content_block_delta",
                json!({"index":0,"delta":{"type":"thinking_delta","thinking":"mirar."}}),
            ),
            (
                "content_block_delta",
                json!({"index":0,"delta":{"type":"signature_delta","signature":"firma-abc"}}),
            ),
            (
                "content_block_start",
                json!({"index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"view"}}),
            ),
            (
                "content_block_delta",
                json!({"index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.rs\"}"}}),
            ),
            (
                "message_delta",
                json!({"delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":5}}),
            ),
        ]
    }

    #[test]
    fn streamed_thinking_is_kept_so_the_next_request_can_replay_it() {
        // Con `thinking` activo, Messages exige que el turno del asistente con `tool_use`
        // reenvíe su bloque de razonamiento con la firma; de lo contrario responde 400.
        let mut acc = AnthropicAcc::for_provider("claude-test", "https://api.anthropic.com");
        let chunks = feed(&mut acc, &thinking_then_tool_use());
        let state = chunks
            .into_iter()
            .find_map(|chunk| match chunk {
                StreamChunk::ProviderState(state) => Some(state),
                _ => None,
            })
            .expect("falta el estado con el razonamiento firmado");
        assert_eq!(state.provider, "anthropic");
        assert_eq!(state.data["model"], "claude-test");
        assert_eq!(state.data["base_url"], "https://api.anthropic.com");
        assert_eq!(
            state.data["assistant_content"],
            json!([
                {"type":"thinking","thinking":"Voy a mirar.","signature":"firma-abc"},
                {"type":"tool_use","id":"toolu_1","name":"view","input":{"path":"a.rs"}}
            ])
        );
    }

    #[test]
    fn replayed_assistant_turn_starts_with_the_thinking_block() {
        let mut acc = AnthropicAcc::for_provider("claude-test", "https://api.anthropic.com");
        let state = feed(&mut acc, &thinking_then_tool_use())
            .into_iter()
            .find_map(|chunk| match chunk {
                StreamChunk::ProviderState(state) => Some(state),
                _ => None,
            })
            .unwrap();
        let mut assistant = crate::message::Message::assistant(vec![]);
        assistant.provider_state = Some(state);
        let wire = wire_messages(&[assistant], "claude-test", "https://api.anthropic.com");
        assert_eq!(wire[0]["content"][0]["type"], "thinking");
        assert_eq!(wire[0]["content"][0]["signature"], "firma-abc");
    }

    #[test]
    fn without_thinking_no_state_is_emitted() {
        let mut acc = AnthropicAcc::for_provider("claude-test", "https://api.anthropic.com");
        let chunks = feed(
            &mut acc,
            &[
                (
                    "content_block_start",
                    json!({"index":0,"content_block":{"type":"tool_use","id":"t","name":"view"}}),
                ),
                (
                    "content_block_delta",
                    json!({"index":0,"delta":{"type":"input_json_delta","partial_json":"{}"}}),
                ),
                ("message_delta", json!({"delta":{"stop_reason":"tool_use"}})),
            ],
        );
        assert!(!chunks
            .iter()
            .any(|chunk| matches!(chunk, StreamChunk::ProviderState(_))));
    }
}
