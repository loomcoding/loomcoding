use super::sse::SseBuffer;
use super::{Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk};
use crate::error::{CoreError, Result};
use crate::message::{FinishReason, Usage};
use async_trait::async_trait;
use futures::StreamExt;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct ToolCallAcc {
    slots: Vec<(String, String, String)>,
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
fn cache_usage(usage: &serde_json::Value) -> Option<super::CacheUsage> {
    let details = &usage["prompt_tokens_details"];
    let read = details["cached_tokens"]
        .as_u64()
        .or_else(|| usage["prompt_cache_hit_tokens"].as_u64());
    let write = details["cache_write_tokens"].as_u64();
    if read.is_none() && write.is_none() {
        return None;
    }
    Some(super::CacheUsage {
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
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    request_timeout: Duration,
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

    fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
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

fn role_str(r: crate::message::Role) -> &'static str {
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
fn wire_messages(messages: &[crate::message::Message]) -> Vec<serde_json::Value> {
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
            return Err(CoreError::Provider(format!("http {status}")));
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

/// Native Responses API adapter. Kept separate from OpenAI-compatible Chat Completions.
pub struct OpenAiResponsesProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiResponsesProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }

    fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
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
                    && is_programmatic_not_supported(message)
                {
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
                return Ok(with_replay(value, replay_items));
            }
            body["input"] = serde_json::Value::Array(replay_items.clone());
        }
        unreachable!()
    }
}

fn is_programmatic_not_supported(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("programmatic_tool_calling")
        || message.contains("allowed_callers")
        || message.contains("programmatic tool calling is not supported")
}

fn with_notice(mut response: serde_json::Value, message: String) -> serde_json::Value {
    response["_codeloom_notice"] = serde_json::Value::String(message);
    response
}

fn with_replay(
    mut response: serde_json::Value,
    replay_items: Vec<serde_json::Value>,
) -> serde_json::Value {
    response["_codeloom_replay_items"] = serde_json::Value::Array(replay_items);
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
        if response["_codeloom_notice"].is_string() {
            chunks.push(StreamChunk::ProviderNotice(
                response["_codeloom_notice"]
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
                "replay_items": response["_codeloom_replay_items"],
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_text_delta() {
        let mut acc = ToolCallAcc::default();
        let data = json!({"choices":[{"delta":{"content":"hi"}}]}).to_string();
        let chunks = map_openai_event(&data, &mut acc);
        assert!(matches!(chunks.as_slice(), [StreamChunk::TextDelta(t)] if t == "hi"));
    }

    #[test]
    fn usage_reports_cached_tokens_and_cache_writes() {
        let mut acc = ToolCallAcc::default();
        let chunks = map_openai_event(
            r#"{"usage":{"prompt_tokens":1000,"completion_tokens":2,
                "prompt_tokens_details":{"cached_tokens":800,"cache_write_tokens":50}}}"#,
            &mut acc,
        );
        // El fragmento de uso conserva el índice 0 y el de caché va a continuación.
        assert!(matches!(&chunks[0], StreamChunk::Usage(usage) if usage.input_tokens == 1000));
        assert!(matches!(
            &chunks[1],
            StreamChunk::CacheUsage(cache) if cache.read_tokens == 800 && cache.write_tokens == 50
        ));
    }

    #[test]
    fn usage_reports_deepseek_cache_hits() {
        let mut acc = ToolCallAcc::default();
        let chunks = map_openai_event(
            r#"{"usage":{"prompt_tokens":500,"completion_tokens":1,"prompt_cache_hit_tokens":300}}"#,
            &mut acc,
        );
        assert!(chunks.iter().any(|chunk| matches!(
            chunk,
            StreamChunk::CacheUsage(cache) if cache.read_tokens == 300 && cache.write_tokens == 0
        )));
    }

    #[test]
    fn usage_without_cache_details_emits_no_cache_chunk() {
        let mut acc = ToolCallAcc::default();
        let chunks = map_openai_event(
            r#"{"usage":{"prompt_tokens":4,"completion_tokens":2}}"#,
            &mut acc,
        );
        assert_eq!(chunks.len(), 1);
        assert!(matches!(&chunks[0], StreamChunk::Usage(_)));
    }

    #[test]
    fn cache_usage_ignores_negative_and_non_numeric_values() {
        assert_eq!(
            cache_usage(&serde_json::json!({
                "prompt_tokens_details":{"cached_tokens":-1,"cache_write_tokens":"x"}
            })),
            None
        );
    }

    #[test]
    fn maps_reported_usage_cost_and_rejects_invalid_cost() {
        let mut acc = ToolCallAcc::default();
        let chunks = map_openai_event(
            r#"{"usage":{"prompt_tokens":4,"completion_tokens":2,"cost":0.0012}}"#,
            &mut acc,
        );
        assert!(
            matches!(&chunks[0], StreamChunk::Usage(usage) if usage.reported_cost_usd == Some(0.0012))
        );
        let chunks = map_openai_event(r#"{"usage":{"cost":-1}}"#, &mut acc);
        assert!(
            matches!(&chunks[0], StreamChunk::Usage(usage) if usage.reported_cost_usd.is_none())
        );
    }

    #[test]
    fn assembles_tool_call_across_events() {
        let mut acc = ToolCallAcc::default();
        let args1 = r#"{"path":"#;
        let args2 = r#""a.rs"}"#;
        let e1 = json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":"c1","function":{"name":"view","arguments":args1}}]}}]})
        .to_string();
        let e2 = json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"function":{"arguments":args2}}]}}]})
        .to_string();
        let e3 = json!({"choices":[{"finish_reason":"tool_calls"}]}).to_string();
        assert!(map_openai_event(&e1, &mut acc).is_empty());
        assert!(map_openai_event(&e2, &mut acc).is_empty());
        let chunks = map_openai_event(&e3, &mut acc);
        assert!(
            matches!(chunks.first(), Some(StreamChunk::ToolCallComplete { name, input, .. })
            if name == "view" && input["path"] == "a.rs")
        );
        assert!(matches!(
            chunks.last(),
            Some(StreamChunk::Done(FinishReason::ToolCalls))
        ));
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_text_then_done() {
        let server = MockServer::start().await;
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n";
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
            .mount(&server)
            .await;

        let p = OpenAiCompatProvider::new(server.uri(), "sk-test");
        let req = ProviderRequest {
            model: "m".into(),
            system: String::new(),
            messages: vec![],
            tools: vec![],
            temperature: 0.0,
            max_tokens: 16,
            effort: None,
            programmatic_tool_calling: false,
        };
        let mut s = p.stream(req, CancellationToken::new()).await.unwrap();
        let mut texts = String::new();
        let mut saw_done = false;
        while let Some(c) = s.next().await {
            match c {
                StreamChunk::TextDelta(t) => texts.push_str(&t),
                StreamChunk::Done(_) => saw_done = true,
                _ => {}
            }
        }
        assert_eq!(texts, "hello");
        assert!(saw_done);
    }

    #[tokio::test]
    async fn request_timeout_reports_failure_before_response_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_raw("data: [DONE]\n\n", "text/event-stream"),
            )
            .mount(&server)
            .await;
        let mut provider = OpenAiCompatProvider::new(server.uri(), "sk-test");
        provider.request_timeout = Duration::from_millis(10);
        let request = ProviderRequest {
            model: "m".into(),
            system: String::new(),
            messages: vec![],
            tools: vec![],
            temperature: 0.0,
            max_tokens: 16,
            effort: None,
            programmatic_tool_calling: false,
        };

        assert!(matches!(
            provider.stream(request, CancellationToken::new()).await,
            Err(CoreError::Provider(message)) if message.contains("timed out before response headers")
        ));
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::message::{ContentPart, Message, Role};

    #[test]
    fn serializes_tool_calls_and_tool_results() {
        let p = OpenAiCompatProvider::new("http://x", "k");
        let req = ProviderRequest {
            model: "m".into(),
            system: "sys".into(),
            messages: vec![
                Message::assistant(vec![ContentPart::ToolCall {
                    id: "c1".into(),
                    name: "view".into(),
                    input: serde_json::json!({"path":"a"}),
                }]),
                Message::new(
                    Role::Tool,
                    vec![ContentPart::ToolResult {
                        call_id: "c1".into(),
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
        assert_eq!(assistant["tool_calls"][0]["id"], "c1");
        assert_eq!(assistant["tool_calls"][0]["function"]["name"], "view");
        assert!(assistant["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap()
            .contains("path"));
        let tool = msgs.iter().find(|m| m["role"] == "tool").unwrap();
        assert_eq!(tool["tool_call_id"], "c1");
        assert_eq!(tool["content"], "ok");
    }

    #[test]
    fn body_requests_usage_in_the_stream() {
        let p = OpenAiCompatProvider::new("http://x", "k");
        let req = ProviderRequest {
            model: "m".into(),
            system: String::new(),
            messages: vec![],
            tools: vec![],
            temperature: 0.0,
            max_tokens: 16,
            effort: None,
            programmatic_tool_calling: false,
        };
        let body = p.build_body(&req);
        assert_eq!(body["stream_options"]["include_usage"], true);
    }
}

#[cfg(test)]
mod effort_tests {
    use super::*;
    use crate::provider::ToolSpec;
    use serde_json::json;

    /// Petición de prueba con una herramienta, para que el cuerpo sea representativo.
    fn peticion(effort: Option<&str>) -> ProviderRequest {
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
            max_tokens: 16,
            effort: effort.map(String::from),
            programmatic_tool_calling: false,
        }
    }

    #[test]
    fn openai_body_without_effort_is_unchanged() {
        // Cuerpo exacto que enviaba `build_body` antes de existir el esfuerzo.
        let previo = json!({
            "model": "m",
            "messages": [{ "role": "system", "content": "sys" }],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "view",
                    "description": "Ve un fichero",
                    "parameters": { "type": "object" }
                }
            }],
            "stream": true,
            "stream_options": { "include_usage": true },
            "temperature": 0.5,
            "max_tokens": 16
        });
        let p = OpenAiCompatProvider::new("http://x", "k");
        assert_eq!(p.build_body(&peticion(None)), previo);
    }

    #[test]
    fn openai_body_sends_reasoning_effort() {
        let p = OpenAiCompatProvider::new("http://x", "k");
        let con = p.build_body(&peticion(Some("high")));
        assert_eq!(con["reasoning_effort"], "high");
        assert!(con.get("reasoning").is_none());
        // Salvo el campo nuevo, el cuerpo es idéntico al de «default».
        let mut sin_campo = con.clone();
        sin_campo
            .as_object_mut()
            .unwrap()
            .remove("reasoning_effort");
        assert_eq!(sin_campo, p.build_body(&peticion(None)));
    }

    #[test]
    fn openrouter_body_nests_effort_under_reasoning() {
        let p = OpenAiCompatProvider::new("https://openrouter.ai/api/v1", "k");
        let con = p.build_body(&peticion(Some("high")));
        assert_eq!(con["reasoning"]["effort"], "high");
        assert!(con.get("reasoning_effort").is_none());
    }

    #[test]
    fn oversized_tool_results_from_old_history_are_capped_on_the_wire() {
        use crate::message::{ContentPart, Message, Role};
        let big = "x".repeat(crate::tools::MAX_TOOL_OUTPUT_BYTES * 4);
        let msgs = vec![Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "c1".into(),
                content: big.clone(),
                is_error: false,
            }],
        )];
        let wire = super::wire_messages(&msgs);
        let sent = wire[0]["content"].as_str().unwrap();
        assert!(sent.len() < big.len());
        // ADAPTADO: el resultado se recorta ahora con cabeza y cola y el marcador «bytes omitted».
        assert!(sent.contains("bytes omitted ...]"));
    }

    #[test]
    fn encodes_local_image_parts_as_openai_data_urls() {
        use crate::message::{ContentPart, Message};
        let mut message = Message::human("What is this?");
        message.parts.push(ContentPart::Image {
            mime_type: "image/png".into(),
            data: "cG5n".into(),
            filename: "sample.png".into(),
        });
        let wire = super::wire_messages(&[message]);
        assert_eq!(wire[0]["content"][0]["text"], "What is this?");
        assert_eq!(
            wire[0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,cG5n"
        );
    }
}

