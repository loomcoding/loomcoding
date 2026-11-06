use crate::app::Focus;
use crate::app::{
    AgentFilterEdit, AgentsFocus, App, ProviderSettings, SessionChoice, SettingsPicker,
    SETTINGS_EFFORT_FIELD,
};
use crate::frame::{FrameScheduler, Urgency, MIN_FRAME};
use crate::render::render;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
    KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, BeginSynchronizedUpdate, EndSynchronizedUpdate,
    EnterAlternateScreen, LeaveAlternateScreen,
};
use loomcoding_core::catalog::Catalog;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub type ActiveTurn = Arc<Mutex<Option<CancellationToken>>>;
const ESC_DOUBLE_PRESS: Duration = Duration::from_millis(350);

mod clipboard;
mod dialogs;
mod effort_picker_keys;
mod events;
mod keys;
mod model_picker_keys;
mod provider_picker_keys;
mod sidebar_keys;
mod slash;

use clipboard::*;
use dialogs::*;
use effort_picker_keys::*;
pub use events::*;
pub use keys::*;
use model_picker_keys::*;
use provider_picker_keys::*;
use slash::*;

#[derive(serde::Serialize, serde::Deserialize)]
pub enum SessionCommand {
    Refresh,
    Select(SessionChoice),
    CancelSubagent(loomcoding_core::session::SessionId),
    /// Parada de emergencia: cancela los hijos y los jobs de la sesión.
    KillAll,
    /// Contesta la pregunta pendiente (`input-required`) de un hijo.
    AnswerSubagent(loomcoding_core::session::SessionId, String),
    /// Lanza un agente por orden directa de la persona usuaria (`/agent` o `@`).
    RunAgent(crate::agent_command::AgentInvocation),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum SessionUpdate {
    List(Vec<loomcoding_core::session::SessionInfo>),
    Loaded(
        loomcoding_core::session::SessionId,
        Vec<loomcoding_core::message::Message>,
    ),
    /// Apuntes de consumo persistidos de la sesión (con sus hijos); llega tras `Loaded`.
    Usage(
        loomcoding_core::session::SessionId,
        Vec<loomcoding_core::session::UsageRecord>,
    ),
    Subagents(
        loomcoding_core::session::SessionId,
        Vec<loomcoding_core::session::SubagentDescriptor>,
    ),
    Workflow(
        loomcoding_core::session::SessionId,
        Option<loomcoding_core::workflow::WorkflowSummary>,
    ),
    Gauntlet(
        loomcoding_core::session::SessionId,
        Option<loomcoding_core::gauntlet::GauntletSummary>,
    ),
    Capabilities(crate::app::RuntimeCapabilities),
    Error(String),
}

pub struct ProviderSettingsCommand {
    pub settings: ProviderSettings,
    pub api_key: Option<String>,
    pub reply: oneshot::Sender<Result<bool, String>>,
}

/// Eventos de teclado que se procesan como máximo antes de pintar.
const MAX_BURST: usize = 512;

/// Restaura el terminal al salir, pase lo que pase (retorno normal, error o
/// panic): sin esto, un fallo en modo raw dejaba la terminal inutilizable.
struct RawGuard;

impl RawGuard {
    /// Entra en modo raw y en la pantalla alternativa. La captura del ratón solo
    /// se activa si `mouse` es `true`; sin ella, la selección nativa del terminal sigue disponible.
    fn enter(mouse: bool) -> std::io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self;
        execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
        )?;
        if mouse {
            execute!(std::io::stdout(), EnableMouseCapture)?;
        }
        Ok(guard)
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            EndSynchronizedUpdate,
            crossterm::cursor::Show,
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}

