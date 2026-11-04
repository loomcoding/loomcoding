//! Proveedor de la API Responses de OpenAI, con llamada programática de herramientas.

use super::*;
use crate::capability::{
    classify_rejection, Capability, CapabilityRegistry, Protocol, State, Target,
};

/// Native Responses API adapter. Kept separate from OpenAI-compatible Chat Completions.
pub struct OpenAiResponsesProvider {
    pub(super) base_url: String,
    pub(super) api_key: String,
    pub(super) client: reqwest::Client,
    /// Lo que se sabe de cada destino: un rechazo real de PTC se recuerda hasta que caduque.
    pub(super) capabilities: CapabilityRegistry,
}

impl OpenAiResponsesProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            capabilities: CapabilityRegistry::default(),
        }
    }

    /// Destino exacto de una petición: el mismo modelo en otra URL es otro destino.
    fn target(&self, model: &str) -> Target {
        Target::new(Protocol::Responses, &self.base_url, model)
    }

    /// `true` si una petición real ya rechazó la llamada programática para este destino.
    fn ptc_rejected(&self, model: &str) -> bool {
        matches!(
            self.capabilities
                .query(&self.target(model), Capability::ProgrammaticToolCalling)
                .state,
            State::Rejected { .. }
        )
    }

    pub(super) fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
        use crate::message::{ContentPart, Role};
        let previous = req
            .messages
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, message)| {
                let state = message.provider_state.as_ref()?;
                (state.provider == "openai_responses"
                    && state.data["model"] == req.model
                    && state.data["base_url"] == self.base_url
                    && state.data["response_id"].is_string())
                .then_some((index, state))
            });
        let (messages, replay_items, callers) = match previous {
            Some((index, state)) => (
                &req.messages[index + 1..],
                state.data["replay_items"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
                state.data["caller_by_call_id"].clone(),
            ),
            None => (&req.messages[..], Vec::new(), serde_json::json!({})),
        };
        let mut input = replay_items;
        for message in messages {
            match message.role {
                Role::Assistant => {
                    if !message.text().is_empty() {
                        input.push(serde_json::json!({
                            "role": "assistant",
                            "content": message.text()
                        }));
                    }
                    for part in &message.parts {
                        if let ContentPart::ToolCall {
                            id,
                            name,
                            input: args,
                        } = part
                        {
                            input.push(serde_json::json!({
                                "type": "function_call",
                                "call_id": id,
                                "name": name,
                                "arguments": args.to_string()
                            }));
                        }
                    }
                }
                Role::Tool => {
                    for part in &message.parts {
                        if let ContentPart::ToolResult {
                            call_id,
                            content,
                            is_error,
                        } = part
                        {
                            let mut output = serde_json::json!({
                                "type": "function_call_output",
                                "call_id": call_id,
                                "output": if *is_error {
                                    format!("Tool error: {}", crate::tools::limit_output(content))
                                } else {
                                    crate::tools::limit_output(content).into_owned()
                                }
                            });
                            if let Some(caller) = callers.get(call_id) {
                                output["caller"] = caller.clone();
                            }
                            input.push(output);
                        }
                    }
                }
                Role::System => input.push(serde_json::json!({
                    "role": "system",
                    "content": message.text()
                })),
                Role::User => input.push(serde_json::json!({
                    "role": "user",
                    "content": message.text()
                })),
            }
        }
        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|tool| {
                let mut value = serde_json::json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                });
                if req.programmatic_tool_calling && tool.programmatic_allowed {
                    value["allowed_callers"] = serde_json::json!(["direct", "programmatic"]);
                }
                value
            })
            .collect();
        let mut body = serde_json::json!({
            "model": req.model,
            "instructions": req.system,
            "input": input,
            "tools": tools,
            "max_output_tokens": req.max_tokens,
            "store": false
        });
        if req.programmatic_tool_calling && req.tools.iter().any(|tool| tool.programmatic_allowed) {
            body["tools"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "type": "programmatic_tool_calling"
                }));
        }
        if let Some(effort) = req.effort.as_deref() {
            body["reasoning"] = serde_json::json!({ "effort": effort });
        } else {
            body["temperature"] = serde_json::json!(req.temperature);
        }
        body
    }

    async fn create_response(
        &self,
        req: &ProviderRequest,
        cancel: &CancellationToken,
    ) -> Result<serde_json::Value> {
        // Un rechazo anterior (todavía vigente) evita repetir en cada turno la petición rechazada.
        let direct_request;
        let mut earlier_notice = None;
        let req = if req.programmatic_tool_calling && self.ptc_rejected(&req.model) {
            direct_request = ProviderRequest {
                programmatic_tool_calling: false,
                ..req.clone()
            };
            earlier_notice = Some(format!(
                "Programmatic tools were rejected earlier for `{}`; using direct calls",
                req.model
            ));
            &direct_request
        } else {
            req
        };
        let mut body = self.build_body(req);
        let mut replay_items = body["input"].as_array().cloned().unwrap_or_default();
        let mut accumulated_output = Vec::new();
        for continuation in 0..4 {
            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                response = self.client.post(format!("{}/responses", self.base_url))
                    .timeout(std::time::Duration::from_secs(120))
                    .bearer_auth(&self.api_key)
                    .json(&body)
                    .send() => response.map_err(|error| CoreError::Provider(error.to_string()))?,
            };
            if !response.status().is_success() {
                let status = response.status();
                let error: serde_json::Value = response.json().await.unwrap_or_default();
                let message = error
                    .pointer("/error/message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if continuation == 0
                    && req.programmatic_tool_calling
                    && body.get("previous_response_id").is_none()
                    && classify_rejection(
                        status.as_u16(),
                        message,
                        Capability::ProgrammaticToolCalling,
                    )
                    .allows_fallback()
                {
                    self.capabilities.record_rejection(
                        &self.target(&req.model),
                        Capability::ProgrammaticToolCalling,
                        status.as_u16(),
                    );
                    let mut direct = req.clone();
                    direct.programmatic_tool_calling = false;
                    let mut response = self.build_body(&direct);
                    response["tools"] = serde_json::Value::Array(
                        req.tools
                            .iter()
                            .map(|tool| {
                                serde_json::json!({
                                    "type": "function",
                                    "name": tool.name,
                                    "description": tool.description,
                                    "parameters": tool.input_schema
                                })
                            })
                            .collect(),
                    );
                    let result = tokio::select! {
                        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                        result = self.client.post(format!("{}/responses", self.base_url))
                            .timeout(std::time::Duration::from_secs(120))
                            .bearer_auth(&self.api_key)
                            .json(&response)
                            .send() => result.map_err(|error| CoreError::Provider(error.to_string()))?,
                    };
                    if !result.status().is_success() {
                        return Err(CoreError::Provider(format!(
                            "OpenAI Responses API http {}",
                            result.status()
                        )));
                    }
                    let value: serde_json::Value = result
                        .json()
                        .await
                        .map_err(|error| CoreError::Provider(error.to_string()))?;
                    let mut replay = replay_items;
                    let output = value["output"].as_array().cloned().unwrap_or_default();
                    replay.extend(output.iter().cloned());
                    accumulated_output.extend(output);
                    let mut value = with_notice(value, format!("OpenAI Responses model `{}` does not support programmatic tools; using direct calls", req.model));
                    value["output"] = serde_json::Value::Array(accumulated_output);
                    return Ok(with_replay(value, replay));
                }
                return Err(CoreError::Provider(format!(
                    "OpenAI Responses API http {status}"
                )));
            }
            let value: serde_json::Value = response.json().await.map_err(|error| {
                CoreError::Provider(format!("invalid Responses API response: {error}"))
            })?;
            let output = value.get("output").and_then(serde_json::Value::as_array);
            let has_function_call = output
                .is_some_and(|items| items.iter().any(|item| item["type"] == "function_call"));
            let has_message =
                output.is_some_and(|items| items.iter().any(|item| item["type"] == "message"));
            let output_items = output.cloned().unwrap_or_default();
            replay_items.extend(output_items.iter().cloned());
            accumulated_output.extend(output_items);
            if has_function_call
                || has_message
                || value["status"] == "incomplete"
                || continuation == 3
            {
                let mut value = value;
                value["output"] = serde_json::Value::Array(accumulated_output);
                if let Some(notice) = earlier_notice {
                    value = with_notice(value, notice);
                }
                return Ok(with_replay(value, replay_items));
            }
            body["input"] = serde_json::Value::Array(replay_items.clone());
        }
        unreachable!()
    }
}

