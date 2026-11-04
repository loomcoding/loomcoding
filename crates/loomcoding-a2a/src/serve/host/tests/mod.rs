//! Tests del módulo, por tema.

use super::bounds::*;
use super::*;
use futures::stream;
use loomcoding_core::config::{LoomcodingConfig, ProviderConfig, ProviderKind};
use loomcoding_core::error::Result as CoreResult;
use loomcoding_core::event::EventBus;
use loomcoding_core::job::LocalJobRegistry;
use loomcoding_core::message::FinishReason;
use loomcoding_core::permission::PermissionService;
use loomcoding_core::provider::{
    Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk,
};
use loomcoding_core::registry::{AgentRegistry, BuiltInSource};
use loomcoding_core::session::sqlite::SqliteStore;
use loomcoding_core::subagent::InProcessSubagentLauncher;
use std::sync::RwLock;

mod bounds;
mod streams;
mod support;
mod tasks;
