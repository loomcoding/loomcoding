//! Tests del módulo, por tema.

use super::bounds::*;
use super::*;
use codeloom_core::config::{CodeLoomConfig, ProviderConfig, ProviderKind};
use codeloom_core::error::Result as CoreResult;
use codeloom_core::event::EventBus;
use codeloom_core::job::LocalJobRegistry;
use codeloom_core::message::FinishReason;
use codeloom_core::permission::PermissionService;
use codeloom_core::provider::{
    Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk,
};
use codeloom_core::registry::{AgentRegistry, BuiltInSource};
use codeloom_core::session::sqlite::SqliteStore;
use codeloom_core::subagent::InProcessSubagentLauncher;
use futures::stream;
use std::sync::RwLock;

mod bounds;
mod streams;
mod support;
mod tasks;