pub(super) fn with_notice(mut response: serde_json::Value, message: String) -> serde_json::Value {
    response["_loomcoding_notice"] = serde_json::Value::String(message);
    response
}

pub(super) fn with_replay(
    mut response: serde_json::Value,
    replay_items: Vec<serde_json::Value>,
) -> serde_json::Value {
    response["_loomcoding_replay_items"] = serde_json::Value::Array(replay_items);
    response
}

#[async_trait]
impl Provider for OpenAiResponsesProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: true,
            streaming: false,
        }
    }

    async fn stream(
        &self,
        req: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        let response = self.create_response(&req, &cancel).await?;
        let output = response
            .get("output")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut chunks = vec![StreamChunk::Started];
        let mut caller_by_call_id = serde_json::Map::new();
        let mut has_calls = false;
        for item in &output {
            match item.get("type").and_then(serde_json::Value::as_str) {
                Some("message") => {
                    if let Some(content) = item.get("content").and_then(serde_json::Value::as_array)
                    {
                        for part in content {
                            if let Some(text) = part.get("text").and_then(serde_json::Value::as_str)
                            {
                                if part["type"] == "output_text" || part["type"] == "refusal" {
                                    chunks.push(StreamChunk::TextDelta(text.to_string()));
                                }
                            }
                        }
                    }
                }
                Some("function_call") => {
                    let id = item["call_id"].as_str().unwrap_or_default();
                    let name = item["name"].as_str().unwrap_or_default();
                    let caller_type = item
                        .pointer("/caller/type")
                        .and_then(serde_json::Value::as_str);
                    match caller_type {
                        Some("program") => {
                            if item
                                .pointer("/caller/caller_id")
                                .and_then(serde_json::Value::as_str)
                                .is_none_or(str::is_empty)
                                || !req
                                    .tools
                                    .iter()
                                    .any(|tool| tool.name == name && tool.programmatic_allowed)
                            {
                                return Err(CoreError::Provider(format!(
                                    "OpenAI returned a programmatic call for ineligible tool `{name}`"
                                )));
                            }
                        }
                        Some("direct") => {}
                        Some(_) => {
                            return Err(CoreError::Provider(
                                "OpenAI Responses returned a function call with an unknown caller"
                                    .into(),
                            ));
                        }
                        None => {}
                    }
                    let arguments = item["arguments"].as_str().unwrap_or("{}");
                    let input = serde_json::from_str(arguments).map_err(|_| {
                        CoreError::Provider(
                            "OpenAI Responses returned invalid function arguments".into(),
                        )
                    })?;
                    if let Some(caller) = item.get("caller") {
                        caller_by_call_id.insert(id.to_string(), caller.clone());
                    }
                    if caller_type == Some("program") {
                        chunks.push(StreamChunk::ProviderNotice(format!(
                            "OpenAI Responses programmatic call {id} → {name}"
                        )));
                    }
                    chunks.push(StreamChunk::ToolCallComplete {
                        id: id.to_string(),
                        name: name.to_string(),
                        input,
                    });
                    has_calls = true;
                }
                _ => {}
            }
        }
        if let Some(usage) = response.get("usage") {
            chunks.push(StreamChunk::Usage(Usage {
                input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
                reported_cost_usd: usage["cost"]
                    .as_f64()
                    .filter(|cost| cost.is_finite() && *cost >= 0.0),
            }));
        }
        if response["_loomcoding_notice"].is_string() {
            chunks.push(StreamChunk::ProviderNotice(
                response["_loomcoding_notice"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ));
        }
        let state = crate::message::ProviderState {
            provider: "openai_responses".into(),
            data: serde_json::json!({
                "response_id": response["id"],
                "model": req.model,
                "base_url": self.base_url,
                "caller_by_call_id": caller_by_call_id,
                "replay_items": response["_loomcoding_replay_items"],
            }),
        };
        if serde_json::to_vec(&state).map_or(true, |bytes| bytes.len() > 64 * 1024) {
            return Err(CoreError::Provider(
                "OpenAI continuation state exceeds 64 KiB".into(),
            ));
        }
        chunks.push(StreamChunk::ProviderState(state));
        chunks.push(StreamChunk::Done(if has_calls {
            FinishReason::ToolCalls
        } else if response["status"] == "incomplete" {
            FinishReason::Length
        } else {
            FinishReason::Stop
        }));
        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}