#[cfg(test)]
mod responses_tests {
    use super::*;
    use crate::provider::ToolSpec;
    use serde_json::json;

    fn request() -> ProviderRequest {
        ProviderRequest {
            model: "model-with-ptc".into(),
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

    #[test]
    fn responses_request_opts_in_only_read_tools_and_keeps_stateless_mode() {
        let provider = OpenAiResponsesProvider::new("http://localhost/v1", "test-key");
        let body = provider.build_body(&request());
        assert_eq!(body["store"], false);
        assert!(body.get("previous_response_id").is_none());
        assert!(body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["type"] == "programmatic_tool_calling"));
        assert_eq!(
            body["tools"][0]["allowed_callers"],
            json!(["direct", "programmatic"])
        );
        assert!(body["tools"][1].get("allowed_callers").is_none());
    }

    #[test]
    fn responses_replay_preserves_program_items_and_caller_on_tool_results() {
        use crate::message::{ContentPart, Message, ProviderState, Role};
        let provider = OpenAiResponsesProvider::new("http://localhost/v1", "test-key");
        let mut assistant = Message::assistant(vec![ContentPart::ToolCall {
            id: "call-1".into(),
            name: "view".into(),
            input: json!({"path":"README.md"}),
        }]);
        assistant.provider_state = Some(ProviderState {
            provider: "openai_responses".into(),
            data: json!({
                "response_id":"resp-1", "model":"model-with-ptc",
                "base_url":"http://localhost/v1",
                "replay_items":[{
                    "type":"program","call_id":"prog-1","code":"const x = 1",
                    "fingerprint":"opaque"
                },{
                    "type":"function_call","call_id":"call-1","name":"view",
                    "arguments":"{}","caller":{"type":"program","caller_id":"prog-1"}
                }],
                "caller_by_call_id":{"call-1":{"type":"program","caller_id":"prog-1"}}
            }),
        });
        let result = Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "call-1".into(),
                content: "file contents".into(),
                is_error: false,
            }],
        );
        let mut req = request();
        req.messages = vec![assistant, result];
        let body = provider.build_body(&req);
        assert_eq!(body["store"], false);
        assert_eq!(body["input"][0]["type"], "program");
        assert_eq!(body["input"][1]["type"], "function_call");
        assert_eq!(body["input"][2]["type"], "function_call_output");
        assert_eq!(body["input"][2]["caller"]["caller_id"], "prog-1");
    }

    #[tokio::test]
    async fn rejects_programmatic_calls_to_ineligible_tools() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"resp_1","status":"completed","output":[{
                    "type":"function_call","call_id":"call_1","name":"write",
                    "arguments":"{}","caller":{"type":"program","caller_id":"prog_1"}
                }]
            })))
            .mount(&server)
            .await;
        let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");
        assert!(provider
            .stream(request(), CancellationToken::new())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn unsupported_programmatic_mode_retries_once_with_direct_tools() {
        use futures::StreamExt;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error":{"message":"programmatic_tool_calling is not supported"}
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"resp_direct","status":"completed","output":[]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");
        let mut stream = provider
            .stream(request(), CancellationToken::new())
            .await
            .unwrap();
        let mut notice = false;
        while let Some(chunk) = stream.next().await {
            if matches!(chunk, StreamChunk::ProviderNotice(ref text) if text.contains("using direct calls"))
            {
                notice = true;
            }
        }
        assert!(notice);
        let sent = server.received_requests().await.unwrap();
        assert_eq!(sent.len(), 2);
        let fallback: serde_json::Value = serde_json::from_slice(&sent[1].body).unwrap();
        assert!(fallback["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "programmatic_tool_calling"));
    }
}
