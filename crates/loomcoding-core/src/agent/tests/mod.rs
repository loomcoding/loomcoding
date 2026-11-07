//! Tests del bucle del agente, repartidos por tema.
//!
//! Cabe señalar que los fixtures compartidos viven en `support`; los que usa un solo tema
//! se quedan en su fichero. Todos ven lo privado del módulo padre.

use super::*;
use crate::provider::{Capabilities, ProviderStream, StreamChunk};
use crate::session::sqlite::SqliteStore;
use crate::subagent_phase::SubagentPhase;
use crate::tools::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Semaphore};

mod support;
use support::*;
mod background_summary;
mod budget;
mod close_gate;
mod compaction;
mod compaction_in_turn;
mod delegation;
mod exploration;
mod handoff;
mod kill;
mod notices;
mod output_limit;
mod reasoning;
mod settle;
mod spill;
mod step_limit;
mod steps;
mod stopped;
mod system_prompt;
mod temperature;
mod tool_observer;
mod turn;
mod usage_records;
mod watch_notices;
