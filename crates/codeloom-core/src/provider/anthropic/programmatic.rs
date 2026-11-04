//! Llamada programática de herramientas (code execution): sondeo de soporte y flujo propio.

use crate::error::{CoreError, Result};
use crate::message::FinishReason;
use crate::provider::{ProviderRequest, ProviderStream, StreamChunk};
use tokio_util::sync::CancellationToken;

use super::*;

impl AnthropicProvider {
    pub(super) async fn supports_code_execution(
        &self,
        model: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<bool>> {
        // OpenRouter's public model record does not expose Anthropic's PTC capability field.
        // Try only the documented Haiku 5.5 model and pin its upstream to Anthropic.
        if self.is_openrouter_haiku_55(model) {
            return Ok(Some(true));
        }
        if let Some(supported) = self.cached_code_execution(model) {
            return Ok(Some(supported));
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
        self.note_code_execution(model, supported);
        Ok(Some(supported))
    }

    pub(super) async fn stream_programmatic(
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
                self.note_ptc_rejection(&req.model, status.as_u16());
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
            chunks.extend(cache_usage(usage).map(StreamChunk::CacheUsage));
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
