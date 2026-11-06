//! Pruebas de la vigilancia sobre `LocalJobRegistry`: jobs en marcha, `set_watch` y lotes.

use super::WatchSpec;
use crate::job::{JobId, JobRegistry, JobSpec, JobStatus, LocalJobRegistry};
use crate::session::SessionId;
use std::time::{Duration, Instant};

fn spec(pattern: &str, max_events: usize) -> WatchSpec {
    WatchSpec::parse(pattern, Some(max_events)).unwrap()
}

// --- LocalJobRegistry: vigilancia sobre jobs en marcha ----------------------------------

const OWNER: SessionId = SessionId(1);

fn job_spec(owner: SessionId, watch: Option<WatchSpec>, run: crate::job::JobBody) -> JobSpec {
    JobSpec {
        kind: "test".into(),
        label: "watched".into(),
        owner,
        output_limit: 64 * 1024,
        watch,
        run,
    }
}

#[tokio::test]
async fn watch_given_at_start_catches_the_first_lines() {
    let registry = LocalJobRegistry::default();
    let job = registry
        .start(job_spec(
            OWNER,
            Some(spec("FAIL", 20)),
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL early\nok\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    let batches = registry.take_watch_batches(OWNER, Instant::now());
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].1.lines, vec!["FAIL early".to_string()]);
    assert_eq!(batches[0].0.watch.as_ref().unwrap().delivered, 1);
}

