pub mod background;
pub mod changes;
mod clear;
pub mod compact;
pub mod dispatch;
pub mod stop;

use crate::budget::{budget_crossings, BudgetConfig};
use crate::catalog::Catalog;
use crate::config::{ConfirmDelegation, LoomcodingConfig};
use crate::error::{CoreError, Result};
use crate::event::{Activation, EventBus, LiveEvent};
use crate::message::{ContentPart, FinishReason, Message, MessageOrigin, Role, Usage};
use crate::permission::PermissionService;
use crate::provider::{ProfileProviderResolver, Provider, ProviderRequest, StreamChunk};
use crate::session::{SessionId, SessionStore, UsageRecord};
use crate::tools::spill::ToolSpill;
use crate::tools::{limit_bytes, ToolCtx, ToolRegistry, MAX_NOTICE_BYTES, MAX_TOOL_OUTPUT_BYTES};
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio_util::sync::CancellationToken;

mod builder;
mod compaction;
mod entry;
mod handoff;
mod kill;
pub use kill::KillReport;
mod settle;
pub use settle::{MAX_SETTLE_WAIT, MAX_WAKE_TURNS};
mod step_limit;
pub use handoff::HANDOFF_MAX_BYTES;
mod subagents;
mod turn;
mod usage;

pub use compaction::*;
#[cfg(test)]
use turn::*;
use usage::*;

const MAX_PROVIDER_STATE_BYTES: usize = 64 * 1024;

/// Cuántos mensajes se leen del almacén en cada paso del bucle.
///
/// Cabe señalar que es una ventana de lectura, no un límite de lo que se guarda: el
/// almacén conserva todos los mensajes y la compactación sigue viendo el historial que
/// necesita. Antes se leía el historial entero en cada paso, y en sesiones largas con
/// subagentes eso llevaba al daemon a decenas de gigabytes.
const HISTORY_WINDOW: usize = 500;

const MAX_TURN_PROVIDER_STATE_BYTES: usize = 256 * 1024;

static NEXT_USAGE_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Herramientas que `delegation = "off"` retira de la petición.
const DELEGATION_TOOLS: [&str; 6] = [
    "subagent",
    "subagent_control",
    "subagent_list",
    "artifact_read",
    "development_workflow",
    "gauntlet",
];

/// Tope de la sección «Available agents» del prompt de sistema, que crece con el número de
/// agentes del registro y viaja en cada petición.
const MAX_AGENT_SECTION_BYTES: usize = 8_000;

/// Guía de paralelismo del prompt de sistema; solo va si el agente puede delegar. Cota: 400 bytes.
pub(crate) const PARALLEL_GUIDANCE: &str = "When several reads or searches are independent, issue them together in the same response so they run in parallel. When a specialist agent is available for part of the work, delegate that part to it instead of doing everything yourself.";

/// Tope del prompt propio de un agente (el cuerpo de su perfil), que se suma al contexto.
/// Tope del prompt adicional del hijo: la persona del perfil, su separador y el prompt de skills, cada
/// uno con su cota propia (véase `subagent::compose_child_system`), más la holgura del marcador de recorte
/// de la persona, para que un recorte previo no provoque un segundo recorte aquí.
const MAX_SYSTEM_EXTRA_BYTES: usize = crate::subagent::MAX_PROFILE_PROMPT_BYTES
    + 2
    + crate::skills::MAX_SKILL_PROMPT_BYTES
    + crate::subagent::TRUNCATION_MARKER_SLACK;

/// Contenido que recibe el modelo para el resultado de una herramienta. Si supera el tope y hay
/// volcado, guarda el original y añade su ruta tras el recorte; la reserva de 256 bytes deja
/// margen al marcador para que el total siga dentro de `MAX_TOOL_OUTPUT_BYTES`. Si no hay
/// volcado o no se pudo escribir, se recorta sin ruta. Todo recorte abre con una cabecera
/// estructurada; la reserva de 512 bytes cubre la cabecera, el marcador y la ruta.
fn contenido_de_herramienta(spill: Option<&ToolSpill>, call_id: &str, content: &str) -> String {
    if content.len() <= MAX_TOOL_OUTPUT_BYTES {
        return content.to_owned();
    }
    let recorte = crate::tools::limit_bytes_with_header(content, MAX_TOOL_OUTPUT_BYTES - 512);
    match spill.and_then(|spill| spill.save(call_id, content)) {
        Some(ruta) => format!("{recorte}\n[full output saved to: {}]", ruta.display()),
        None => recorte.into_owned(),
    }
}

/// Instrucción final de la petición de resumen, después de los mensajes que se resumen.
const INSTRUCCION_DE_RESUMEN: &str =
    "Summarize the conversation above following your instructions.";

