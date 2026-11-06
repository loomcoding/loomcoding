//! Pruebas de `track_job` con la vigilancia: el padre recibe `JobWatchReady` cuando hay un lote
//! listo, en cada tick de `BATCH_INTERVAL` y al asentarse el job. Reloj pausado: los 5 s son
//! virtuales.

use super::*;
use crate::event::{EventBus, LiveEvent};
use crate::job::{JobRegistry, JobSpec, LocalJobRegistry};
use crate::job_watch::{WatchSpec, BATCH_INTERVAL};
use crate::permission::PermissionService;
use crate::session::SessionId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

const OWNER: SessionId = SessionId(31);

fn contexto(jobs: Arc<LocalJobRegistry>, bus: EventBus) -> ToolCtx {
    ToolCtx {
        cwd: std::env::current_dir().unwrap(),
        store: None,
        permission: Arc::new(PermissionService::allow_all()),
        session: OWNER,
        jobs,
        bus,
        subagents: None,
    }
}

/// Espera a un `JobWatchReady` del dueño. `false` si pasa `plazo` sin que llegue.
async fn espera_aviso(rx: &mut broadcast::Receiver<LiveEvent>, plazo: Duration) -> bool {
    tokio::time::timeout(plazo, async {
        loop {
            match rx.recv().await {
                Ok(LiveEvent::JobWatchReady { session }) if session == OWNER => return true,
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

fn job_vigilado(run: crate::job::JobBody) -> JobSpec {
    JobSpec {
        kind: "test".into(),
        label: "vigilado".into(),
        owner: OWNER,
        output_limit: 64 * 1024,
        watch: Some(WatchSpec::parse("FAIL", Some(20)).unwrap()),
        run,
    }
}

#[tokio::test(start_paused = true)]
async fn a_running_job_announces_its_ready_batch_on_a_tick() {
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let (go_tx, go_rx) = oneshot::channel::<()>();
    let job = jobs
        .start(job_vigilado(Box::new(move |handle, _| {
            Box::pin(async move {
                handle.append("FAIL uno\n");
                let _ = go_rx.await;
                Ok(None)
            })
        })))
        .unwrap();
    track_job(&contexto(jobs.clone(), bus), job.id()).unwrap();
    assert!(
        espera_aviso(&mut rx, Duration::from_secs(30)).await,
        "con el job en marcha, el tick debía avisar del lote listo"
    );
    go_tx.send(()).ok();
    jobs.wait(OWNER, job.id()).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_job_that_settles_with_pending_lines_announces_them_at_once() {
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    // Termina antes del primer tick con líneas: el aviso sale al asentarse, no en un tick.
    let job = jobs
        .start(job_vigilado(Box::new(|handle, _| {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                handle.append("FAIL final\n");
                Ok(None)
            })
        })))
        .unwrap();
    let began = tokio::time::Instant::now();
    track_job(&contexto(jobs.clone(), bus), job.id()).unwrap();
    assert!(espera_aviso(&mut rx, Duration::from_secs(30)).await);
    assert!(
        began.elapsed() < BATCH_INTERVAL,
        "el aviso del asentamiento debía salir antes del primer tick con avisos"
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_is_announced_without_matching_lines() {
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let (go_tx, go_rx) = oneshot::channel::<()>();
    let job = jobs
        .start(job_vigilado(Box::new(move |handle, _| {
            Box::pin(async move {
                handle.append("todo bien\n");
                let _ = go_rx.await;
                Ok(None)
            })
        })))
        .unwrap();
    track_job(&contexto(jobs.clone(), bus), job.id()).unwrap();
    assert!(
        !espera_aviso(&mut rx, Duration::from_secs(12)).await,
        "sin líneas que casen no hay nada que avisar"
    );
    go_tx.send(()).ok();
    jobs.wait(OWNER, job.id()).await.unwrap();
}
