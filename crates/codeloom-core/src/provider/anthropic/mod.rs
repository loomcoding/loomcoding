//! Proveedor de la Messages API de Anthropic (y de los gateways compatibles, como OpenRouter).
//!
//! El reparto: `stream` acumula y mapea los eventos SSE; `body` construye la petición;
//! `programmatic` sondea y sigue la llamada programática de herramientas; `errors` interpreta
//! los fallos HTTP. Aquí quedan el tipo, sus notas de capacidades y el `impl Provider`.

use super::sse::SseBuffer;
use super::{Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk};
use crate::capability::{Capability, CapabilityRegistry, Protocol, Source, State, Target};
use crate::error::{CoreError, Result};
use crate::message::FinishReason;
use async_trait::async_trait;
use futures::StreamExt;
use tokio_util::sync::CancellationToken;

const OPENROUTER_HAIKU_55: &str = "anthropic/claude-haiku-5.5";
const OPENROUTER_PTC_UNSUPPORTED: &str = "OpenRouter rejected Anthropic programmatic tool calling";

mod body;
mod errors;
mod programmatic;
mod stream;

use errors::*;
use stream::*;
pub use stream::{map_anthropic_event, AnthropicAcc};

#[cfg(test)]
mod tests;

pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    capabilities: CapabilityRegistry,
    /// Si marca puntos de caché de prompts en el cuerpo. Apagado por omisión.
    prompt_cache: bool,
}

impl AnthropicProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            capabilities: CapabilityRegistry::default(),
            prompt_cache: false,
        }
    }

    /// Activa los puntos de caché de prompts (`cache_control`) en las peticiones.
    pub fn with_prompt_cache(mut self, enabled: bool) -> Self {
        self.prompt_cache = enabled;
        self
    }

    /// Destino exacto de una petición: el mismo modelo en otra URL es otro destino.
    fn target(&self, model: &str) -> Target {
        Target::new(Protocol::AnthropicMessages, &self.base_url, model)
    }

    fn cached_code_execution(&self, model: &str) -> Option<bool> {
        match self
            .capabilities
            .query(&self.target(model), Capability::CodeExecution)
            .state
        {
            State::Supported => Some(true),
            State::Unsupported => Some(false),
            State::Unknown | State::Rejected { .. } => None,
        }
    }

    fn note_code_execution(&self, model: &str, supported: bool) {
        self.capabilities.record(
            &self.target(model),
            Capability::CodeExecution,
            supported,
            Source::OfficialMetadata,
        );
    }

    /// Un rechazo real de PTC se recuerda hasta que caduque, para no repetirlo en cada turno.
    fn note_ptc_rejection(&self, model: &str, status: u16) {
        self.capabilities.record_rejection(
            &self.target(model),
            Capability::ProgrammaticToolCalling,
            status,
        );
    }

    fn ptc_rejected(&self, model: &str) -> bool {
        matches!(
            self.capabilities
                .query(&self.target(model), Capability::ProgrammaticToolCalling)
                .state,
            State::Rejected { .. }
        )
    }

    fn is_openrouter(&self) -> bool {
        reqwest::Url::parse(&self.base_url)
            .is_ok_and(|url| url.host_str() == Some("openrouter.ai") && url.path() == "/api")
    }

    fn is_openrouter_haiku_55(&self, model: &str) -> bool {
        self.is_openrouter() && model == OPENROUTER_HAIKU_55
    }
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
                Some(true) if self.ptc_rejected(&req.model) => {
                    fallback_notice = Some(format!(
                        "Programmatic tools were rejected earlier for `{}`; using direct calls",
                        req.model
                    ));
                }
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
