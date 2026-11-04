//! Constantes, capacidades del runtime, prompt pendiente, foco y utilidades de área.

use super::*;

pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

pub const MAX_IMAGE_BASE64_BYTES: usize = MAX_IMAGE_BYTES.div_ceil(3) * 4;

pub const MAX_PROMPT_IMAGES: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum McpConfigState {
    #[default]
    Unknown,
    NotConfigured,
    Loaded,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct McpServerCapability {
    pub id: String,
    pub initialized: bool,
    pub tool_count: usize,
    /// Tokens estimados de las definiciones de sus herramientas; cero si no se midieron.
    #[serde(default)]
    pub schema_tokens: u64,
    pub issue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillCapability {
    pub agent: String,
    pub id: String,
    pub name: String,
    pub available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeCapabilities {
    pub mcp_config: McpConfigState,
    pub mcp_servers: Vec<McpServerCapability>,
    pub lsp_registered: bool,
    pub rust_analyzer_available: bool,
    pub announced_skills: Vec<SkillCapability>,
    pub announced_skill_total: usize,
    pub available_skill_total: usize,
    pub declared_skill_scopes: Vec<(String, String)>,
    pub declared_skill_scope_total: usize,
    pub local_skill_loader_available: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PendingPrompt {
    pub text: String,
    pub images: Vec<ContentPart>,
}

impl From<String> for PendingPrompt {
    fn from(text: String) -> Self {
        Self {
            text,
            images: Vec::new(),
        }
    }
}

impl From<&str> for PendingPrompt {
    fn from(text: &str) -> Self {
        text.to_string().into()
    }
}

impl PartialEq<str> for PendingPrompt {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for PendingPrompt {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

/// Líneas a partir de las cuales se pliega un bloque largo de la conversación.
pub const MAX_BLOCK_LINES: usize = 10;

pub const MAX_A2A_OBSERVATIONS: usize = 128;

/// Transcript vacío con el pliegue de bloques largos activado.
pub(super) fn new_transcript() -> loomcoding_cloth::state::Transcript {
    let mut transcript = loomcoding_cloth::state::Transcript::new();
    transcript.set_fold_limit(Some(MAX_BLOCK_LINES));
    transcript
}

/// Dónde va el teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Composer,
    Transcript,
    /// Panel lateral izquierdo (look Classic, si se pinta).
    Sidebar,
    /// Panel «Agent» de métricas; «Token usage» comparte esta ventana.
    Agent,
    /// Panel «Activity» de herramientas (look Classic).
    Activity,
    /// Widgets de capacidades del panel derecho.
    Mcp,
    Lsp,
    Skills,
    /// Rail derecho del look Crush.
    Rail,
}

impl Focus {
    /// Ventanas laterales: no escriben, y una tecla imprimible las devuelve al composer.
    pub fn is_side_pane(self) -> bool {
        matches!(
            self,
            Focus::Sidebar
                | Focus::Agent
                | Focus::Activity
                | Focus::Mcp
                | Focus::Lsp
                | Focus::Skills
                | Focus::Rail
        )
    }
}

/// `true` si la celda (x, y) cae dentro del rectángulo (bordes derecho e inferior excluidos).
pub(super) fn rect_contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && y >= area.y
        && u32::from(x) < u32::from(area.x) + u32::from(area.width)
        && u32::from(y) < u32::from(area.y) + u32::from(area.height)
}

pub(crate) fn composer_text_area(area: Rect) -> Rect {
    Rect {
        width: if area.width > 1 {
            area.width - 1
        } else {
            area.width
        },
        ..area
    }
}

/// A quién va lo que se escribe en el composer: la sesión abierta o un hijo directo suyo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerTarget {
    /// La sesión que muestra la interfaz (el comportamiento de siempre).
    #[default]
    Open,
    /// Un hijo directo de la sesión abierta, sin cambiar de sesión.
    Child(SessionId),
}

/// Qué se hace con un mensaje dirigido a un hijo, según el estado A2A de su tarea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildDispatch {
    /// El hijo espera respuesta (`input-required`): el texto la contesta.
    Answer,
    /// El hijo trabaja (`submitted` o `working`): el texto espera su turno de entrada.
    Queue,
    /// El texto no tiene destino: la tarea pide credenciales o ya terminó.
    Reject,
}

/// Máximo de mensajes en espera por hijo; el que no cabe se queda en el borrador.
pub const MAX_CHILD_QUEUE: usize = 4;

/// Máximo de bytes de un mensaje dirigido a un hijo.
pub const MAX_CHILD_MESSAGE_BYTES: usize = 4_000;

/// Decide el despacho de un mensaje a un hijo. Una tarea terminal de A2A no se reinicia,
/// así que no admite mensajes nuevos: se continúa abriendo su sesión.
pub fn child_dispatch(state: TaskState) -> ChildDispatch {
    match state {
        TaskState::InputRequired => ChildDispatch::Answer,
        TaskState::Submitted | TaskState::Working => ChildDispatch::Queue,
        TaskState::AuthRequired
        | TaskState::Completed
        | TaskState::Failed
        | TaskState::Canceled
        | TaskState::Rejected => ChildDispatch::Reject,
    }
}
