//! Observabilidad de la vigilancia de jobs: la nota «watching: N alerts» aparece en el rail
//! (sección Jobs) y en la actividad del transcript, solo para los jobs que llevan `watch`.

use super::*;
use loomcoding_core::event::LiveEvent;
use loomcoding_core::job::{JobId, JobSnapshot, JobStatus};
use loomcoding_core::job_watch::WatchInfo;
use loomcoding_core::session::SessionId;

/// Job de la sesión 1 con la vigilancia dada (o sin ella).
fn job_con_vigilancia(label: &str, watch: Option<WatchInfo>) -> JobSnapshot {
    JobSnapshot {
        id: JobId(format!("job-{label}")),
        kind: "shell".into(),
        label: label.into(),
        owner: SessionId(1),
        status: JobStatus::Running,
        progress: None,
        output: String::new(),
        output_start: 0,
        output_end: 0,
        detail: None,
        result: None,
        watch,
    }
}

fn vigilancia(delivered: usize, suppressed: usize, limit_reached: bool) -> WatchInfo {
    WatchInfo {
        pattern: "FAIL".into(),
        delivered,
        suppressed,
        limit_reached,
    }
}

/// Crush con la sesión 1 abierta y los jobs dados aplicados como eventos (el transcript los
/// recibe también como actividad).
fn app_con_jobs(jobs: Vec<JobSnapshot>) -> App {
    let mut app = crush_ctx();
    app.session_id = Some(SessionId(1));
    for job in jobs {
        app.apply(LiveEvent::JobChanged(job));
    }
    app
}

/// Crush con los jobs solo en el registro que lee el rail: sin actividad en el transcript, así
/// que cualquier nota que aparezca en pantalla sale del rail.
fn app_solo_rail(jobs: Vec<JobSnapshot>) -> App {
    let mut app = crush_ctx();
    app.session_id = Some(SessionId(1));
    for job in jobs {
        app.jobs.insert(job.id.clone(), job);
    }
    app
}

#[test]
fn rail_shows_the_watch_note_of_a_watched_job() {
    let mut app = app_solo_rail(vec![job_con_vigilancia(
        "tests",
        Some(vigilancia(2, 1, false)),
    )]);
    let content = buffer_of(&mut app, 160, 40);
    assert!(content.contains("tests"), "{content}");
    // El rail parte la nota por palabras (puede quedar «+1» al final de una línea): se comprueban
    // las palabras, no la cadena entera.
    assert!(content.contains("watching: 2 alerts"), "{content}");
    assert!(content.contains("suppressed"), "{content}");
}

#[test]
fn rail_marks_a_reached_limit() {
    let mut app = app_solo_rail(vec![job_con_vigilancia(
        "tests",
        Some(vigilancia(20, 0, true)),
    )]);
    let content = buffer_of(&mut app, 160, 40);
    assert!(content.contains("watching: 20 alerts"), "{content}");
    assert!(content.contains("reached"), "{content}");
}

#[test]
fn unwatched_jobs_show_no_watch_note() {
    let mut app = app_solo_rail(vec![job_con_vigilancia("build", None)]);
    let content = buffer_of(&mut app, 160, 40);
    assert!(content.contains("build"), "{content}");
    assert!(!content.contains("watching:"), "{content}");
}

#[test]
fn transcript_activity_shows_the_watch_note_of_the_job() {
    let mut app = app_con_jobs(vec![job_con_vigilancia(
        "tests",
        Some(vigilancia(3, 0, false)),
    )]);
    // Sin rail, la nota solo puede venir de la actividad del transcript.
    app.rail_hidden = true;
    let content = buffer_of(&mut app, 160, 40);
    assert!(content.contains("watching: 3 alerts"), "{content}");
}
