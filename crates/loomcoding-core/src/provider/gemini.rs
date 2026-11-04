use super::{Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk};
use crate::capability::{
    classify_rejection, Capability, CapabilityRegistry, Protocol, State, Target,
};
use crate::error::{CoreError, Result};
use crate::message::{ContentPart, FinishReason, Role, Usage};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct GeminiProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    capabilities: CapabilityRegistry,
}

impl GeminiProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            capabilities: CapabilityRegistry::default(),
        }
    }

    fn target(&self, model: &str) -> Target {
        Target::new(Protocol::Gemini, &self.base_url, model)
    }

    fn combination_rejected(&self, model: &str) -> bool {
        matches!(
            self.capabilities
                .query(&self.target(model), Capability::CodeExecution)
                .state,
            State::Rejected { .. }
        )
    }

    fn supports_tool_combination(model: &str) -> bool {
        let model = model.strip_prefix("models/").unwrap_or(model);
        model.starts_with("gemini-3") || model.starts_with("gemini-4")
    }

    fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
        let previous = req
            .messages
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, message)| {
                let state = message.provider_state.as_ref()?;
                (state.provider == "gemini_interactions"
                    && state.data["model"] == req.model
                    && state.data["base_url"] == self.base_url
                    && state.data["interaction_id"].is_string())
                .then_some((index, state))
            });
        let mut input = Vec::new();
        let mut body = serde_json::json!({
            "model": req.model,
            "system_instruction": req.system,
            "tools": req.tools.iter().map(|tool| serde_json::json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.input_schema
            })).collect::<Vec<_>>(),
            "store": true
        });
        if req.programmatic_tool_calling
            && Self::supports_tool_combination(&req.model)
            && req.tools.iter().any(|tool| tool.programmatic_allowed)
        {
            body["tools"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "type": "code_execution"
                }));
        }

        if let Some((index, state)) = previous {
            body["previous_interaction_id"] = state.data["interaction_id"].clone();
            let tool_names = &state.data["tool_names_by_call_id"];
            for message in &req.messages[index + 1..] {
                match message.role {
                    Role::User => input.push(serde_json::json!({
                        "type": "user_input",
                        "content": [{"type": "text", "text": message.text()}]
                    })),
                    Role::Tool => {
                        for part in &message.parts {
                            if let ContentPart::ToolResult {
                                call_id,
                                content,
                                is_error,
                            } = part
                            {
                                let mut name = tool_names[call_id].clone();
                                if name.as_str().unwrap_or_default().is_empty() {
                                    name = serde_json::Value::String("unknown".into());
                                }
                                input.push(serde_json::json!({
                                    "type": "function_result",
                                    "name": name,
                                    "call_id": call_id,
                                    "result": [{
                                        "type": "text",
                                        "text": if *is_error {
                                            format!("Tool error: {}", crate::tools::limit_output(content))
                                        } else {
                                            crate::tools::limit_output(content).into_owned()
                                        }
                                    }]
                                }));
                            }
                        }
                    }
                    Role::Assistant if !message.text().is_empty() => {
                        input.push(serde_json::json!({
                            "type": "user_input",
                            "content": [{"type": "text", "text": format!("Previous assistant response: {}", message.text())}]
                        }));
                    }
                    _ => {}
                }
            }
        } else {
            let mut transcript = String::new();
            for message in &req.messages {
                let role = match message.role {
                    Role::User => "User",
                    Role::Assistant => "Assistant",
                    Role::Tool => "Tool result",
                    Role::System => "System",
                };
                let text = message.text();
                if !text.is_empty() {
                    transcript.push_str(role);
                    transcript.push_str(":\n");
                    transcript.push_str(&text);
                    transcript.push_str("\n\n");
                }
                for part in &message.parts {
                    if let ContentPart::ToolResult {
                        call_id, content, ..
                    } = part
                    {
                        transcript.push_str(&format!("Tool result {call_id}:\n{}\n\n", content));
                    }
                }
            }
            input.push(serde_json::json!({
                "type": "user_input",
                "content": [{"type": "text", "text": transcript}]
            }));
        }
        body["input"] = serde_json::Value::Array(input);
        body
    }

    async fn create_interaction(
        &self,
        req: &ProviderRequest,
        cancel: &CancellationToken,
    ) -> Result<(serde_json::Value, Option<String>)> {
        let direct_request;
        let mut earlier_notice = None;
        let req = if req.programmatic_tool_calling && self.combination_rejected(&req.model) {
            direct_request = ProviderRequest {
                programmatic_tool_calling: false,
                ..req.clone()
            };
            earlier_notice = Some(format!(
                "Gemini tool combination was rejected earlier for `{}`; using direct tool calls",
                req.model
            ));
            &direct_request
        } else {
            req
        };
        let body = self.build_body(req);
        let combination = body["tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|tool| tool["type"] == "code_execution"));
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            response = self.client.post(format!("{}/interactions", self.base_url))
                .timeout(std::time::Duration::from_secs(120))
                .header("x-goog-api-key", &self.api_key)
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
            let retry_direct = combination
                && body.get("previous_interaction_id").is_none()
                && classify_rejection(status.as_u16(), message, Capability::CodeExecution)
                    .allows_fallback();
            if retry_direct {
                self.capabilities.record_rejection(
                    &self.target(&req.model),
                    Capability::CodeExecution,
                    status.as_u16(),
                );
                let mut direct = body;
                direct["tools"] = serde_json::Value::Array(
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
                let response = tokio::select! {
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    response = self.client.post(format!("{}/interactions", self.base_url))
                        .timeout(std::time::Duration::from_secs(120))
                        .header("x-goog-api-key", &self.api_key)
                        .json(&direct)
                        .send() => response.map_err(|error| CoreError::Provider(error.to_string()))?,
                };
                if !response.status().is_success() {
                    return Err(CoreError::Provider(format!(
                        "Gemini Interactions API http {}",
                        response.status()
                    )));
                }
                let value = response
                    .json()
                    .await
                    .map_err(|error| CoreError::Provider(error.to_string()))?;
                return Ok((value, Some("Gemini tool combination is unavailable for this model; using direct tool calls".into())));
            }
            return Err(CoreError::Provider(format!(
                "Gemini Interactions API http {status}"
            )));
        }
        let value = response
            .json()
            .await
            .map_err(|error| CoreError::Provider(format!("invalid Gemini response: {error}")))?;
        Ok((value, earlier_notice))
    }
}

