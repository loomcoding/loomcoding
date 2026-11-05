use super::sse::SseBuffer;
use super::{Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk};
use crate::error::{CoreError, Result};
use crate::message::{FinishReason, Usage};
use async_trait::async_trait;
use futures::StreamExt;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

mod compat;
mod responses;

pub(crate) use compat::stream_error_text;
pub use compat::{map_openai_event, OpenAiCompatProvider, ToolCallAcc};
pub use responses::OpenAiResponsesProvider;

#[cfg(test)]
mod tests;
