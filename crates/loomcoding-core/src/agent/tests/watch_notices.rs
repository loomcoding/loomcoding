//! Entrega de los lotes de vigilancia (`watch`) como mensajes de la conversación: una sola vez,
//! persistidos, publicados en el bus y sin mezclar jobs.

use super::*;
use crate::job::{JobRegistry, JobResult, JobSpec, LocalJobRegistry};
use crate::job_watch::WatchSpec;
use tokio::sync::oneshot;

/// Agente de prueba con el registro de jobs dado y un bus propio.
fn agente_con_jobs(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    bus: EventBus,
    jobs: Arc<LocalJobRegistry>,
) -> AgentLoop {
    AgentLoop::new(
        Arc::new(QueueProvider {
            scripts: Mutex::new(VecDeque::new()),
        }),
        store,
        Arc::new(ToolRegistry::new()),
        bus,
        Arc::new(PermissionService::allow_all()),
        LoomcodingConfig {
            provider: crate::config::ProviderConfig {
                kind: crate::config::ProviderKind::OpenAiCompat,
                base_url: "http://x".into(),
                api_key_env: "K".into(),
            },
            model: "m".into(),
            max_steps: 5,
            effort: None,
        },
        dir.to_path_buf(),
    )
    .with_jobs(jobs)
}

/// Job del dueño `sid` con vigilancia del patrón dado.
fn job_vigilado(
    sid: SessionId,
    patron: &str,
    max_events: usize,
    run: crate::job::JobBody,
) -> JobSpec {
    JobSpec {
        kind: "test".into(),
        label: "vigilado".into(),
        owner: sid,
        output_limit: 64 * 1024,
        watch: Some(WatchSpec::parse(patron, Some(max_events)).unwrap()),
        run,
    }
}

/// Textos de los avisos de vigilancia persistidos en la sesión.
async fn textos_de_vigilancia(store: &SqliteStore, sid: SessionId) -> Vec<String> {
    store
        .messages(sid)
        .await
        .unwrap()
        .iter()
        .map(|m| m.text())
        .filter(|texto| texto.starts_with("[Job ") && texto.contains(" watch]"))
        .collect()
}

/// Textos de los `SessionMessageAdded` publicados para la sesión hasta ese momento.
fn publicados(rx: &mut broadcast::Receiver<LiveEvent>, sid: SessionId) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let LiveEvent::SessionMessageAdded { session, message } = event {
            if session == sid {
                out.push(message.text());
            }
        }
    }
    out
}

async fn almacen(dir: &std::path::Path, nombre: &str) -> Arc<SqliteStore> {
    Arc::new(
        SqliteStore::open(dir.join(nombre).to_str().unwrap())
            .await
            .unwrap(),
    )
}

#[tokio::test]
async fn a_watch_batch_is_delivered_once_persisted_and_published() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "una_vez.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let agent = agente_con_jobs(dir.path(), store.clone(), bus, jobs.clone());
    let (go_tx, go_rx) = oneshot::channel::<()>();
    let (ready_tx, ready_rx) = oneshot::channel::<()>();
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            20,
            Box::new(move |handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    let _ = ready_tx.send(());
                    // El job sigue en marcha: no hay aviso de fin todavía.
                    let _ = go_rx.await;
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    ready_rx.await.unwrap();

    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 1);
    // Sin líneas nuevas, el mismo lote no vuelve a salir.
    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 0);

    let esperado = format!("[Job {} watch] FAIL uno", job.id().0);
    assert_eq!(
        textos_de_vigilancia(&store, sid).await,
        vec![esperado.clone()]
    );
    assert!(publicados(&mut rx, sid).contains(&esperado));

    go_tx.send(()).ok();
    jobs.wait(sid, job.id()).await.unwrap();
    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 1);
    assert_eq!(textos_de_vigilancia(&store, sid).await, vec![esperado]);
}

#[tokio::test]
async fn the_final_job_notice_still_arrives_after_the_watch_lines() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "final.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_jobs(dir.path(), store.clone(), EventBus::new(64), jobs.clone());
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(Some(JobResult {
                        content: "listo".into(),
                    }))
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 2);
    let textos: Vec<String> = store
        .messages(sid)
        .await
        .unwrap()
        .iter()
        .map(|m| m.text())
        .collect();
    assert!(textos.contains(&format!("[Job {} watch] FAIL uno", job.id().0)));
    assert!(textos.contains(&format!("[Job {} completed] vigilado\nlisto", job.id().0)));
}

#[tokio::test]
async fn reaching_max_events_announces_the_watch_limit() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "limite.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_jobs(dir.path(), store.clone(), EventBus::new(64), jobs.clone());
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            1,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\nFAIL dos\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    agent.flush_subagent_notices(sid).await.unwrap();
    assert_eq!(
        textos_de_vigilancia(&store, sid).await,
        vec![format!(
            "[Job {} watch] FAIL uno\nwatch limit reached",
            job.id().0
        )]
    );
}

#[tokio::test]
async fn a_batch_reports_the_lines_it_summarised() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "suprimidas.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_jobs(dir.path(), store.clone(), EventBus::new(64), jobs.clone());
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    let texto: String = (0..12).map(|i| format!("FAIL {i}\n")).collect();
                    handle.append(&texto);
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    agent.flush_subagent_notices(sid).await.unwrap();
    let lineas: String = (1..10).map(|i| format!("\nFAIL {i}")).collect();
    assert_eq!(
        textos_de_vigilancia(&store, sid).await,
        vec![format!(
            "[Job {} watch] FAIL 0{lineas}\n+2 more suppressed",
            job.id().0
        )]
    );
}