/// Job de prueba que sincroniza con el test en dos puntos: `ready` avisa y `go` deja seguir.
fn gated_job(
    phases: Vec<(
        &'static str,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    )>,
) -> crate::job::JobBody {
    Box::new(move |handle, _| {
        Box::pin(async move {
            for (text, go, ready) in phases {
                let _ = go.await;
                handle.append(text);
                let _ = ready.send(());
            }
            Ok(None)
        })
    })
}

#[tokio::test]
async fn set_watch_only_sees_lines_after_it_is_set() {
    let registry = LocalJobRegistry::default();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (hold_tx, hold_rx) = tokio::sync::oneshot::channel::<()>();
    let job = registry
        .start(job_spec(
            OWNER,
            None,
            Box::new(move |handle, _| {
                Box::pin(async move {
                    handle.append("FAIL before\n");
                    let _ = ready_tx.send(());
                    // Se retiene el job hasta que la vigilancia esté puesta.
                    let _ = hold_rx.await;
                    handle.append("FAIL after\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    ready_rx.await.unwrap();
    // La salida ya escrita no se relee: la vigilancia nace después de «before».
    registry
        .set_watch(OWNER, job.id(), Some(spec("FAIL", 20)))
        .unwrap();
    hold_tx.send(()).ok();
    registry.wait(OWNER, job.id()).await.unwrap();
    let batches = registry.take_watch_batches(OWNER, Instant::now());
    assert_eq!(batches[0].1.lines, vec!["FAIL after".to_string()]);
}

#[tokio::test]
async fn set_watch_changes_the_pattern_in_flight() {
    let registry = LocalJobRegistry::default();
    let (go1_tx, go1_rx) = tokio::sync::oneshot::channel();
    let (ready1_tx, ready1_rx) = tokio::sync::oneshot::channel();
    let (go2_tx, go2_rx) = tokio::sync::oneshot::channel();
    let (ready2_tx, ready2_rx) = tokio::sync::oneshot::channel();
    let job = registry
        .start(job_spec(
            OWNER,
            None,
            gated_job(vec![
                ("FAIL one\nWARN one\n", go1_rx, ready1_tx),
                ("FAIL two\nWARN two\n", go2_rx, ready2_tx),
            ]),
        ))
        .unwrap();
    let now = Instant::now();
    registry
        .set_watch(OWNER, job.id(), Some(spec("FAIL", 20)))
        .unwrap();
    go1_tx.send(()).ok();
    ready1_rx.await.unwrap();
    let first = registry.take_watch_batches(OWNER, now);
    assert_eq!(first[0].1.lines, vec!["FAIL one".to_string()]);

    registry
        .set_watch(OWNER, job.id(), Some(spec("WARN", 20)))
        .unwrap();
    go2_tx.send(()).ok();
    ready2_rx.await.unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    let second = registry.take_watch_batches(OWNER, now);
    assert_eq!(second[0].1.lines, vec!["WARN two".to_string()]);
    assert_eq!(second[0].0.watch.as_ref().unwrap().pattern, "WARN");
}

#[tokio::test]
async fn set_watch_none_removes_the_watch() {
    let registry = LocalJobRegistry::default();
    let (go1_tx, go1_rx) = tokio::sync::oneshot::channel();
    let (ready1_tx, ready1_rx) = tokio::sync::oneshot::channel();
    let (go2_tx, go2_rx) = tokio::sync::oneshot::channel();
    let (ready2_tx, ready2_rx) = tokio::sync::oneshot::channel();
    let job = registry
        .start(job_spec(
            OWNER,
            Some(spec("FAIL", 20)),
            gated_job(vec![
                ("FAIL a\n", go1_rx, ready1_tx),
                ("FAIL b\n", go2_rx, ready2_tx),
            ]),
        ))
        .unwrap();
    let now = Instant::now();
    go1_tx.send(()).ok();
    ready1_rx.await.unwrap();
    // Lo ya casado se entrega antes de quitar la vigilancia; lo pendiente se descarta al cambiarla.
    let before = registry.take_watch_batches(OWNER, now);
    assert_eq!(before[0].1.lines, vec!["FAIL a".to_string()]);
    registry.set_watch(OWNER, job.id(), None).unwrap();
    go2_tx.send(()).ok();
    ready2_rx.await.unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    assert!(registry.take_watch_batches(OWNER, now).is_empty());
    assert!(registry.list(OWNER)[0].watch.is_none());
}

#[tokio::test]
async fn set_watch_rejects_unknown_foreign_and_finished_jobs() {
    let registry = LocalJobRegistry::default();
    let job = registry
        .start(job_spec(
            OWNER,
            None,
            Box::new(|_, _| Box::pin(async { Ok(None) })),
        ))
        .unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    let unknown = JobId("nope-9".into());
    assert!(registry.set_watch(OWNER, &unknown, None).is_err());
    assert!(registry
        .set_watch(SessionId(2), job.id(), Some(spec("FAIL", 1)))
        .is_err());
    let finished = registry.set_watch(OWNER, job.id(), Some(spec("FAIL", 1)));
    assert!(finished.is_err());
    assert!(finished.unwrap_err().to_string().contains("finished"));
}

#[tokio::test]
async fn finished_job_delivers_its_unterminated_rest_once() {
    let registry = LocalJobRegistry::default();
    let job = registry
        .start(job_spec(
            OWNER,
            Some(spec("FAIL", 20)),
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL no newline");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    let batches = registry.take_watch_batches(OWNER, Instant::now());
    assert_eq!(batches[0].1.lines, vec!["FAIL no newline".to_string()]);
    assert!(registry
        .take_watch_batches(OWNER, Instant::now() + Duration::from_secs(10))
        .is_empty());
}

/// Un job que se asienta dentro del intervalo de un lote anterior no debe retener sus líneas
/// pendientes: el aviso final es la última ocasión de entregarlas.
#[tokio::test]
async fn settled_job_pending_lines_are_not_held_by_the_interval() {
    let registry = LocalJobRegistry::default();
    let (go_tx, go_rx) = tokio::sync::oneshot::channel();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let job = registry
        .start(job_spec(
            OWNER,
            Some(spec("FAIL", 20)),
            Box::new(move |handle, _| {
                Box::pin(async move {
                    handle.append("FAIL a\n");
                    let _ = ready_tx.send(());
                    let _ = go_rx.await;
                    handle.append("FAIL b\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    ready_rx.await.unwrap();
    let now = Instant::now();
    let first = registry.take_watch_batches(OWNER, now);
    assert_eq!(first[0].1.lines, vec!["FAIL a".to_string()]);

    go_tx.send(()).ok();
    registry.wait(OWNER, job.id()).await.unwrap();
    // Dentro de los 5 s del lote anterior, pero el job ya está asentado.
    let tail = registry.take_watch_batches(OWNER, now + Duration::from_secs(1));
    assert_eq!(tail.len(), 1, "el resto de un job asentado debe salir ya");
    assert_eq!(tail[0].1.lines, vec!["FAIL b".to_string()]);
}

#[tokio::test]
async fn two_watched_jobs_keep_their_own_lines_and_limits() {
    let registry = LocalJobRegistry::default();
    let job_a = registry
        .start(job_spec(
            OWNER,
            Some(spec("A-FAIL", 1)),
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("A-FAIL 1\nA-FAIL 2\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    let job_b = registry
        .start(job_spec(
            OWNER,
            Some(spec("B-FAIL", 3)),
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("B-FAIL 1\nA-FAIL 3\nB-FAIL 2\nB-FAIL 3\nB-FAIL 4\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    registry.wait(OWNER, job_a.id()).await.unwrap();
    registry.wait(OWNER, job_b.id()).await.unwrap();
    let batches = registry.take_watch_batches(OWNER, Instant::now());
    assert_eq!(batches.len(), 2);
    let a = batches.iter().find(|(s, _)| s.id == *job_a.id()).unwrap();
    let b = batches.iter().find(|(s, _)| s.id == *job_b.id()).unwrap();
    assert_eq!(a.1.lines, vec!["A-FAIL 1".to_string()]);
    assert!(a.1.limit_reached);
    assert_eq!(b.1.lines, vec!["B-FAIL 1", "B-FAIL 2", "B-FAIL 3"]);
    assert!(b.1.limit_reached);
}

#[tokio::test]
async fn take_watch_batches_is_owner_scoped() {
    let registry = LocalJobRegistry::default();
    let job = registry
        .start(job_spec(
            OWNER,
            Some(spec("FAIL", 20)),
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL x\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    assert!(registry
        .take_watch_batches(SessionId(2), Instant::now())
        .is_empty());
    assert_eq!(registry.take_watch_batches(OWNER, Instant::now()).len(), 1);
}

#[tokio::test]
async fn killing_a_watched_job_leaves_no_live_watch_work() {
    let registry = LocalJobRegistry::default();
    let (go_tx, go_rx) = tokio::sync::oneshot::channel();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let job = registry
        .start(job_spec(
            OWNER,
            None,
            Box::new(move |handle, cancel| {
                Box::pin(async move {
                    let _ = ready_tx.send(());
                    let _ = go_rx.await;
                    handle.append("FAIL before kill\n");
                    cancel.cancelled().await;
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    ready_rx.await.unwrap();
    registry
        .set_watch(OWNER, job.id(), Some(spec("FAIL", 20)))
        .unwrap();
    go_tx.send(()).ok();
    registry.kill(OWNER, job.id()).unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(2), registry.wait(OWNER, job.id()))
        .await
        .expect("el job debe terminar tras kill")
        .unwrap();
    assert_eq!(outcome.status, JobStatus::Killed);

    let now = Instant::now();
    let batches = registry.take_watch_batches(OWNER, now);
    assert_eq!(batches[0].1.lines, vec!["FAIL before kill".to_string()]);
    assert!(registry
        .take_watch_batches(OWNER, now + Duration::from_secs(10))
        .is_empty());
    assert!(registry
        .set_watch(OWNER, job.id(), Some(spec("FAIL", 1)))
        .is_err());
}
