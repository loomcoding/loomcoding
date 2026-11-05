//! Postura del muñeco de estado según la situación de la aplicación.
//!
//! Cabe señalar que esta función es pura: recibe el estado y el instante actual, y devuelve la
//! postura y el texto corto de «qué hace». No dibuja nada; el dibujo es cosa de otra pieza.

use crate::app::App;
use loomcoding_cloth::state::ToolStatus;
use loomcoding_cloth::widgets::mascot::Pose;
use std::time::{Duration, Instant};

/// Tiempo que el muñeco se queda en `Fail` tras un turno con error.
const FAIL_HOLD: Duration = Duration::from_secs(10);
/// Tiempo que el muñeco se queda en `Done` tras un turno correcto.
const DONE_HOLD: Duration = Duration::from_secs(3);

/// Herramientas de exploración: su ejecución se muestra como caza en lugar de carrera.
fn is_search(name: &str) -> bool {
    matches!(name, "grep" | "glob" | "view" | "ls")
}

/// Postura y texto del muñeco, o `None` si no hay actividad que mostrar.
///
/// Las reglas se evalúan en orden y la primera que se cumple gana.
pub fn pose_of(app: &App, now: Instant) -> Option<(Pose, String)> {
    let waiting = app.question.is_some()
        || app.permission.is_some()
        || app
            .tools
            .iter()
            .any(|t| t.status() == ToolStatus::AwaitingPermission);
    if waiting {
        return Some((Pose::Wait, "waiting for you".into()));
    }

    if let Some((ended, false)) = app.last_turn_end {
        if now.saturating_duration_since(ended) < FAIL_HOLD {
            return Some((Pose::Fail, app.status.clone()));
        }
    }

    let running = || {
        app.tools
            .iter()
            .filter(|t| t.status() == ToolStatus::Running)
    };
    if let Some(tool) = running().find(|t| is_search(&t.name)) {
        return Some((Pose::Hunt, format!("searching · {}", tool.name)));
    }
    if let Some(tool) = running().next() {
        return Some((Pose::Run, format!("running · {}", tool.name)));
    }

    if app.thinking {
        return Some((Pose::Think, "thinking".into()));
    }
    if app.turn_in_flight {
        return Some((Pose::Think, "working".into()));
    }

    if let Some((ended, true)) = app.last_turn_end {
        if now.saturating_duration_since(ended) < DONE_HOLD {
            return Some((Pose::Done, "done".into()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use loomcoding_cloth::state::ToolItem;
    use loomcoding_core::permission::PermissionRequest;
    use loomcoding_core::question::{Question, QuestionOption};

    /// Herramienta en marcha con el nombre indicado.
    fn running(name: &str) -> ToolItem {
        let mut tool = ToolItem::new("call", name);
        tool.start();
        tool
    }

    /// Herramienta que espera permiso del usuario.
    fn awaiting(name: &str) -> ToolItem {
        let mut tool = running(name);
        tool.await_permission();
        tool
    }

    /// Pregunta de prueba; el receptor se devuelve para que el canal siga abierto.
    fn ask(
        app: &mut App,
    ) -> tokio::sync::oneshot::Receiver<Option<Vec<loomcoding_core::question::Answer>>> {
        let option = |label: &str| QuestionOption {
            label: label.into(),
            description: String::new(),
        };
        let question = Question {
            header: "Stack".into(),
            question: "Which?".into(),
            options: vec![option("a"), option("b")],
            multi_select: false,
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.enqueue_questions(vec![question], tx);
        rx
    }

    /// Ancla de tiempo fija: todas las pruebas miden desde aquí.
    fn anchor() -> Instant {
        Instant::now()
    }

    #[test]
    fn a_pending_question_makes_it_wait() {
        let mut app = App::new();
        let _rx = ask(&mut app);
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Wait, "waiting for you".into()))
        );
    }

    #[test]
    fn a_pending_permission_makes_it_wait() {
        let mut app = App::new();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        app.enqueue_permission(
            PermissionRequest {
                tool: "bash".into(),
                action: "run".into(),
                detail: r#"{"command":"cargo test"}"#.into(),
            },
            tx,
        );
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Wait, "waiting for you".into()))
        );
    }

    #[test]
    fn a_tool_awaiting_permission_makes_it_wait() {
        let mut app = App::new();
        app.tools.push_back(awaiting("bash"));
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Wait, "waiting for you".into()))
        );
    }

    #[test]
    fn a_failed_turn_shows_fail_for_ten_seconds() {
        let mut app = App::new();
        let ended = anchor();
        app.last_turn_end = Some((ended, false));
        app.status = "error: boom".into();
        assert_eq!(
            pose_of(&app, ended + Duration::from_secs(9)),
            Some((Pose::Fail, "error: boom".into()))
        );
        assert_eq!(pose_of(&app, ended + Duration::from_secs(11)), None);
    }

    #[test]
    fn a_running_search_tool_hunts() {
        for name in ["grep", "glob", "view", "ls"] {
            let mut app = App::new();
            app.tools.push_back(running(name));
            assert_eq!(
                pose_of(&app, anchor()),
                Some((Pose::Hunt, format!("searching · {name}"))),
                "herramienta {name}"
            );
        }
    }

    #[test]
    fn another_running_tool_runs() {
        let mut app = App::new();
        app.tools.push_back(running("bash"));
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Run, "running · bash".into()))
        );
    }

    #[test]
    fn thinking_makes_it_think() {
        let mut app = App::new();
        app.thinking = true;
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Think, "thinking".into()))
        );
    }

    #[test]
    fn a_turn_in_flight_without_tools_thinks() {
        let mut app = App::new();
        app.turn_in_flight = true;
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Think, "working".into()))
        );
    }

    #[test]
    fn a_finished_turn_shows_done_for_three_seconds() {
        let mut app = App::new();
        let ended = anchor();
        app.last_turn_end = Some((ended, true));
        assert_eq!(
            pose_of(&app, ended + Duration::from_secs(2)),
            Some((Pose::Done, "done".into()))
        );
        assert_eq!(pose_of(&app, ended + Duration::from_secs(4)), None);
    }

    #[test]
    fn idle_has_no_mascot() {
        let app = App::new();
        assert_eq!(pose_of(&app, anchor()), None);
    }

    #[test]
    fn question_wins_over_a_running_tool() {
        let mut app = App::new();
        app.tools.push_back(running("bash"));
        let _rx = ask(&mut app);
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Wait, "waiting for you".into()))
        );
    }

    #[test]
    fn a_running_tool_wins_over_thinking() {
        let mut app = App::new();
        app.thinking = true;
        app.tools.push_back(running("bash"));
        assert_eq!(
            pose_of(&app, anchor()),
            Some((Pose::Run, "running · bash".into()))
        );
    }
}
