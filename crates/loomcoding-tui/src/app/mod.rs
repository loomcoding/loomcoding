use crate::permission_flow::{dialog_for, spec_for, PermissionReply, QuestionReply};
use crate::rail::{agent_state, format_tokens};
use crate::tool_facts::tool_facts;
use loomcoding_cloth::widgets::composer::hit_index;
use loomcoding_core::agent::compact::parse_summary_text;
use loomcoding_core::budget::BudgetMetric;
use loomcoding_core::catalog::{Catalog, CatalogModel, CatalogProvider};
use loomcoding_core::config::{LoomcodingConfig, ProviderKind, UiConfig};
use loomcoding_core::event::LiveEvent;
use loomcoding_core::event::{A2aDirection, A2aObservation, A2aOperation};
use loomcoding_core::message::{ContentPart, Message, MessageOrigin, Role, Usage, UsageSource};
use loomcoding_core::permission::{Decision, PermissionRequest};
use loomcoding_core::question::{Answer, Question};
use loomcoding_core::session::{SessionId, SessionInfo, UsageRecord};
use loomcoding_core::task::{TaskState, TaskStatus};
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod activity;
mod agents;
mod child_types;
mod children;
mod composer;
mod events;
mod pointer;
mod prefs;
mod prompts;
mod session;
mod settings;
mod settings_types;
mod target;
mod tool_sync;
mod turn;
mod types;
mod usage;

pub use child_types::*;
pub use settings_types::*;
pub use types::*;
use usage::*;

/// Lo que el usuario tenía escrito en una sesión al apartarse de ella.
struct ComposerDraft {
    composer: loomcoding_cloth::state::Composer,
    images: Vec<ContentPart>,
}

