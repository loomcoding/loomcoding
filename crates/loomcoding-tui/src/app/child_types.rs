//! Notas de actividad, registros de hijos, uso activo, filas de agentes y cachés.

use super::*;

/// Petición de permiso en pantalla, con su canal de respuesta.
pub struct PendingPermission {
    pub dialog: loomcoding_cloth::state::PermissionDialog,
    pub(super) reply: PermissionReply,
}

/// Diálogo de preguntas abierto, con las preguntas originales y su canal de respuesta.
pub struct PendingQuestion {
    pub dialog: loomcoding_cloth::state::QuestionDialog,
    pub(super) questions: Vec<Question>,
    pub(super) reply: QuestionReply,
}

/// Elemento de actividad (job o subagente) del transcript en el look Crush.
pub(super) struct ActivityEntry {
    /// Clave estable: `job:{id}`, `subagent:{hijo}` o `notice:{n}`.
    pub(super) key: String,
    pub(super) id: loomcoding_cloth::state::EntryId,
    /// Líneas con las que está sincronizado el elemento, esté o no visible.
    pub(super) lines: Vec<loomcoding_cloth::state::Line>,
    pub(super) kind: &'static str,
    pub(super) label: String,
    /// `true` si el elemento nació de un aviso y no de un evento en vivo.
    pub(super) from_notice: bool,
    /// `true` si un aviso ya se asoció a este elemento en vivo.
    pub(super) notice_consumed: bool,
}

/// Aviso de texto de un job o subagente terminado, ya interpretado.
pub(super) enum Notice<'a> {
    Job {
        id: &'a str,
        status: loomcoding_cloth::state::ToolStatus,
        label: &'a str,
        body: &'a str,
    },
    /// Lote de vigilancia de un job en marcha. Va como línea de sistema, con el actor «job» en
    /// su cabecera; no abre ni cierra ningún elemento de actividad.
    JobWatch,
    Subagent {
        label: &'a str,
    },
    SubagentInput {
        label: &'a str,
        body: &'a str,
    },
    /// Resumen del historial que escribe el bucle al compactar. Lo emite Loomcoding.
    Compaction {
        covers: usize,
        body: String,
    },
}

/// Rótulo del aviso de compactación. Es una función de Loomcoding: no es un estado de A2A ni un
/// mensaje de la persona usuaria.
pub(super) fn compaction_label(covers: usize) -> String {
    if covers == 1 {
        "contexto compactado (1 mensaje resumido)".to_string()
    } else {
        format!("contexto compactado ({covers} mensajes resumidos)")
    }
}

/// Interpreta `[Job {id} {estado}] {etiqueta}\n{cuerpo}`, `[Job {id} watch] {líneas}` (lote de
/// vigilancia), `[Subagent {etiqueta} completed]\n{cuerpo}` y el resumen de compactación
/// `[Context summary: first {N} messages]\n{cuerpo}`. Devuelve `None` si el texto no
/// encaja con ninguno de los formatos.
pub(super) fn parse_notice(text: &str) -> Option<Notice<'_>> {
    if let Some((covers, body)) = parse_summary_text(text) {
        return Some(Notice::Compaction { covers, body });
    }
    let (header, body) = text.split_once('\n').unwrap_or((text, ""));
    if let Some(rest) = header.strip_prefix("[Job ") {
        let (inside, label) = rest.split_once(']')?;
        let mut parts = inside.split_whitespace();
        let (id, status) = (parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        // `[Job {id} watch] …`: lote de vigilancia de un job en marcha, no un estado terminal.
        if status == "watch" {
            return Some(Notice::JobWatch);
        }
        let status = match status {
            "completed" => loomcoding_cloth::state::ToolStatus::Success,
            "killed" => loomcoding_cloth::state::ToolStatus::Canceled,
            "failed" => loomcoding_cloth::state::ToolStatus::Error,
            _ => return None,
        };
        return Some(Notice::Job {
            id,
            status,
            label: label.trim(),
            body,
        });
    }
    let rest = header.strip_prefix("[Subagent ")?;
    let (inside, _) = rest.split_once(']')?;
    if let Some(label) = inside.strip_suffix(" completed") {
        return Some(Notice::Subagent { label });
    }
    inside
        .strip_suffix(" needs input")
        .map(|label| Notice::SubagentInput { label, body })
}

/// Estado visible de un job, según el mapeo de la especificación.
pub(super) fn job_tool_status(
    status: loomcoding_core::job::JobStatus,
) -> loomcoding_cloth::state::ToolStatus {
    use loomcoding_cloth::state::ToolStatus;
    use loomcoding_core::job::JobStatus;
    match status {
        JobStatus::Running | JobStatus::Stopping => ToolStatus::Running,
        JobStatus::Completed => ToolStatus::Success,
        JobStatus::Failed => ToolStatus::Error,
        JobStatus::Killed => ToolStatus::Canceled,
    }
}