#[tokio::test]
async fn two_watched_jobs_do_not_mix_their_lines() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "dos_jobs.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_jobs(dir.path(), store.clone(), EventBus::new(64), jobs.clone());
    let job_a = jobs
        .start(job_vigilado(
            sid,
            "A-FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("A-FAIL 1\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    let job_b = jobs
        .start(job_vigilado(
            sid,
            "B-FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("B-FAIL 1\nA-FAIL 9\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job_a.id()).await.unwrap();
    jobs.wait(sid, job_b.id()).await.unwrap();

    agent.flush_subagent_notices(sid).await.unwrap();
    let vigilancia = textos_de_vigilancia(&store, sid).await;
    assert_eq!(vigilancia.len(), 2);
    let de_a = vigilancia
        .iter()
        .find(|t| t.starts_with(&format!("[Job {} watch]", job_a.id().0)))
        .expect("el job A debe tener su aviso");
    let de_b = vigilancia
        .iter()
        .find(|t| t.starts_with(&format!("[Job {} watch]", job_b.id().0)))
        .expect("el job B debe tener su aviso");
    assert!(de_a.contains("A-FAIL 1") && !de_a.contains("B-FAIL"));
    assert!(de_b.contains("B-FAIL 1") && !de_b.contains("A-FAIL"));
}

#[tokio::test]
async fn a_job_without_watch_adds_no_watch_message() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "sin_vigilancia.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_jobs(dir.path(), store.clone(), EventBus::new(64), jobs.clone());
    let job = jobs
        .start(JobSpec {
            kind: "test".into(),
            label: "sin vigilar".into(),
            owner: sid,
            output_limit: 1024,
            watch: None,
            run: Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(None)
                })
            }),
        })
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 1);
    assert!(textos_de_vigilancia(&store, sid).await.is_empty());
}

/// Guion de un turno con una sola respuesta de texto.
fn respuesta(texto: &str) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(texto.into()),
        StreamChunk::Done(FinishReason::Stop),
    ]
}

/// Agente con un proveedor de guion dado y el registro de jobs dado.
fn agente_con_guion(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    jobs: Arc<LocalJobRegistry>,
    guion: Vec<Vec<StreamChunk>>,
) -> AgentLoop {
    AgentLoop::new(
        Arc::new(QueueProvider {
            scripts: Mutex::new(guion.into()),
        }),
        store,
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        LoomcodingConfig {
            provider: crate::config::ProviderConfig {
                kind: crate::config::ProviderKind::OpenAiCompat,
                base_url: "http://x".into(),
                api_key_env: "K".into(),
            },
            model: "m".into(),
            max_steps: 5,
            effort: None,
        },
        dir.to_path_buf(),
    )
    .with_jobs(jobs)
}

#[tokio::test]
async fn a_woken_turn_delivers_the_watch_batch() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "despertado.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_guion(
        dir.path(),
        store.clone(),
        jobs.clone(),
        vec![respuesta("visto")],
    );
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    // El despertar entrega el lote y el turno que se lanza ya lo ve.
    let outcome = agent
        .run_woken(sid, CancellationToken::new())
        .await
        .unwrap()
        .expect("un lote pendiente debía producir un turno");
    assert_eq!(outcome.final_text, "visto");
    assert!(textos_de_vigilancia(&store, sid)
        .await
        .contains(&format!("[Job {} watch] FAIL uno", job.id().0)));
    // Con todo entregado, un nuevo despertar no gasta turno.
    assert!(agent
        .run_woken(sid, CancellationToken::new())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn a_batch_left_undelivered_reaches_the_next_user_turn() {
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "siguiente_turno.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = agente_con_guion(
        dir.path(),
        store.clone(),
        jobs.clone(),
        vec![respuesta("contestado")],
    );
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    // Sin despertar (presupuesto agotado), el lote espera: el turno del usuario lo entrega.
    agent
        .run(sid, "¿qué ha pasado?".into(), CancellationToken::new())
        .await
        .unwrap();
    assert!(textos_de_vigilancia(&store, sid)
        .await
        .contains(&format!("[Job {} watch] FAIL uno", job.id().0)));
}

/// Último recuento de vigilancia publicado para el job en el bus.
fn ultimo_recuento_publicado(
    rx: &mut broadcast::Receiver<LiveEvent>,
    id: &crate::job::JobId,
) -> Option<crate::job_watch::WatchInfo> {
    let mut ultimo = None;
    while let Ok(event) = rx.try_recv() {
        if let LiveEvent::JobChanged(snapshot) = event {
            if &snapshot.id == id {
                ultimo = snapshot.watch;
            }
        }
    }
    ultimo
}

#[tokio::test]
async fn a_settled_job_publishes_the_count_of_the_batch_it_delivered() {
    // Un job corto termina antes de que se entregue su lote: el bucle de seguimiento ya no escucha,
    // así que el recuento entregado debe republicarse al entregar el lote. Si no, la TUI lo muestra a 0.
    let dir = tempfile::tempdir().unwrap();
    let store = almacen(dir.path(), "recuento.db").await;
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let agent = agente_con_jobs(dir.path(), store.clone(), bus, jobs.clone());
    let job = jobs
        .start(job_vigilado(
            sid,
            "FAIL",
            20,
            Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(None)
                })
            }),
        ))
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();
    agent.flush_subagent_notices(sid).await.unwrap();

    let recuento = ultimo_recuento_publicado(&mut rx, job.id())
        .expect("el job vigilado debe publicar su recuento");
    assert_eq!(
        recuento.delivered, 1,
        "el recuento publicado debe incluir la línea entregada"
    );
}