#[async_trait]
impl Provider for GeminiProvider {
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
        let requested_combination =
            req.programmatic_tool_calling && req.tools.iter().any(|tool| tool.programmatic_allowed);
        let known_model = Self::supports_tool_combination(&req.model);
        let (response, fallback_notice) = if requested_combination && !known_model {
            let mut direct = req.clone();
            direct.programmatic_tool_calling = false;
            (
                self.create_interaction(&direct, &cancel).await?.0,
                Some(format!(
                    "Gemini model `{}` is not in the verified Gemini 3/4 tool-combination families; using direct tool calls",
                    req.model
                )),
            )
        } else {
            self.create_interaction(&req, &cancel).await?
        };
        let steps = response["steps"].as_array().cloned().unwrap_or_default();
        let mut chunks = vec![StreamChunk::Started];
        let mut tool_names_by_call_id = serde_json::Map::new();
        let mut has_calls = false;
        for step in &steps {
            match step["type"].as_str().unwrap_or_default() {
                "model_output" => {
                    if let Some(content) = step["content"].as_array() {
                        for part in content {
                            if part["type"] == "text" {
                                if let Some(text) = part["text"].as_str() {
                                    chunks.push(StreamChunk::TextDelta(text.to_string()));
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let id = step["id"].as_str().unwrap_or_default();
                    let name = step["name"].as_str().unwrap_or_default();
                    tool_names_by_call_id.insert(id.to_string(), serde_json::json!(name));
                    chunks.push(StreamChunk::ToolCallComplete {
                        id: id.to_string(),
                        name: name.to_string(),
                        input: step
                            .get("arguments")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    });
                    has_calls = true;
                }
                _ => {}
            }
        }
        if let Some(usage) = response.get("usage") {
            chunks.push(StreamChunk::Usage(Usage {
                input_tokens: usage["total_input_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: usage["total_output_tokens"].as_u64().unwrap_or(0) as u32,
                reported_cost_usd: usage["cost"]
                    .as_f64()
                    .filter(|cost| cost.is_finite() && *cost >= 0.0),
            }));
        }
        if let Some(notice) = fallback_notice {
            chunks.push(StreamChunk::ProviderNotice(notice));
        }
        let interaction_id = response["id"].as_str().unwrap_or_default();
        if interaction_id.is_empty() {
            return Err(CoreError::Provider(
                "Gemini response has no interaction ID".into(),
            ));
        }
        let state = crate::message::ProviderState {
            provider: "gemini_interactions".into(),
            data: serde_json::json!({
                "interaction_id": interaction_id,
                "model": req.model,
                "base_url": self.base_url,
                "tool_names_by_call_id": tool_names_by_call_id,
            }),
        };
        if serde_json::to_vec(&state).map_or(true, |bytes| bytes.len() > 64 * 1024) {
            return Err(CoreError::Provider(
                "Gemini continuation state exceeds 64 KiB".into(),
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
    use crate::provider::ToolSpec;
    use serde_json::json;

    fn request(model: &str, eligible: bool) -> ProviderRequest {
        ProviderRequest {
            model: model.into(),
            system: "system".into(),
            messages: vec![],
            tools: vec![ToolSpec {
                name: "view".into(),
                description: "read a file".into(),
                input_schema: json!({"type":"object"}),
                programmatic_allowed: eligible,
            }],
            temperature: 0.0,
            max_tokens: 128,
            effort: None,
            programmatic_tool_calling: true,
        }
    }

    #[test]
    fn adds_native_tool_combination_only_for_supported_model_and_eligible_tool() {
        let provider = GeminiProvider::new("http://localhost/v1beta", "test-key");
        let body = provider.build_body(&request("gemini-4-argon", true));
        assert!(body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["type"] == "code_execution"));

        let old_model = provider.build_body(&request("gemini-2.5-flash", true));
        assert!(old_model["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "code_execution"));
        let ineligible = provider.build_body(&request("gemini-4-argon", false));
        assert!(ineligible["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "code_execution"));
    }

    #[test]
    fn interaction_continuation_returns_client_tool_result_to_previous_interaction() {
        use crate::message::{ContentPart, Message, ProviderState, Role};
        let provider = GeminiProvider::new("http://localhost/v1beta", "test-key");
        let mut assistant = Message::assistant(vec![ContentPart::ToolCall {
            id: "call-1".into(),
            name: "view".into(),
            input: json!({"path":"README.md"}),
        }]);
        assistant.provider_state = Some(ProviderState {
            provider: "gemini_interactions".into(),
            data: json!({"interaction_id":"interaction-1","model":"gemini-4-argon",
                "base_url":"http://localhost/v1beta",
                "tool_names_by_call_id":{"call-1":"view"}}),
        });
        let result = Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "call-1".into(),
                content: "file contents".into(),
                is_error: false,
            }],
        );
        let mut req = request("gemini-4-argon", true);
        req.messages = vec![assistant, result];
        let body = provider.build_body(&req);
        assert_eq!(body["previous_interaction_id"], "interaction-1");
        assert_eq!(body["input"][0]["type"], "function_result");
        assert_eq!(body["input"][0]["name"], "view");
        assert_eq!(body["input"][0]["call_id"], "call-1");
    }

    #[tokio::test]
    async fn unknown_model_falls_back_to_direct_tools() {
        use futures::StreamExt;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/interactions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"interaction-1","steps":[]
            })))
            .mount(&server)
            .await;
        let provider = GeminiProvider::new(server.uri(), "test-key");
        let mut stream = provider
            .stream(request("gemini-2.5-flash", true), CancellationToken::new())
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
        assert_eq!(sent.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&sent[0].body).unwrap();
        assert!(body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "code_execution"));
    }

    #[tokio::test]
    async fn unsupported_combination_response_retries_with_direct_tools() {
        use futures::StreamExt;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/interactions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error":{"message":"code_execution tool combination unsupported"}
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/interactions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"interaction-direct","steps":[]
            })))
            .mount(&server)
            .await;