/// Estado visible de un subagente, según el mapeo de la especificación.
pub(super) fn subagent_tool_status(state: TaskState) -> loomcoding_cloth::state::ToolStatus {
    use loomcoding_cloth::state::ToolStatus;
    // Varios estados de A2A comparten aspecto: la cabecera nombra el estado exacto, para que
    // esta agrupación no lo oculte (ver `sync_subagent_activity`).
    match state {
        TaskState::Submitted
        | TaskState::Working
        | TaskState::InputRequired
        | TaskState::AuthRequired => ToolStatus::Running,
        TaskState::Completed => ToolStatus::Success,
        TaskState::Failed | TaskState::Rejected => ToolStatus::Error,
        TaskState::Canceled => ToolStatus::Canceled,
    }
}

/// Anchura máxima de cada línea de la vista previa del último avance de un hijo.
pub(super) const PREVIEW_COLUMNS: usize = 100;

/// De 1 a 3 líneas de lo que el hijo va haciendo: su último avance publicado, con el nombre al principio de
/// la primera. Solo mientras la tarea sigue en marcha (en una terminada manda su resultado), sin saltos y
/// truncadas por anchura; vacío si no ha publicado nada.
pub(super) fn progress_preview(
    subagent: &loomcoding_core::session::SubagentDescriptor,
) -> Vec<String> {
    if subagent.status.state.is_terminal() {
        return Vec::new();
    }
    let Some(avance) = subagent.progress_artifacts().last() else {
        return Vec::new();
    };
    let texto = avance.text();
    let mut lineas = texto
        .lines()
        .map(str::trim)
        .filter(|linea| !linea.is_empty())
        .take(3)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lineas.is_empty() {
        return Vec::new();
    }
    if let Some(nombre) = avance.name.as_deref() {
        lineas[0] = format!("{nombre}: {}", lineas[0]);
    }
    lineas
        .into_iter()
        .map(|linea| {
            if linea.chars().count() > PREVIEW_COLUMNS {
                let corta: String = linea.chars().take(PREVIEW_COLUMNS - 1).collect();
                format!("{corta}…")
            } else {
                linea
            }
        })
        .collect()
}

/// Etiqueta del agente de un hijo: `[front]`; con origen de proyecto o remoto,
/// `[front·project]`, `[front·catalog]` o `[front·remote]`. Es política local de Loomcoding, no del protocolo A2A.
/// Devuelve `None` si el descriptor no conoce su agente, para no pintar corchetes vacíos.
pub(crate) fn agent_tag(subagent: &loomcoding_core::session::SubagentDescriptor) -> Option<String> {
    let agente = subagent
        .agent
        .as_deref()
        .filter(|nombre| !nombre.is_empty())?;
    let nivel = match subagent.agent_origin.as_deref() {
        Some(origen) if origen.starts_with("project:") => "·project",
        Some(origen) if origen.starts_with("catalog:") => "·catalog",
        Some(origen) if origen.starts_with("remote:") => "·remote",
        _ => "",
    };
    Some(format!("[{agente}{nivel}]"))
}

/// Profundidad máxima de la cadena de subagentes que la interfaz sigue (el techo duro del motor).
pub(super) const MAX_TREE_DEPTH: usize = 3;

/// Posición de `child` en el árbol que cuelga de la sesión `root`: su profundidad (1 para un hijo
/// directo) y las etiquetas desde el primer descendiente hasta él. `None` si la cadena de padres no
/// llega a `root` o supera la cota: un padre desconocido se descarta.
pub(crate) fn profundidad_y_camino(
    subagents: &BTreeMap<i64, loomcoding_core::session::SubagentDescriptor>,
    root: Option<SessionId>,
    child: i64,
) -> Option<(usize, Vec<String>)> {
    let root = root?;
    let mut camino = Vec::new();
    let mut actual = child;
    while camino.len() < MAX_TREE_DEPTH {
        let descriptor = subagents.get(&actual)?;
        camino.push(descriptor.label.clone());
        if descriptor.parent == root {
            camino.reverse();
            return Some((camino.len(), camino));
        }
        actual = descriptor.parent.0;
    }
    None
}

/// Etiqueta de un nodo anidado: el camino completo separado por `›`, truncado por la izquierda con
/// `…` si supera `max` caracteres, para que el último actor siga legible.
pub(crate) fn etiqueta_de_camino(camino: &[String], max: usize) -> String {
    let completo = camino.join(" › ");
    let total = completo.chars().count();
    if total <= max {
        return completo;
    }
    let cola: String = completo
        .chars()
        .skip(total - max.saturating_sub(1))
        .collect();
    format!("…{cola}")
}

