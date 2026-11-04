//! Adaptador del servidor sobre el lanzador de subagentes (A8b).
//!
//! Cabe señalar que servir no inventa un ejecutor nuevo: cada tarea entrante es un subagente local
//! lanzado bajo una sesión padre propia del servidor. Así hereda el estado A2A, la cancelación, el
//! almacén y la observabilidad que ya existen. Lo que sí añade es lo propio de servir: un tope de
//! tareas vivas, un tiempo máximo por tarea, un tope del texto devuelto y la certeza de que solo
//! se ven las tareas de esta sesión.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use codeloom_core::error::CoreError;
use codeloom_core::event::{A2aDirection, A2aObservation, A2aOperation, EventBus, LiveEvent};
use codeloom_core::session::{SessionId, SessionStore, SubagentDescriptor};
use codeloom_core::subagent::SubagentLauncher;
use codeloom_core::tools::glob::GlobTool;
use codeloom_core::tools::grep::GrepTool;
use codeloom_core::tools::view::ViewTool;
use codeloom_core::tools::ToolRegistry;
use tokio_util::sync::CancellationToken;

use super::config::ServeConfig;
use super::confine::ConfinedView;
use super::rpc::{EventStream, HostError, TaskHost, TaskView};
use crate::wire::{WirePart, WireStreamResponse, WireTask};
use codeloom_core::task_events::{flujo_de_tarea, Refrescar};
use futures::StreamExt;

mod bounds;
mod launcher;
mod task_host;

use bounds::*;
pub use bounds::{served_tools, HostLimits, MAX_RESULT_BYTES, MAX_STREAMS};
use launcher::*;

#[cfg(test)]
mod tests;

/// Anfitrión de las tareas entrantes.
pub struct LauncherHost {
    launcher: Arc<dyn SubagentLauncher>,
    parent: SessionId,
    agent: String,
    limits: HostLimits,
    bus: Option<EventBus>,
    /// Huecos de flujos SSE abiertos; cada flujo lleva el suyo hasta que se suelta.
    flujos: Arc<tokio::sync::Semaphore>,
    /// Serializa «contar tareas vivas y arrancar»: sin esto dos envíos simultáneos pasarían el tope.
    arranque: tokio::sync::Mutex<()>,
}
