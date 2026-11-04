//! Tests del lanzador de subagentes, repartidos por tema.
//!
//! Cabe señalar que los proveedores y lanzadores de prueba compartidos viven en `support`;
//! los que usa un solo tema se quedan en su fichero. Todos ven lo privado del módulo padre.

use super::*;
use crate::config::{ProviderConfig, ProviderKind};
use crate::job::{JobRegistry, JobStatus, LocalJobRegistry};
use crate::message::FinishReason;
use crate::permission::PermissionService;
use crate::provider::testutil::ScriptedProvider;
use crate::provider::StreamChunk;
use crate::registry::RemoteFetched;
use crate::registry::{
    AgentOrigin, AgentSource, BuiltInSource, FileSource, ProfileLevel, SourceLoad, TrustStore,
};
use crate::session::sqlite::SqliteStore;
use crate::task::TaskState;
use crate::tools::subagent::SubagentTool;
use crate::tools::{Tool, ToolCtx, ToolOutput, MAX_NOTICE_BYTES};
use std::collections::VecDeque;
use std::time::Duration;

mod support;
use support::*;
mod aprobacion_y_capacidad;
mod ciclo_de_vida;
mod depth_y_persona;
mod enrutado_y_cancelacion;
mod entrada_y_reenvio;
mod perfiles_y_registro;
mod recursion_y_skills;
mod remoto_espejo;
mod remoto_streaming;
mod seguimiento;