/// Formato de un recuento por estado A2A exacto: `working 1, input-required 2`.
/// Recibe el recuento ya filtrado (solo los estados con cuenta mayor que cero).
pub fn formato_recuento(recuento: &[(TaskState, usize)]) -> String {
    recuento
        .iter()
        .map(|(estado, cuenta)| format!("{} {cuenta}", agent_state(*estado).label()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Herramienta con la que el modelo reemplaza la lista de tareas. Su llamada
/// no tiene tarjeta en el transcript: la lista se pinta como elemento propio.
pub(super) const TODO_TOOL: &str = "todo_write";

/// Turnos automáticos que puede encadenar el padre tras cada mensaje del usuario.
pub const MAX_AUTO_TURNS: u32 = 8;

/// Métricas vivas de un subagente. Se alimentan solo de `LiveEvent::ChildEvent`
/// y no se persisten: al cambiar de sesión desaparecen.
#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    /// Paso en curso del hijo dentro de su turno.
    pub step: usize,
    pub max_steps: usize,
    /// Herramienta que el hijo está ejecutando ahora, si hay alguna.
    pub tool: Option<String>,
    /// Tokens acumulados por el hijo; no suman al contador del turno del padre.
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
}

/// Número máximo de elementos que se conservan en el registro de un hijo. Al superarlo
/// se descarta el más antiguo, de modo que la memoria no crece con un hijo muy verboso.
pub const MAX_CHILD_ITEMS: usize = 200;

/// Longitud máxima, en caracteres, del texto de un mensaje del hijo en su registro.
pub(super) const MAX_CHILD_TEXT: usize = 240;

/// Longitud máxima, en caracteres, del resumen de una llamada a herramienta del hijo.
pub(super) const MAX_CHILD_SUMMARY: usize = 80;

/// Elemento del registro de un hijo: un texto del asistente o una llamada a herramienta.
#[derive(Debug, Clone, PartialEq)]
pub enum ChildItem {
    /// Texto del asistente del hijo, recortado a `MAX_CHILD_TEXT` caracteres.
    Text(String),
    /// Llamada a herramienta del hijo. `status` pasa de `Running` a `Success` o `Error`
    /// cuando llega su resultado.
    Tool {
        call_id: String,
        name: String,
        summary: Option<String>,
        status: loomcoding_cloth::state::ToolStatus,
    },
}

/// Registro acotado de lo que dice y hace un hijo. Se alimenta de los `ChildEvent` y,
/// como `AgentStats`, no se persiste: al cambiar de sesión desaparece.
#[derive(Debug, Clone, Default)]
pub struct ChildLog {
    pub items: VecDeque<ChildItem>,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveUsage {
    pub(super) model: String,
    pub(super) provider: Option<Usage>,
    pub(super) provider_final: bool,
    pub(super) estimate: Usage,
    pub(super) components: Vec<loomcoding_core::session::UsageComponent>,
}

impl ActiveUsage {
    pub(super) fn live_counts(&self) -> Usage {
        if self.provider_final {
            self.provider
                .clone()
                .unwrap_or_else(|| self.estimate.clone())
        } else {
            self.estimate.clone()
        }
    }

    pub(super) fn stored_counts(&self) -> (Usage, UsageSource) {
        match (&self.provider, self.provider_final) {
            (Some(usage), true) => (usage.clone(), UsageSource::Provider),
            _ => (self.estimate.clone(), UsageSource::Estimate),
        }
    }
}

/// Recorta `texto` a `maximo` caracteres. Cuenta caracteres, no bytes, de modo que no
/// parte ningún carácter multibyte.
pub(super) fn recortar(texto: &str, maximo: usize) -> String {
    texto.chars().take(maximo).collect()
}

/// Clave de la entrada de actividad de un subagente, por id interno del hijo.
pub(super) fn node_key(child: i64) -> String {
    format!("subagent:{child}")
}

/// Aplana `texto` a una sola línea: los saltos de línea pasan a espacio y se recorta el
/// sobrante de los extremos.
pub(super) fn una_linea(texto: &str) -> String {
    texto.replace(['\n', '\r'], " ").trim().to_string()
}

/// Una fila del popup de agentes: lo que se muestra de un agente del registro.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRow {
    pub name: String,
    /// Origen en texto estable (`built-in`, `global:<ruta>`, `project:<ruta>`, `remote:<url>`).
    pub origin: String,
    pub approval: loomcoding_core::registry::Approval,
    /// Modelo y esfuerzo propios del perfil, o la herencia.
    pub execution: String,
    pub description: String,
    pub version: String,
    pub card_skills: Vec<loomcoding_core::card::AgentSkill>,
    pub scope: loomcoding_core::registry::profile::AgentScope,
    pub profile_path: Option<std::path::PathBuf>,
    pub content_hash: Option<String>,
    pub editable: bool,
    pub capabilities: loomcoding_core::card::AgentCapabilities,
    /// Prompt de sistema del perfil: lo que la persona usuaria revisa antes de aprobar.
    pub body: String,
}

/// Texto de ejecución de una fila y prompt a revisar. De un remoto, además, el motivo por el que no
/// se puede usar (falta una credencial, esquema no soportado…) para que la persona sepa qué hacer
/// sin abrir un registro. Ese motivo se acota y se aplana: puede incluir nombres de un tercero.
pub(super) fn execution_label(
    execution: &loomcoding_core::registry::Execution,
) -> (String, String) {
    use loomcoding_core::registry::Execution;
    const MAX_MOTIVO: usize = 120;
    match execution {
        Execution::Local(local) => (
            format!(
                "model {} / effort {}",
                local.model.as_deref().unwrap_or("inherits"),
                local.effort.as_deref().unwrap_or("inherits")
            ),
            local.system_prompt(),
        ),
        Execution::Remote(spec) => {
            let motivo = spec
                .fetched
                .as_ref()
                .and_then(|descargada| descargada.auth_problem.as_deref())
                .map(una_linea);
            let texto = match motivo {
                Some(motivo) => format!(
                    "remote · auth: {}",
                    motivo.chars().take(MAX_MOTIVO).collect::<String>()
                ),
                None => "remote".to_string(),
            };
            (texto, String::new())
        }
    }
}

impl AgentRow {
    pub(super) fn from_entry(
        entrada: &loomcoding_core::registry::AgentEntry,
        instantanea: &loomcoding_core::registry::RegistrySnapshot,
    ) -> Self {
        let (execution, body) = execution_label(&entrada.execution);
        AgentRow {
            name: entrada.name.clone(),
            origin: entrada.origin.as_text(),
            approval: instantanea
                .approval(&entrada.name)
                .unwrap_or(loomcoding_core::registry::Approval::Pending),
            execution,
            description: entrada.card.description.trim().to_string(),
            version: entrada.card.version.clone(),
            card_skills: entrada.card.skills.clone(),
            scope: match &entrada.execution {
                loomcoding_core::registry::Execution::Local(local) => local.scope.clone(),
                loomcoding_core::registry::Execution::Remote(_) => Default::default(),
            },
            profile_path: match &entrada.origin {
                loomcoding_core::registry::AgentOrigin::File { path, .. }
                    if matches!(
                        &entrada.execution,
                        loomcoding_core::registry::Execution::Local(_)
                    ) =>
                {
                    Some(path.clone())
                }
                _ => None,
            },
            content_hash: entrada.content_hash.clone(),
            editable: matches!(
                (&entrada.origin, &entrada.execution),
                (
                    loomcoding_core::registry::AgentOrigin::File { .. },
                    loomcoding_core::registry::Execution::Local(_)
                )
            ),
            capabilities: entrada.card.capabilities.clone(),
            body,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentEditorDraft {
    pub description: String,
    pub version: String,
    /// Una skill por línea: id | nombre | descripción | tags | ejemplos | input modes | output modes.
    pub card_skills: String,
    pub mcp: String,
    pub local_skills: String,
    pub tools: String,
    pub field: usize,
    pub editing_value: bool,
}

/// Qué filtro de la ventana /agents se está editando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentFilterEdit {
    Query,
    Tag,
}

/// Qué parte de la ventana /agents tiene el foco para navegación con Tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentsFocus {
    #[default]
    List,
    Query,
    Tag,
}

#[derive(Debug, Clone)]
pub struct A2aAgentView {
    pub name: String,
    pub card: loomcoding_core::card::AgentCard,
    pub origin: String,
    pub approval: Option<loomcoding_core::registry::Approval>,
    pub internal: bool,
    pub enabled: bool,
    pub execution: Option<loomcoding_core::registry::Execution>,
}

/// Ventana de contexto ya buscada en el catálogo, con las entradas que la determinan. Mientras
/// no cambie ninguna, el pie reutiliza el valor en vez de recorrer el catálogo en cada fotograma.
pub(super) struct ContextWindowCache {
    pub(super) catalog: Arc<Catalog>,
    pub(super) base_url: String,
    pub(super) model: String,
    pub(super) window: Option<u64>,
}

/// Estado del resumen anticipado de la compactación en segundo plano, según los eventos del núcleo.
/// `Running` guarda el instante de arranque para contar los segundos al pintar, sin temporizadores.
/// `Ready` marca que hay un resumen listo para activarse («ahorro pendiente» en el medidor).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum BackgroundSummary {
    #[default]
    None,
    Running {
        since: Instant,
    },
    Ready,
}

/// Estimación de tokens de un texto de `bytes` bytes: cuatro bytes por token, redondeo hacia arriba.
pub(super) fn estimate_tokens(bytes: usize) -> u64 {
    bytes.div_ceil(4) as u64
}