        let provider = GeminiProvider::new(server.uri(), "test-key");
        let mut stream = provider
            .stream(request("gemini-4-argon", true), CancellationToken::new())
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
        assert_eq!(sent.len(), 2);
        let attempted: serde_json::Value = serde_json::from_slice(&sent[0].body).unwrap();
        let fallback: serde_json::Value = serde_json::from_slice(&sent[1].body).unwrap();
        assert!(attempted["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["type"] == "code_execution"));
        assert!(fallback["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "code_execution"));
    }

    async fn servidor_que_rechaza_la_combinacion(
        status: u16,
        mensaje: &str,
    ) -> wiremock::MockServer {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/interactions"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "error":{"message": mensaje}
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/interactions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"interaction-direct","steps":[]
            })))
            .mount(&server)
            .await;
        server
    }

    fn usa_la_combinacion(cuerpo: &[u8]) -> bool {
        let value: serde_json::Value = serde_json::from_slice(cuerpo).unwrap();
        value["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["type"] == "code_execution")
    }

    #[tokio::test]
    async fn a_rejected_combination_is_remembered_and_not_repeated_on_the_next_turn() {
        use futures::StreamExt;
        let server =
            servidor_que_rechaza_la_combinacion(400, "code_execution tool combination unsupported")
                .await;
        let provider = GeminiProvider::new(server.uri(), "test-key");
        let mut avisos = Vec::new();
        for _ in 0..2 {
            let mut stream = provider
                .stream(request("gemini-4-argon", true), CancellationToken::new())
                .await
                .unwrap();
            avisos.clear();
            while let Some(chunk) = stream.next().await {
                if let StreamChunk::ProviderNotice(texto) = chunk {
                    avisos.push(texto);
                }
            }
        }
        let sent = server.received_requests().await.unwrap();
        // Turno 1: combinación rechazada + reintento directo. Turno 2: directo desde el principio.
        assert_eq!(sent.len(), 3);
        assert!(usa_la_combinacion(&sent[0].body));
        assert!(!usa_la_combinacion(&sent[1].body));
        assert!(
            !usa_la_combinacion(&sent[2].body),
            "no se debe repetir la petición rechazada"
        );
        assert!(
            avisos
                .iter()
                .any(|texto| texto.contains("rejected earlier")),
            "{avisos:?}"
        );
    }

    #[tokio::test]
    async fn an_auth_or_quota_error_that_mentions_the_combination_does_not_fall_back() {
        for status in [401u16, 402, 429, 500] {
            let server = servidor_que_rechaza_la_combinacion(
                status,
                "code_execution: check your key or quota",
            )
            .await;
            let provider = GeminiProvider::new(server.uri(), "test-key");
            let resultado = provider
                .stream(request("gemini-4-argon", true), CancellationToken::new())
                .await;
            assert!(resultado.is_err(), "HTTP {status} no es incompatibilidad");
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }
}
