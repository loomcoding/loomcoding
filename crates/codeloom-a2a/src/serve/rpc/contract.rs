//! Contrato del servidor: límites, códigos de error, vista de tarea y anfitrión.

use super::*;

/// Tope del cuerpo de una petición.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Tope del texto de un mensaje.
pub const MAX_PROMPT_BYTES: usize = 32 * 1024;

pub(super) const MAX_ID_BYTES: usize = 200;

// Errores de JSON-RPC.
pub(super) const PARSE_ERROR: i64 = -32700;

pub(super) const INVALID_REQUEST: i64 = -32600;

pub(super) const METHOD_NOT_FOUND: i64 = -32601;

pub(super) const INVALID_PARAMS: i64 = -32602;

pub(super) const INTERNAL_ERROR: i64 = -32603;

// Errores de A2A (§5.4).
pub(super) const TASK_NOT_FOUND: i64 = -32001;

pub(super) const TASK_NOT_CANCELABLE: i64 = -32002;

pub(super) const PUSH_NOT_SUPPORTED: i64 = -32003;

pub(super) const UNSUPPORTED_OPERATION: i64 = -32004;

pub(super) const CONTENT_TYPE_NOT_SUPPORTED: i64 = -32005;

pub(super) const VERSION_NOT_SUPPORTED: i64 = -32009;

/// «Servidor ocupado»: código nuestro, A2A no tiene uno (rango de errores de servidor de JSON-RPC).
pub(super) const SERVER_BUSY: i64 = -32000;

/// Flujo de eventos de una tarea (`StreamResponse` en nombres del cable), listo para emitir por SSE.
pub type EventStream = BoxStream<'static, Value>;

/// Cada cuánto se emite un comentario SSE para que ningún intermediario cierre un flujo inactivo.
pub(super) const LATIDO: Duration = Duration::from_secs(15);

/// Una tarea tal como la ve el cable, con el dato que el enrutador necesita para decidir si espera.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskView {
    /// `Task` en nombres del cable.
    pub json: Value,
    /// `true` si el estado ya es terminal.
    pub terminal: bool,
}

/// Por qué el anfitrión no pudo atender una operación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// No existe, o no es de este servidor: no se distingue (§3.3.2).
    NotFound,
    NotCancelable,
    /// Demasiadas tareas vivas.
    Busy,
    /// La operación no es posible en este momento (p. ej. suscribirse a una tarea terminal) o este
    /// anfitrión no sabe hacerla. Se devuelve como `UnsupportedOperationError`.
    Unsupported,
    /// Fallo interno; el texto no se envía al cliente.
    Other(String),
}

/// Lo que el servidor necesita saber hacer con las tareas.
#[async_trait]
pub trait TaskHost: Send + Sync {
    /// Crea una tarea con este texto.
    async fn start(&self, prompt: String) -> Result<TaskView, HostError>;
    async fn start_for(
        &self,
        _tenant: Option<&str>,
        prompt: String,
    ) -> Result<TaskView, HostError> {
        self.start(prompt).await
    }
    /// Una tarea propia; `NotFound` para cualquier otra, exista o no.
    async fn get(&self, id: &str) -> Result<TaskView, HostError>;
    async fn get_for(&self, _tenant: Option<&str>, id: &str) -> Result<TaskView, HostError> {
        self.get(id).await
    }
    async fn cancel(&self, id: &str) -> Result<TaskView, HostError>;
    async fn cancel_for(&self, _tenant: Option<&str>, id: &str) -> Result<TaskView, HostError> {
        self.cancel(id).await
    }
    /// Espera a un estado terminal hasta `max`; si vence, devuelve la tarea como esté.
    async fn wait_terminal(&self, id: &str, max: Duration) -> Result<TaskView, HostError>;
    async fn wait_terminal_for(
        &self,
        _tenant: Option<&str>,
        id: &str,
        max: Duration,
    ) -> Result<TaskView, HostError> {
        self.wait_terminal(id, max).await
    }
    /// `SubscribeToTask`: flujo que empieza por la tarea y se cierra al estado terminal. Sin soporte,
    /// el valor por defecto es `Unsupported`.
    async fn subscribe(&self, _id: &str) -> Result<EventStream, HostError> {
        Err(HostError::Unsupported)
    }
    async fn subscribe_for(
        &self,
        _tenant: Option<&str>,
        id: &str,
    ) -> Result<EventStream, HostError> {
        self.subscribe(id).await
    }
    /// `SendStreamingMessage`: crea la tarea y devuelve su flujo desde el `Task` inicial.
    async fn start_streaming(&self, _prompt: String) -> Result<EventStream, HostError> {
        Err(HostError::Unsupported)
    }
    async fn start_streaming_for(
        &self,
        _tenant: Option<&str>,
        prompt: String,
    ) -> Result<EventStream, HostError> {
        self.start_streaming(prompt).await
    }
}

/// Estado compartido del servidor.
pub struct ServeState {
    pub host: Arc<dyn TaskHost>,
    pub auth: Authenticator,
    pub card: ServedCard,
    pub expected_tenant: Option<String>,
    pub agent_cards: Arc<std::sync::RwLock<std::collections::HashMap<String, ServedCard>>>,
    pub max_wait: Duration,
}