/// Bucle principal por eventos: teclado (prioritario), permisos, bus del
/// agente, vencimiento de frame y animaciones. Solo pinta cuando hay cambios;
/// las ráfagas de teclado se agrupan en un frame; cada frame es atómico.
// Los canales tienen tipos distintos y los coordina este bucle único.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    mut app: App,
    prompt_tx: UnboundedSender<crate::app::PendingPrompt>,
    // Canal de despertares del padre ocioso: el trabajador ejecuta un turno sin mensaje de usuario.
    wake_tx: UnboundedSender<()>,
    // Canal de compactaciones manuales (`/compact`): el trabajador resume la sesión abierta.
    compact_tx: UnboundedSender<()>,
    settings_tx: UnboundedSender<ProviderSettingsCommand>,
    mut bus_rx: tokio::sync::broadcast::Receiver<loomcoding_core::event::LiveEvent>,
    mut perm_rx: tokio::sync::mpsc::Receiver<(
        loomcoding_core::permission::PermissionRequest,
        crate::permission_flow::PermissionReply,
    )>,
    mut ask_rx: tokio::sync::mpsc::Receiver<(
        Vec<loomcoding_core::question::Question>,
        crate::permission_flow::QuestionReply,
    )>,
    active_turn: ActiveTurn,
    session_command_tx: UnboundedSender<SessionCommand>,
    mut session_update_rx: tokio::sync::mpsc::UnboundedReceiver<SessionUpdate>,
    // Catálogo de models.dev: llega en segundo plano y nunca bloquea el arranque.
    mut catalog_rx: tokio::sync::mpsc::UnboundedReceiver<Arc<Catalog>>,
) -> std::io::Result<()> {
    let _guard = RawGuard::enter(app.mouse_capture)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || loop {
        if input_tx.is_closed() {
            break;
        }
        match crossterm::event::poll(Duration::from_millis(50)) {
            Ok(true) => match crossterm::event::read() {
                Ok(event) => {
                    if input_tx.send(Ok(event)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = input_tx.send(Err(error));
                    break;
                }
            },
            Ok(false) => {}
            Err(error) => {
                let _ = input_tx.send(Err(error));
                break;
            }
        }
    });
    let mut frames = FrameScheduler::new(MIN_FRAME);
    let mut anim = tokio::time::interval(loomcoding_cloth::widgets::FRAME_INTERVAL);
    anim.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut fx: Option<loomcoding_cloth::widgets::fx::Effect> = None;
    let mut fx_clock = Instant::now();
    let (mut bus_open, mut perm_open, mut ask_open) = (true, true, true);
    let mut session_update_open = true;
    let mut catalog_open = true;
    let mut pending_settings: Option<oneshot::Receiver<Result<bool, String>>> = None;
    send_session_commands(&mut app, &session_command_tx);
    // Pintar antes de esperar eventos para que la TUI aparezca al arrancar.
    execute!(std::io::stdout(), BeginSynchronizedUpdate)?;
    terminal.draw(|f| render(f, &mut app))?;
    execute!(std::io::stdout(), EndSynchronizedUpdate)?;
    frames.drawn(Instant::now());

    while !app.should_quit {
        let animating = wants_clock(&app, fx.is_some(), Instant::now());
        // Sin reloj en reposo, el logo añade un único plazo: el de su próxima ráfaga (o el fin de
        // la actual). Así el bucle despierta a su hora sin ticks periódicos; durante una ráfaga,
        // el reloj de animación de abajo pinta los fotogramas.
        let deadline = earliest(frames.deadline(), app.logo_fx_deadline());
        tokio::select! {
            biased;
            ev = input_rx.recv() => match ev {
                Some(Ok(ev)) => {
                    let prompt = handle_event(&mut app, ev);
                    send_provider_settings(&mut app, &settings_tx, &mut pending_settings);
                    send(&mut app, &prompt_tx, prompt);
                    send_session_commands(&mut app, &session_command_tx);
                    cancel_active_turn(&mut app, &active_turn);
                    for _ in 0..MAX_BURST {
                        match input_rx.try_recv() {
                            Ok(Ok(ev)) => {
                                let prompt = handle_event(&mut app, ev);
                                send_provider_settings(&mut app, &settings_tx, &mut pending_settings);
                                send(&mut app, &prompt_tx, prompt);
                                send_session_commands(&mut app, &session_command_tx);
                                cancel_active_turn(&mut app, &active_turn);
                                if app.should_quit {
                                    break;
                                }
                            }
                            Ok(Err(error)) => return Err(error),
                            Err(_) => break,
                        }
                    }
                    dispatch_wake(&mut app, &wake_tx);
                    dispatch_compact(&mut app, &compact_tx);
                    frames.request(Urgency::Input);
                }
                Some(Err(error)) => return Err(error),
                None => break,
            },
            req = perm_rx.recv(), if perm_open => match req {
                Some((req, reply)) => {
                    app.enqueue_permission(req, reply);
                    frames.request(Urgency::Input);
                }
                None => perm_open = false,
            },
            ask = ask_rx.recv(), if ask_open => match ask {
                Some((questions, reply)) => {
                    app.enqueue_questions(questions, reply);
                    frames.request(Urgency::Input);
                }
                None => ask_open = false,
            },
            update = session_update_rx.recv(), if session_update_open => {
                match update {
                    Some(SessionUpdate::List(sessions)) => app.update_sessions(sessions),
                    Some(SessionUpdate::Loaded(session, messages)) => {
                        app.load_session(session, messages);
                    }
                    Some(SessionUpdate::Usage(session, records)) => {
                        app.load_usage(session, records);
                    }
                    Some(SessionUpdate::Subagents(session, subagents)) => {
                        app.load_subagents(session, subagents);
                    }
                    Some(SessionUpdate::Workflow(session, workflow)) => {
                        if let Some(workflow) = workflow {
                            app.apply(loomcoding_core::event::LiveEvent::DevelopmentWorkflowChanged(workflow));
                        }
                        let _ = session;
                    }
                    Some(SessionUpdate::Gauntlet(_session, gauntlet)) => {
                        // Al abrir la sesión se reenvía el estado sin actor ni detalle.
                        if let Some(summary) = gauntlet {
                            app.apply(loomcoding_core::event::LiveEvent::GauntletChanged {
                                summary,
                                actor: String::new(),
                                detail: String::new(),
                            });
                        }
                    }
                    Some(SessionUpdate::Capabilities(capabilities)) => {
                        app.runtime_capabilities = Some(capabilities);
                        app.capability_scroll = [0; 3];
                    }
                    Some(SessionUpdate::Error(error)) => {
                        app.session_load_failed();
                        app.status = format!("error: {error}");
                    }
                    None => {
                        session_update_open = false;
                        app.session_load_failed();
                        app.status = "error: session worker closed".into();
                    }
                }
                dispatch_next(&mut app, &prompt_tx);
                dispatch_wake(&mut app, &wake_tx);
                dispatch_compact(&mut app, &compact_tx);
                frames.request(Urgency::Input);
            },
            // Catálogo llegado en segundo plano: basta con repintar; no hay trabajo pesado aquí.
            catalog = catalog_rx.recv(), if catalog_open => match catalog {
                Some(catalog) => {
                    apply_catalog(&mut app, catalog);
                    frames.request(Urgency::Background);
                }
                None => catalog_open = false,
            },
            saved = async { pending_settings.as_mut().unwrap().await }, if pending_settings.is_some() => {
                pending_settings = None;
                app.settings_saved(saved.unwrap_or_else(|_| Err("Settings update failed".into())));
                dispatch_next(&mut app, &prompt_tx);
                dispatch_wake(&mut app, &wake_tx);
                dispatch_compact(&mut app, &compact_tx);
                frames.request(Urgency::Input);
            },
            ev = bus_rx.recv(), if bus_open => {
                match ev {
                    Ok(ev) => app.apply(ev),
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => bus_open = false,
                }
                while let Ok(ev) = bus_rx.try_recv() {
                    app.apply(ev);
                }
                dispatch_next(&mut app, &prompt_tx);
                dispatch_wake(&mut app, &wake_tx);
                dispatch_compact(&mut app, &compact_tx);
                frames.request(Urgency::Background);
            }
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(
                deadline.unwrap_or_else(Instant::now),
            )), if deadline.is_some() => {}
            _ = anim.tick(), if animating => {
                app.matrix_rain.advance();
                app.animation.advance();
                frames.request(Urgency::Background);
            },
        }

        cancel_active_turn(&mut app, &active_turn);
        // El diff de la sesión se pide y se recoge aquí, sin esperar: `git` corre en otro hilo.
        app.refresh_session_diff();
        app.poll_session_diff();
        if std::mem::take(&mut app.fx_pending) {
            fx = Some(loomcoding_cloth::widgets::fx::dialog_open(&app.theme));
            fx_clock = Instant::now();
        }
        if let Some(text) = app.clipboard.take() {
            use std::io::Write;
            let mut out = std::io::stdout();
            let _ = out.write_all(crate::clipboard::osc52(&text).as_bytes());
            let _ = out.flush();
        }
        if std::mem::take(&mut app.mouse_capture_dirty) {
            if app.mouse_capture {
                execute!(std::io::stdout(), EnableMouseCapture)?;
            } else {
                execute!(std::io::stdout(), DisableMouseCapture)?;
            }
        }
        // El planificador del logo avanza en cada pasada, también oculto (así su plazo no queda
        // vencido). Si el fotograma cambió, se repinta.
        if app.tick_logo_fx(Instant::now()) {
            frames.request(Urgency::Background);
        }

        let now = Instant::now();
        if frames.due(now) {
            let elapsed = now.duration_since(fx_clock);
            fx_clock = now;
            execute!(std::io::stdout(), BeginSynchronizedUpdate)?;
            terminal.draw(|f| {
                render(f, &mut app);
                if let (Some(e), Some(permission)) = (fx.as_mut(), app.permission.as_ref()) {
                    let area =
                        loomcoding_cloth::widgets::permission::area(f.area(), &permission.dialog);
                    e.process(elapsed.into(), f.buffer_mut(), area);
                }
            })?;
            execute!(std::io::stdout(), EndSynchronizedUpdate)?;
            if fx.as_ref().is_some_and(|e| e.done()) {
                fx = None;
            }
            frames.drawn(now);
        }
    }
    if let Ok(active) = active_turn.lock() {
        if let Some(token) = active.as_ref() {
            token.cancel();
        }
    }
    app.cancel_question();
    Ok(())
}