/// Bytes que se reservan para el marcador de omisión que `limit_bytes` añade al recortar.
const MARCADOR_DE_RECORTE: usize = 64;

pub struct TurnOutcome {
    pub session: SessionId,
    pub final_text: String,
    /// Rondas que gastaron presupuesto de trabajo.
    pub steps: usize,
    /// Rondas de solo lectura, que tienen su propio tope.
    pub read_steps: usize,
    /// Tope que cortó el turno, si lo hubo (cancelación y `hard_stop` no lo rellenan).
    pub stopped: Option<stop::StopReason>,
}

impl TurnOutcome {
    /// Informe de un turno que terminó sin respuesta final, o `None` si la hubo.
    ///
    /// Cabe señalar que quien lanza el agente sin interfaz solo ve este texto: callar el motivo
    /// de la parada lo deja a ciegas.
    pub fn unfinished_report(&self) -> Option<String> {
        // Un texto cortado por el tope de salida no es una respuesta: suele ser un bucle de repetición.
        let cut = self.stopped == Some(stop::StopReason::OutputLimit);
        if !self.final_text.trim().is_empty() && !cut {
            return None;
        }
        let cause = self.stopped.map_or(
            "cancellation or an exhausted budget",
            stop::StopReason::label,
        );
        Some(format!(
            "the turn ended without a final answer: stopped by {cause} after {} work steps and {} read steps",
            self.steps, self.read_steps
        ))
    }
}

pub struct AgentLoop {
    provider: Arc<RwLock<Arc<dyn Provider>>>,
    store: Arc<dyn SessionStore>,
    tools: Arc<ToolRegistry>,
    /// Tools available only to local profile-based subagents (currently MCP).
    subagent_tools: Vec<Arc<dyn crate::tools::Tool>>,
    bus: EventBus,
    permission: Arc<PermissionService>,
    config: Arc<RwLock<LoomcodingConfig>>,
    cwd: PathBuf,
    changes: crate::agent::changes::ChangeTracker,
    jobs: Arc<dyn crate::job::JobRegistry>,
    subagents: Option<Arc<dyn crate::subagent::SubagentLauncher>>,
    /// Catálogo compartido de modelos y precios; vacío hasta que `with_catalog` lo sustituye.
    catalog: Arc<RwLock<Catalog>>,
    /// Prompt de sistema propio del agente (perfil). Se añade tras el contexto del proyecto;
    /// `None` o vacío no añade nada.
    system_extra: Option<String>,
    /// Componentes estimados del prompt propio (por ejemplo, fragmentos de skills).
    system_usage_components: Vec<crate::session::UsageComponent>,
    /// Registro de agentes compartido con los subagentes; `None` deja solo el agente `default`.
    registry: Option<Arc<crate::registry::AgentRegistry>>,
    /// Modo de confirmación al delegar; viaja a `RegistrySnapshot::prompt_section` en cada turno.
    delegation_confirm: ConfirmDelegation,
    /// Subagentes activos a la vez (`[agents] max_active_subagents`).
    max_active_subagents: usize,
    /// Topes de lectura y paralelismo (`[limits]`).
    limits: crate::config::LimitsConfig,
    /// Mando de delegación (`[agents] delegation`); `None` es el comportamiento de siempre.
    delegation: Option<crate::config::DelegationMode>,
    /// Techo de profundidad de la delegación (`[agents] max_depth`).
    depth_ceiling: u32,
    /// Enrutador de agentes (A12b); sin él no se elige agente automáticamente.
    router: Option<Arc<dyn crate::registry::router::AgentRouter>>,
    /// Volcado a disco de las salidas recortadas; `None` no guarda nada y no añade ruta.
    spill: Option<ToolSpill>,
    remote_clients: Option<Arc<dyn crate::remote_task::RemoteClientFactory>>,
    profile_provider_resolver: Option<Arc<ProfileProviderResolver>>,
    /// Presupuesto de gasto de la sesión; vacío no avisa ni lee el consumo acumulado. Los
    /// subagentes lo reciben y suman al mismo total (véase `budget_family`).
    budget: BudgetConfig,
    /// Sesión raíz del presupuesto compartido. En un hijo, la del padre: su consumo y el de sus
    /// hermanos suman al mismo acumulado. `None` en el agente raíz, que cuenta con su propia sesión.
    budget_family: Option<SessionId>,
    /// Configuración de la compactación (`[compaction]`). Por defecto, sin segundo plano. Solo
    /// afecta a este agente: el lanzador de subagentes no lo hereda.
    compaction: compact::CompactionConfig,
    /// Estado por sesión de la compactación en segundo plano. Es privado: los subagentes no lo
    /// comparten.
    background: background::BackgroundSlots,
}

#[cfg(test)]
mod tests;