pub struct App {
    /// Transcript persistente: se actualiza con los eventos, no por frame.
    pub transcript: loomcoding_cloth::state::Transcript,
    /// Entrada de la respuesta en curso (los deltas se añaden a ella).
    assistant: Option<loomcoding_cloth::state::EntryId>,
    pub composer: loomcoding_cloth::state::Composer,
    /// Imágenes del borrador, todavía no asignadas a un prompt en cola.
    pub draft_images: Vec<ContentPart>,
    /// Borradores de las sesiones que no están a la vista, uno por sesión.
    composer_drafts: BTreeMap<SessionId, ComposerDraft>,
    /// A quién va lo que se escribe: la sesión abierta o uno de sus hijos.
    pub composer_target: ComposerTarget,
    /// Respuesta a un hijo que pide entrada, pendiente de enviar al daemon.
    pub answer_subagent_requested: Option<(SessionId, String)>,
    /// Mensajes en espera por hijo, a la espera de que pida entrada.
    child_queue: BTreeMap<SessionId, VecDeque<String>>,
    /// Snapshot runtime recibido del daemon; `None` significa que aún no llegó.
    pub runtime_capabilities: Option<RuntimeCapabilities>,
    pub slash_selected: usize,
    pub slash_dismissed: Option<String>,
    pub slash_feedback: Option<String>,
    pub history: loomcoding_cloth::state::History,
    pub status: String,
    pub phase: String,
    phase_started_at: Option<Instant>,
    phase_elapsed: Duration,
    pub phase_durations: Vec<(String, Duration)>,
    pub should_quit: bool,
    pub cancel_requested: bool,
    pub cancel_subagent_requested: Option<SessionId>,
    /// Parada de emergencia pedida con `Ctrl+X` fuera del panel de la cola.
    pub kill_all_requested: bool,
    /// Agente que la persona usuaria pidió lanzar con `/agent` o `@`; lo recoge el bucle para enviarlo.
    pub run_agent_requested: Option<crate::agent_command::AgentInvocation>,
    pub(crate) last_escape: Option<Instant>,
    pub cwd: String,
    pub model: String,
    /// Mostrar avisos de proveedor en el transcript (request started/response received).
    /// Si `false`, se muestra un spinner en la barra de estado en su lugar.
    pub verbose_provider_notices: bool,
    pub theme: loomcoding_cloth::widgets::Theme,
    pub theme_preset: loomcoding_cloth::widgets::ThemePreset,
    /// Fichero donde se guardan las preferencias de la interfaz. `None` = no persistir.
    pub prefs_path: Option<std::path::PathBuf>,
    /// Captura del ratón (`/mouse`). Apagada, la terminal conserva su selección nativa.
    pub mouse_capture: bool,
    /// Hay un cambio de `mouse_capture` pendiente de aplicar al terminal.
    pub mouse_capture_dirty: bool,
    pub matrix_rain: loomcoding_cloth::widgets::matrix_rain::MatrixRain,
    pub animation: loomcoding_cloth::widgets::Anim,
    pub tools: VecDeque<loomcoding_cloth::state::ToolItem>,
    /// Si se invocó LSP durante la sesión visible.
    pub lsp_invoked: bool,
    /// Entrada del transcript de cada herramienta y las líneas con las que
    /// está sincronizada (usado por el look `Inline`).
    tool_entries: HashMap<
        String,
        (
            loomcoding_cloth::state::EntryId,
            Vec<loomcoding_cloth::state::Line>,
        ),
    >,
    /// Elementos de actividad (jobs y subagentes) del transcript del look Crush.
    activity_entries: Vec<ActivityEntry>,
    /// Lista de tareas de la sesión abierta, tal como la dejó el último `todo_write`.
    pub todos: Vec<loomcoding_core::todo::TodoItem>,
    /// Entrada del transcript de la lista de tareas y las líneas con las que
    /// está sincronizada.
    todo_entry: Option<(
        loomcoding_cloth::state::EntryId,
        Vec<loomcoding_cloth::state::Line>,
    )>,
    /// Llamadas `todo_write` con entrada en `tool_entries`; su tarjeta nunca se pinta.
    todo_tool_calls: HashSet<String>,
    pub jobs: BTreeMap<loomcoding_core::job::JobId, loomcoding_core::job::JobSnapshot>,
    pub subagents: BTreeMap<i64, loomcoding_core::session::SubagentDescriptor>,
    /// Métricas de los hijos de la sesión abierta, por id interno del hijo. Solo
    /// contiene hijos de esa sesión: se filtra al recibir y se limpia al cargarla.
    pub agent_stats: HashMap<i64, AgentStats>,
    /// Skills seleccionadas/cargadas por cada subagente local de la sesión.
    pub skill_statuses_by_child: HashMap<i64, Vec<loomcoding_core::skills::SkillLoadStatus>>,
    /// Registro acotado de cada hijo de la sesión abierta, por id interno del hijo. Se
    /// alimenta de los mismos `ChildEvent` que `agent_stats` y se limpia al cargar otra sesión.
    pub child_logs: HashMap<i64, ChildLog>,
    streaming_child_text: HashSet<i64>,
    /// Nodos de subagente desplegados, por id interno del hijo. Vacío significa que todos
    /// están plegados. Vive solo en memoria de la interfaz: no se persiste.
    pub expanded_nodes: HashSet<i64>,
    /// Llamadas del padre que apuntan a un hijo (`job_output`, `job_kill` o `subagent_control`),
    /// por id interno del hijo: el nombre de cada una, una entrada por llamada. Se muestran
    /// como `↳ parent: {herramienta}` bajo el nodo, en lugar de como líneas sueltas.
    pub node_actions: HashMap<i64, Vec<String>>,
    /// Llamada del padre (`call_id`) que ya se anotó en `node_actions`, con el hijo al que apunta.
    /// Sirve para que re-sincronizar la misma llamada no la cuente dos veces.
    node_calls: HashMap<String, i64>,
    /// Subagente resaltado en el rail (clave de `subagents`), si hay. Solo se mueve
    /// con el foco en el rail y se descarta si el hijo deja de existir.
    pub rail_selected: Option<i64>,
    /// Sesiones desde las que se entró en un subagente, de la más antigua a la más reciente.
    pub parent_stack: Vec<SessionId>,
    /// Navegación en curso (`open_child` o `go_parent`): la siguiente carga de sesión
    /// conserva la pila en vez de vaciarla.
    nav_keep_stack: bool,
    pub tools_open: bool,
    pub tool_selected: usize,
    pub tool_scroll: (u16, u16),
    pub activity_scroll: u16,
    pub capability_scroll: [u16; 3],
    pub a2a_open: bool,
    pub a2a_focus: usize,
    pub a2a_agent_selected: usize,
    pub a2a_task_selected: usize,
    pub a2a_event_selected: usize,
    pub a2a_detail_scroll: u16,
    pub a2a_observations: VecDeque<A2aObservation>,
    pub reasoning: String,
    pub thinking: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub turn_input_tokens: u64,
    pub turn_output_tokens: u64,
    /// Tokens de entrada de la última petición de la sesión mostrada: el tamaño real del contexto
    /// enviado. Alimenta el medidor del pie. Un uso con entrada 0 no lo pisa.
    pub context_tokens: u64,
    /// Tokens estimados del resumen vigente (la memoria del medidor); 0 si no hay resumen.
    pub context_memory: u64,
    /// Estado del resumen anticipado en segundo plano de la sesión mostrada.
    background_summary: BackgroundSummary,
    context_window_cache: RefCell<Option<ContextWindowCache>>,
    pub current_step: usize,
    pub max_steps: usize,
    pub changes: loomcoding_cloth::state::FileChanges,
    /// Ficheros editados por sesión (padre e hijos): sesión → ruta → (añadidas, quitadas).
    /// La última edición de cada ruta gana. Alimenta el rail y el panel de cambios al
    /// cargar una sesión; se vacía cuando la carga no es una navegación padre↔hijo.
    pub files_by_session: HashMap<i64, BTreeMap<String, (usize, usize)>>,
    /// Scroll del transcript (sigue el final salvo que el usuario suba).
    pub viewport: loomcoding_cloth::state::Viewport,
    pub layout: loomcoding_cloth::widgets::transcript::Layout,
    pub focus: Focus,
    /// Ventanas del último fotograma, en orden de pintado. `render` lo reescribe
    /// en cada frame reutilizando el mismo `Vec`.
    pub pane_rects: Vec<(Focus, Rect)>,
    /// Área del composer del último fotograma, si se pintó.
    pub composer_area: Option<Rect>,
    /// Ancla (en grafemas) del arrastre de ratón en curso dentro del composer.
    /// `None` si no hay arrastre; cualquier tecla o pegado lo anula.
    pub mouse_drag: Option<usize>,
    /// Líneas seleccionadas del transcript, si hay.
    pub selection: Option<loomcoding_cloth::state::Selection>,
    /// Selección de texto por caracteres del transcript, si la hay. Excluyente con
    /// `selection` y con la del composer: crear una anula las otras.
    pub text_sel: Option<loomcoding_cloth::state::TextSelection>,
    /// Ancho de texto del transcript cuando se creó `text_sel`. Si el diseño cambia
    /// de ancho, las columnas guardadas dejan de valer y la selección se descarta.
    pub text_sel_width: u16,
    /// Arrastre de ratón en curso sobre el transcript.
    pub text_drag: bool,
    /// Área en la que se pintó el texto del transcript en el último fotograma.
    pub transcript_area: Option<Rect>,
    /// Bloque activo del transcript: índice de entrada (`Transcript::entry`). Se
    /// navega con `[` y `]` y se fija con un clic; `None` si no hay ninguno.
    pub block: Option<usize>,
    /// Texto pendiente de enviar al portapapeles (lo consume el bucle).
    pub clipboard: Option<String>,
    /// Número de prompts pendientes en `pending_prompts`.
    pub queued: usize,
    /// Prompts pendientes; el worker recibe solo uno cada vez.
    pub pending_prompts: VecDeque<PendingPrompt>,
    pub queue_open: bool,
    pub queue_selected: usize,
    pub queue_paused: bool,
    pub queue_editing: bool,
    pub queue_editor: loomcoding_cloth::state::Composer,
    pub turn_in_flight: bool,
    /// Instante en que acabó el último turno y si acabó bien (`true`) o con error (`false`).
    /// Lo lee el muñeco de estado; se borra al empezar un turno nuevo.
    pub last_turn_end: Option<(Instant, bool)>,
    /// Hay un despertar pedido por un hijo (pregunta o cierre) que aún no se ha lanzado.
    pub wake_pending: bool,
    /// Despertar ya pedido al trabajador, sin turno iniciado ni `WakeIdle` de respuesta.
    pub wake_in_flight: bool,
    /// `/compact` pedido por la persona usuaria y aún no entregado al trabajador.
    pub compact_requested: bool,
    /// Compactación manual ya entregada al trabajador, sin `ManualCompactionEnded` de respuesta.
    /// Mientras dura, no se despachan prompts ni despertares: la compactación y un turno sobre la
    /// misma sesión no deben solaparse.
    pub compact_in_flight: bool,
    /// Causas `(hijo, motivo)` de los despertares pendientes desde el último turno despertado.
    /// Motivos: `"completed"`, `"failed"` o `"asks input"`. Solo son memoria de la interfaz.
    pub wake_causes: Vec<(i64, &'static str)>,
    /// Turnos automáticos que quedan antes de exigir un mensaje del usuario.
    pub auto_turns_left: u32,
    /// Diálogo de permiso abierto, si hay.
    pub permission: Option<PendingPermission>,
    /// Peticiones esperando a que se cierre la actual.
    permission_queue: VecDeque<(PermissionRequest, PermissionReply)>,
    /// Diálogo de preguntas (`ask_user`) abierto, si hay.
    pub question: Option<PendingQuestion>,
    /// Llamadas de preguntas esperando a que se cierre la actual.
    question_queue: VecDeque<(Vec<Question>, QuestionReply)>,
    /// `true` cuando se acaba de abrir un diálogo y falta lanzar su efecto.
    pub fx_pending: bool,
    /// Sessions list for the session picker UI.
    /// When empty, automatically triggers creation of a new session.
    /// When non-empty, shows existing sessions to resume.
    pub sessions: Vec<SessionInfo>,
    /// Index of the currently selected session in the picker.
    /// - 0 = first session
    /// - sessions.len() = "New session" option
    pub session_selected: usize,
    pub session_id: Option<SessionId>,
    pub session_query: String,
    pub session_searching: bool,
    pub session_picker_return_to_chat: bool,
    pub session_list_refresh_requested: bool,
    /// Whether the session picker overlay is open.
    /// - false: empty sessions → automatically creating new session
    /// - true: non-empty sessions → user can select existing or create new
    pub session_picker_open: bool,
    /// What the user chose in the picker (None = still choosing).
    /// Set when user presses Enter or 'n' in the session picker.
    pub session_choice: Option<SessionChoice>,
    /// Whether the sidebar is expanded (visible on all screen widths).
    pub sidebar_expanded: bool,
    /// Oculta el rail derecho del layout `ContextRail` (look Crush).
    pub rail_hidden: bool,
    /// `true` when loading a session (shows loading spinner).
    /// Set after user makes a selection and before messages are loaded.
    pub session_loading: bool,
    pub provider_settings: Option<ProviderSettings>,
    /// Niveles de esfuerzo declarados en `[provider] efforts`; solo valen si el catálogo no sabe los del modelo.
    pub declared_efforts: Vec<String>,
    /// Catálogo de proveedores y modelos de models.dev. Vacío hasta que llega la carga en segundo plano.
    pub catalog: Arc<Catalog>,
    /// Registro de agentes, para `/agents`. `None` si el binario no lo cablea.
    pub agent_registry: Option<Arc<loomcoding_core::registry::AgentRegistry>>,
    /// Pide al trabajador que vuelva a descargar las tarjetas de los remotos (`/refresh-agents`).
    /// `None` si el consumo de agentes remotos no está activo.
    refresh_agents_hook: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Apuntes de consumo de la sesión abierta: los persistidos al abrirla más los vivos.
    pub usage_records: Vec<UsageRecord>,
    /// Peticiones de provider activas por sesión y request id, con cifras provisionales y finales.
    active_usage: HashMap<(i64, u64), ActiveUsage>,
    /// Contador de peticiones de provider en vuelo (para spinner en status bar).
    pub provider_in_flight: usize,
    /// Desglose estimado de la última petición de la sesión mostrada; `None` hasta recibir uno.
    pub context_breakdown: Option<loomcoding_core::agent::compact::ContextBreakdown>,
    /// Copia de los ajustes al abrir el diálogo y tras guardar con éxito.
    /// Sirve para saber si hay cambios sin guardar.
    pub settings_baseline: Option<ProviderSettings>,
    /// `true` tras avisar de cambios sin guardar; una segunda `Esc` descarta.
    pub settings_warned: bool,
    pub settings_open: bool,
    /// Ventana `/temas` abierta: árbol Temas → Efectos → Matrix rain.
    pub temas_open: bool,
    /// Nivel actual del árbol de temas.
    pub temas_level: TemasLevel,
    /// Fila seleccionada dentro del nivel actual.
    pub temas_selected: usize,
    /// Copia editable de la config de la lluvia mientras la ventana está abierta.
    pub temas_matrix: loomcoding_cloth::widgets::matrix_rain::MatrixConfig,
    /// Spinner elegido en `/temas › Spinner`.
    pub spinner_choice: SpinnerChoice,
    /// `true` si la config en pantalla difiere de la guardada.
    pub temas_dirty: bool,
    pub settings_field: usize,
    pub settings_editing: bool,
    pub settings_editor: loomcoding_cloth::state::Composer,
    pub settings_api_key: String,
    pub settings_api_key_configured: bool,
    pub settings_save_requested: bool,
    pub settings_saving: bool,
    pub settings_feedback: Option<String>,
    /// Selector abierto sobre el campo de proveedor o de modelo, si lo hay.
    pub settings_picker: Option<SettingsPicker>,
    /// Texto con el que se filtra el selector abierto.
    pub picker_query: String,
    /// Índice de la fila resaltada dentro de `picker_items()`.
    pub picker_selected: usize,
    pub help_open: bool,
    /// Popup de `/agents` abierto, con el informe que mostraba el registro al abrirlo.
    pub agents_open: bool,
    pub agent_rows: Vec<AgentRow>,
    pub agents_selected: usize,
    /// Texto de búsqueda para filtrar agentes por nombre/descripción.
    pub agents_query: String,
    /// Buffer temporal para el tag que se está escribiendo en modo multiselect.
    pub agents_tag_input: String,
    /// Tags por los que filtrar agentes (multiselect).
    pub agents_tag_filter: Option<Vec<String>>,
    /// Qué filtro se está editando en el popup de agentes (None = navegación normal).
    pub agents_filter_edit: Option<AgentFilterEdit>,
    /// Qué parte de la ventana /agents tiene el foco (lista, query, tag).
    pub agents_focus: AgentsFocus,
    pub agent_edit: Option<AgentEditorDraft>,
    pub agent_editor: loomcoding_cloth::state::Composer,
    /// Resultado de la última aprobación o revocación, para pintarlo en el popup.
    pub agents_status: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            transcript: new_transcript(),
            assistant: None,
            composer: loomcoding_cloth::state::Composer::new(),
            draft_images: Vec::new(),
            composer_drafts: BTreeMap::new(),
            composer_target: ComposerTarget::Open,
            answer_subagent_requested: None,
            child_queue: BTreeMap::new(),
            runtime_capabilities: None,
            slash_selected: 0,
            slash_dismissed: None,
            slash_feedback: None,
            history: loomcoding_cloth::state::History::new(),
            status: "ready".into(),
            phase: "ready".into(),
            phase_started_at: None,
            phase_elapsed: Duration::ZERO,
            phase_durations: Vec::new(),
            should_quit: false,
            cancel_requested: false,
            cancel_subagent_requested: None,
            kill_all_requested: false,
            run_agent_requested: None,
            last_escape: None,
            cwd: String::new(),
            model: String::new(),
            verbose_provider_notices: false,
            theme: loomcoding_cloth::widgets::Theme::default_dark(),
            theme_preset: loomcoding_cloth::widgets::ThemePreset::Cloth,
            prefs_path: None,
            mouse_capture: true,
            mouse_capture_dirty: false,
            matrix_rain: loomcoding_cloth::widgets::matrix_rain::MatrixRain::default(),
            animation: loomcoding_cloth::widgets::Anim::new(),
            tools: VecDeque::new(),
            lsp_invoked: false,
            tool_entries: HashMap::new(),
            activity_entries: Vec::new(),
            todos: Vec::new(),
            todo_entry: None,
            todo_tool_calls: HashSet::new(),
            jobs: BTreeMap::new(),
            subagents: BTreeMap::new(),
            agent_stats: HashMap::new(),
            skill_statuses_by_child: HashMap::new(),
            child_logs: HashMap::new(),
            streaming_child_text: HashSet::new(),
            expanded_nodes: HashSet::new(),
            node_actions: HashMap::new(),
            node_calls: HashMap::new(),
            rail_selected: None,
            parent_stack: Vec::new(),
            nav_keep_stack: false,
            tools_open: false,
            tool_selected: 0,
            tool_scroll: (0, 0),
            activity_scroll: 0,
            capability_scroll: [0; 3],
            a2a_open: false,
            a2a_focus: 0,
            a2a_agent_selected: 0,
            a2a_task_selected: 0,
            a2a_event_selected: 0,
            a2a_detail_scroll: 0,
            a2a_observations: VecDeque::new(),
            reasoning: String::new(),
            thinking: false,
            input_tokens: 0,
            output_tokens: 0,
            turn_input_tokens: 0,
            turn_output_tokens: 0,
            context_tokens: 0,
            context_memory: 0,
            background_summary: BackgroundSummary::None,
            context_window_cache: RefCell::new(None),
            current_step: 0,
            max_steps: 0,
            changes: loomcoding_cloth::state::FileChanges::new(),
            files_by_session: HashMap::new(),
            viewport: loomcoding_cloth::state::Viewport::new(),
            layout: loomcoding_cloth::widgets::transcript::Layout::new(),
            focus: Focus::Composer,
            pane_rects: Vec::new(),
            composer_area: None,
            mouse_drag: None,
            selection: None,
            text_sel: None,
            text_sel_width: 0,
            text_drag: false,
            transcript_area: None,
            block: None,
            clipboard: None,
            queued: 0,
            pending_prompts: VecDeque::new(),
            queue_open: false,
            queue_selected: 0,
            queue_paused: false,
            queue_editing: false,
            queue_editor: loomcoding_cloth::state::Composer::new(),
            turn_in_flight: false,
            last_turn_end: None,
            wake_pending: false,
            wake_in_flight: false,
            compact_requested: false,
            compact_in_flight: false,
            wake_causes: Vec::new(),
            auto_turns_left: MAX_AUTO_TURNS,
            permission: None,
            permission_queue: VecDeque::new(),
            question: None,
            question_queue: VecDeque::new(),
            fx_pending: false,
            sessions: Vec::new(),
            session_selected: 0,
            session_id: None,
            session_query: String::new(),
            session_searching: false,
            session_picker_return_to_chat: false,
            session_list_refresh_requested: false,
            session_picker_open: false,
            session_choice: None,
            session_loading: false,
            provider_settings: None,
            declared_efforts: Vec::new(),
            catalog: Arc::new(Catalog::default()),
            agent_registry: None,
            refresh_agents_hook: None,
            usage_records: Vec::new(),
            active_usage: HashMap::new(),
            provider_in_flight: 0,
            context_breakdown: None,
            settings_baseline: None,
            settings_warned: false,
            settings_open: false,
            temas_open: false,
            temas_level: TemasLevel::Root,
            temas_selected: 0,
            temas_matrix: loomcoding_cloth::widgets::matrix_rain::MatrixConfig::default(),
            spinner_choice: SpinnerChoice::default(),
            temas_dirty: false,
            settings_field: 0,
            settings_editing: false,
            settings_editor: loomcoding_cloth::state::Composer::new(),
            settings_api_key: String::new(),
            settings_api_key_configured: false,
            settings_save_requested: false,
            settings_saving: false,
            settings_feedback: None,
            settings_picker: None,
            picker_query: String::new(),
            picker_selected: 0,
            help_open: false,
            agents_open: false,
            agent_rows: Vec::new(),
            agents_selected: 0,
            agents_query: String::new(),
            agents_tag_input: String::new(),
            agents_tag_filter: None,
            agents_filter_edit: None,
            agents_focus: AgentsFocus::default(),
            agent_edit: None,
            agent_editor: loomcoding_cloth::state::Composer::new(),
            agents_status: None,
            sidebar_expanded: false,
            rail_hidden: false,
        }
    }

    pub fn with_context(cwd: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            model: model.into(),
            ..Self::new()
        }
    }

    /// Aplica la sección `[ui]` de `loomcoding.toml` (avisos de proveedor, etc.).
    pub fn with_ui_config(mut self, ui: UiConfig) -> Self {
        self.verbose_provider_notices = ui.verbose_provider_notices;
        self
    }
}

#[cfg(test)]
mod tests;
