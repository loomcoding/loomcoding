//! Pruebas del actor «job» en el transcript: el origen `Job` del mensaje decide el tipo de línea,
//! tanto en vivo como al reabrir la sesión. Los avisos antiguos (origen `Internal`) se siguen
//! reconociendo por su texto.

use super::*;
use loomcoding_cloth::state::{LineKind, ToolStatus};
use loomcoding_core::agent::compact::summary_message;

/// Líneas de tipo actor job del transcript, sin su prefijo.
fn job_lines(app: &App) -> Vec<String> {
    app.transcript
        .iter()
        .filter(|l| l.kind == LineKind::Job)
        .map(|l| l.plain().to_string())
        .collect()
}

/// Aviso de un job que llega en vivo por la sesión 1.
fn job_message_live(text: &str) -> LiveEvent {
    LiveEvent::SessionMessageAdded {
        session: SessionId(1),
        message: Message::from_job("3", text),
    }
}

#[test]
fn live_watch_notice_is_a_job_line() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(job_message_live("[Job 3 watch] FAIL uno"));
    assert_eq!(job_lines(&app), vec!["[Job 3 watch] FAIL uno".to_string()]);
    assert!(system_lines(&app).is_empty());
}

#[test]
fn the_origin_decides_even_when_the_text_is_not_recognised() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(job_message_live("redacción que nadie reconoce"));
    assert_eq!(
        job_lines(&app),
        vec!["redacción que nadie reconoce".to_string()]
    );
    assert!(system_lines(&app).is_empty());
}

#[test]
fn live_finished_job_notice_is_a_job_line() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(job_message_live("[Job 3 completed] build\nlisto"));
    assert!(job_lines(&app)
        .iter()
        .any(|t| t == "[Job 3 completed] build"));
    assert!(system_lines(&app).is_empty());
}

#[test]
fn reopened_session_paints_job_notices_as_job_lines() {
    let mut app = App::new();
    app.restore_messages(vec![
        Message::human("hola"),
        Message::from_job("3", "[Job 3 watch] FAIL uno"),
        Message::from_job("3", "[Job 3 completed] build\nlisto"),
    ]);
    assert_eq!(job_lines(&app)[0], "[Job 3 watch] FAIL uno");
    assert!(job_lines(&app)
        .iter()
        .any(|t| t == "[Job 3 completed] build"));
    assert!(system_lines(&app).is_empty());
}

#[test]
fn legacy_job_notice_with_internal_origin_is_still_recognised() {
    let mut app = App::new();
    app.restore_messages(vec![Message::user("[Job 3 watch] FAIL uno")]);
    assert_eq!(job_lines(&app), vec!["[Job 3 watch] FAIL uno".to_string()]);
    assert!(!texts(&app).iter().any(|t| t == "[Mensaje interno]"));
}

#[test]
fn subagent_compaction_and_internal_notices_stay_system_lines() {
    let mut app = App::new();
    app.restore_messages(vec![
        Message::user("[Subagent review completed]\nlisto"),
        summary_message(2, "resumen del tramo"),
        Message::user("texto interno sin formato"),
    ]);
    assert!(job_lines(&app).is_empty());
    let sistema = system_lines(&app);
    assert!(sistema
        .iter()
        .any(|t| t.starts_with("[Subagent review completed;")));
    assert!(sistema.iter().any(|t| t.starts_with("contexto compactado")));
    assert!(sistema.iter().any(|t| t == "[Mensaje interno]"));
}

#[test]
fn tool_inline_look_keeps_its_activity_for_finished_jobs() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(job_message_live("[Job 3 completed] build\nok"));
    assert_eq!(
        heads(&app),
        vec![("job  build".to_string(), ToolStatus::Success)]
    );
    assert!(job_lines(&app).is_empty());
}

#[test]
fn tool_inline_look_paints_watch_batches_as_job_lines() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(job_message_live("[Job 3 watch] FAIL uno"));
    assert!(heads(&app).is_empty());
    assert_eq!(job_lines(&app), vec!["[Job 3 watch] FAIL uno".to_string()]);
}