fn send_provider_settings(
    app: &mut App,
    tx: &UnboundedSender<ProviderSettingsCommand>,
    pending: &mut Option<oneshot::Receiver<Result<bool, String>>>,
) {
    if !std::mem::take(&mut app.settings_save_requested) || pending.is_some() {
        return;
    }
    let Some(settings) = app.provider_settings.clone() else {
        return;
    };
    let (reply, receiver) = oneshot::channel();
    let api_key = (!app.settings_api_key.is_empty()).then(|| app.settings_api_key.clone());
    match tx.send(ProviderSettingsCommand {
        settings,
        api_key,
        reply,
    }) {
        Ok(()) => {
            app.settings_saving = true;
            *pending = Some(receiver);
        }
        Err(_) => app.settings_feedback = Some("Settings worker is unavailable".into()),
    }
}

fn send_session_commands(app: &mut App, tx: &UnboundedSender<SessionCommand>) {
    if std::mem::take(&mut app.session_list_refresh_requested) {
        let _ = tx.send(SessionCommand::Refresh);
    }
    if let Some(choice) = app.session_choice.take() {
        let _ = tx.send(SessionCommand::Select(choice));
    }
    if std::mem::take(&mut app.kill_all_requested) {
        let _ = tx.send(SessionCommand::KillAll);
    }
    if let Some(child) = app.cancel_subagent_requested.take() {
        let _ = tx.send(SessionCommand::CancelSubagent(child));
    }
    if let Some(invocation) = app.run_agent_requested.take() {
        let _ = tx.send(SessionCommand::RunAgent(invocation));
    }
    if let Some((child, text)) = app.answer_subagent_requested.take() {
        let _ = tx.send(SessionCommand::AnswerSubagent(child, text));
    }
}

/// Indica si el reloj de 50 ms debe seguir avanzando. Además de los efectos, la lluvia, el
/// pensamiento, el proveedor y las herramientas que giran, cuenta el muñeco: mientras tenga
/// postura que mostrar (también `Done` y `Fail`), el reloj corre, y al vencer se para solo.
/// Una petición de diff en vuelo también lo mantiene, para recogerla al llegar. El logo cuenta
/// solo mientras tiene una ráfaga en pantalla.
fn wants_clock(app: &App, fx_active: bool, now: Instant) -> bool {
    (fx_active && app.permission.is_some())
        || app.matrix_rain.animating()
        || app.thinking
        || app.provider_in_flight > 0
        || app.session_diff.pending.is_some()
        || app.tools.iter().any(|tool| tool.is_spinning())
        || crate::mascot::pose_of(app, now).is_some()
        || app.logo_fx_frame.is_some()
}

/// El más próximo de dos plazos opcionales: el bucle despierta en cuanto venza el primero.
fn earliest(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) | (None, x) => x,
    }
}

#[cfg(test)]
mod tests;
