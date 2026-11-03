//! Tipos del protocolo entre cliente y daemon: tramas, órdenes y eventos del servidor.

use super::*;

#[derive(Serialize, Deserialize)]
pub enum ClientFrame {
    Hello {
        version: u16,
        token: String,
    },
    Open {
        cwd: String,
        attach_agent: Option<String>,
        yolo: bool,
    },
    Print {
        cwd: String,
        prompt: String,
        yolo: bool,
    },
    Command(ClientCommand),
    Disconnect,
}

#[derive(Serialize, Deserialize)]
pub enum ClientCommand {
    Prompt(codeloom_tui::app::PendingPrompt),
    Wake,
    Compact,
    CancelTurn,
    RefreshAgents,
    Session(codeloom_tui::run::SessionCommand),
    ProviderSettings {
        id: u64,
        settings: codeloom_tui::app::ProviderSettings,
        api_key: Option<String>,
    },
    PermissionReply {
        id: u64,
        decision: codeloom_core::permission::Decision,
    },
    QuestionReply {
        id: u64,
        answers: Option<Vec<codeloom_core::question::Answer>>,
    },
    Disconnect,
}

#[derive(Serialize, Deserialize)]
pub enum ServerFrame {
    HelloAccepted { version: u16 },
    HelloRejected { message: String },
    Event(Box<ServerEvent>),
}

impl ServerFrame {
    pub(super) fn event(event: ServerEvent) -> Self {
        Self::Event(Box::new(event))
    }
}

#[derive(Serialize, Deserialize)]
pub enum ServerEvent {
    Ready {
        agent_id: String,
        runtime_generation: String,
        cwd: String,
        model: String,
        settings: codeloom_tui::app::ProviderSettings,
        api_key_configured: bool,
    },
    Capabilities(codeloom_tui::app::RuntimeCapabilities),
    /// Va en caja porque `LiveEvent` creció al añadir la fase y los contadores al
    /// descriptor de subagente, y sin indirección el enum entero engorda y clippy
    /// lo rechaza (`large_enum_variant`).
    Live(Box<codeloom_core::event::LiveEvent>),
    Session(codeloom_tui::run::SessionUpdate),
    SessionListPage {
        items: Vec<codeloom_core::session::SessionInfo>,
        done: bool,
    },
    SessionLoadedPage {
        session: SessionId,
        messages: Vec<codeloom_core::message::Message>,
        usage: Vec<codeloom_core::session::UsageRecord>,
        subagents: Vec<codeloom_core::session::SubagentDescriptor>,
        done: bool,
    },
    Catalog(codeloom_core::catalog::Catalog),
    PermissionRequest {
        id: u64,
        request: codeloom_core::permission::PermissionRequest,
    },
    Questions {
        id: u64,
        questions: Vec<codeloom_core::question::Question>,
    },
    ProviderSettingsResult {
        id: u64,
        result: Result<bool, String>,
    },
    PrintResult {
        result: Result<String, String>,
    },
    Error {
        message: String,
    },
}
