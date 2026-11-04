use crate::error::Result;
use crate::message::{FinishReason, Message, Usage};
use async_trait::async_trait;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

pub mod anthropic;
pub mod gemini;
pub mod openai;
pub mod sse;

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    /// Whether this tool is read-only and may be called from provider-hosted code.
    pub programmatic_allowed: bool,
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Nivel de esfuerzo de razonamiento; `None` significa «default» y la petición no cambia.
    pub effort: Option<String>,
    /// Explicit opt-in. False keeps the current direct tool-calling protocol.
    pub programmatic_tool_calling: bool,
}

#[derive(Debug, Clone)]
pub enum StreamChunk {
    Started,
    ReasoningDelta(String),
    TextDelta(String),
    ToolInputDelta {
        id: String,
        json: String,
    },
    ToolCallComplete {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ProviderState(crate::message::ProviderState),
    ProviderNotice(String),
    Error(String),
    Usage(Usage),
    /// Snapshot parcial del uso durante una respuesta. Los adapters emiten totales acumulados;
    /// `None` significa que ese campo aún no ha sido informado.
    UsageUpdate(UsageUpdate),
    /// Tokens de caché que el proveedor informa para esta petición, como totales acumulados.
    CacheUsage(CacheUsage),
    Done(FinishReason),
}

/// Tokens de entrada que el proveedor informa como leídos de su caché o escritos en ella.
///
/// Cabe señalar que se conservan tal como llegan y no se suman a `input_tokens`: en Anthropic ese
/// campo los excluye, mientras que en los proveedores compatibles con OpenAI los incluye. Un cero
/// significa «no informado o sin caché», nunca una medición.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheUsage {
    pub read_tokens: u64,
    pub write_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct UsageUpdate {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub reported_cost_usd: Option<f64>,
    pub final_update: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub tool_use: bool,
    pub reasoning: bool,
    pub streaming: bool,
}

pub type ProviderStream = Pin<Box<dyn futures::Stream<Item = StreamChunk> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    async fn stream(
        &self,
        req: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderStream>;
}

/// Resuelve un proveedor local declarado por un perfil de subagente.
pub type ProfileProviderResolver = dyn Fn(&crate::config::ProviderConfig) -> crate::error::Result<std::sync::Arc<dyn Provider>>
    + Send
    + Sync;

pub async fn stream_with_retry(
    provider: &dyn Provider,
    req: ProviderRequest,
    cancel: CancellationToken,
    max_retries: u32,
) -> Result<ProviderStream> {
    let mut attempt = 0u32;
    loop {
        match provider.stream(req.clone(), cancel.clone()).await {
            Ok(s) => return Ok(s),
            Err(e) if attempt < max_retries => {
                let backoff = std::time::Duration::from_millis(250 * 2u64.pow(attempt));
                tracing::warn!("provider retry {attempt}: {e}");
                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    _ = cancel.cancelled() => return Err(crate::error::CoreError::Cancelled),
                }
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use futures::stream;

    pub struct ScriptedProvider {
        pub script: Vec<StreamChunk>,
    }

    #[async_trait::async_trait]
    impl Provider for ScriptedProvider {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            _req: ProviderRequest,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> Result<ProviderStream> {
            let items = self.script.clone();
            Ok(Box::pin(stream::iter(items)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::ScriptedProvider;
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn mock_provider_yields_scripted_chunks() {
        let p = ScriptedProvider {
            script: vec![
                StreamChunk::Started,
                StreamChunk::TextDelta("hi".into()),
                StreamChunk::Done(FinishReason::Stop),
            ],
        };
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
        let mut s = p
            .stream(req, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();
        let mut got = vec![];
        while let Some(c) = s.next().await {
            got.push(c);
        }
        assert_eq!(got.len(), 3);
    }
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct FlakyProvider {
        attempts: AtomicU32,
        fail_times: u32,
    }

    #[async_trait::async_trait]
    impl Provider for FlakyProvider {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            _req: ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<ProviderStream> {
            let n = self.attempts.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_times {
                return Err(crate::error::CoreError::Provider(format!("boom {n}")));
            }
            Ok(Box::pin(futures::stream::iter(vec![StreamChunk::Done(
                FinishReason::Stop,
            )])))
        }
    }

    #[tokio::test]
    async fn retries_until_success() {
        let p = FlakyProvider {
            attempts: AtomicU32::new(0),
            fail_times: 2,
        };
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
        let res = stream_with_retry(&p, req, CancellationToken::new(), 2).await;
        assert!(res.is_ok());
        assert_eq!(p.attempts.load(Ordering::SeqCst), 3);
    }
}
