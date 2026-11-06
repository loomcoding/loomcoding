//! El bucle `run_until_settled`: sin TUI, el padre espera a sus hijos y se despierta solo.

use super::*;
use crate::agent::settle::MAX_WAKE_TURNS;
use crate::session::SubagentDescriptor;
use crate::task::{TaskState, TaskStatus};

fn child(parent: SessionId, state: TaskState) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent,
        child: SessionId(99),
        job_id: "subagent-99".into(),
        label: "hijo".into(),
        status: TaskStatus::new(state),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

/// Lanzador de prueba: el hijo sigue vivo hasta que se entrega su aviso; con `always`, siempre hay aviso.
struct SettleLauncher {
    parent: SessionId,
    alive: std::sync::atomic::AtomicBool,
    /// El aviso solo existe una vez el hijo ha terminado: la prueba lo activa antes de publicar el evento.
    ready: Arc<std::sync::atomic::AtomicBool>,
    always: bool,
}

#[async_trait]
impl crate::subagent::SubagentLauncher for SettleLauncher {
    async fn start(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: Option<String>,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, crate::job::JobId)> {
        Err(crate::error::CoreError::Tool("not used".into()))
    }

    async fn control(
        &self,
        _parent: SessionId,
        _child: SessionId,
        _action: &str,
        _message: Option<String>,
    ) -> Result<()> {
        Err(crate::error::CoreError::Tool("not used".into()))
    }

    async fn list(&self, _parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
        let state = if self.alive.load(Ordering::SeqCst) {
            TaskState::Working
        } else {
            TaskState::Completed
        };
        Ok(vec![child(self.parent, state)])
    }

    async fn deliver_notices(&self, _parent: SessionId) -> Result<Vec<Message>> {
        if self.always {
            return Ok(vec![Message::user(
                "[Subagent hijo completed]
listo",
            )]);
        }
        if self.ready.swap(false, Ordering::SeqCst) {
            self.alive.store(false, Ordering::SeqCst);
            return Ok(vec![Message::user("[Subagent hijo completed]\nlisto")]);
        }
        Ok(Vec::new())
    }
}

struct Rig {
    agent: AgentLoop,
    sid: SessionId,
    bus: EventBus,
    provider: Arc<QueueProvider>,
    store: Arc<SqliteStore>,
    ready: Arc<std::sync::atomic::AtomicBool>,
}

fn text(t: &str) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(t.into()),
        StreamChunk::Done(FinishReason::Stop),
    ]
}

async fn rig(scripts: Vec<Vec<StreamChunk>>, launcher: Option<(bool, bool)>) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let bus = EventBus::new(256);
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut agent = AgentLoop::new(
        provider.clone(),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        bus.clone(),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.keep(),
    );
    if let Some((alive, always)) = launcher {
        agent.subagents = Some(Arc::new(SettleLauncher {
            parent: sid,
            alive: std::sync::atomic::AtomicBool::new(alive),
            ready: ready.clone(),
            always,
        }));
    }
    Rig {
        agent,
        sid,
        bus,
        provider,
        store,
        ready,
    }
}

fn settled(parent: SessionId, status: TaskState) -> LiveEvent {
    LiveEvent::SubagentSettled {
        parent,
        child: SessionId(99),
        status,
        summary: None,
    }
}

/// Espera a que el proveedor haya servido el primer guion: el primer turno ya pasó por su vaciado de avisos.
async fn first_round_started(provider: &Arc<QueueProvider>, total: usize) {
    while provider.scripts.lock().unwrap().len() == total {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

fn left(rig: &Rig) -> usize {
    rig.provider.scripts.lock().unwrap().len()
}

#[tokio::test]
async fn without_children_it_behaves_like_run() {
    let rig = rig(vec![text("hola")], None).await;
    let outcome = rig
        .agent
        .run_until_settled(rig.sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(outcome.final_text, "hola");
    assert_eq!(left(&rig), 0);
    let assistants = rig
        .store
        .messages(rig.sid)
        .await
        .unwrap()
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .count();
    assert_eq!(assistants, 1);
}

#[tokio::test]
async fn a_completed_child_wakes_the_parent_once() {
    let rig = rig(
        vec![text("lanzado"), text("resumen"), text("sobra")],
        Some((true, false)),
    )
    .await;
    let (bus, sid, ready) = (rig.bus.clone(), rig.sid, rig.ready.clone());
    let provider = rig.provider.clone();
    let total = left(&rig);
    tokio::spawn(async move {
        first_round_started(&provider, total).await;
        ready.store(true, Ordering::SeqCst);
        bus.publish(settled(sid, TaskState::Completed));
    });
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        rig.agent
            .run_until_settled(rig.sid, "hi".into(), CancellationToken::new()),
    )
    .await
    .expect("no debía quedarse esperando")
    .unwrap();
    assert_eq!(outcome.final_text, "resumen");
    assert_eq!(
        left(&rig),
        1,
        "el proveedor se consulta exactamente dos veces"
    );
}

#[tokio::test]
async fn a_canceled_child_does_not_wake_the_parent() {
    let rig = rig(vec![text("lanzado"), text("sobra")], Some((false, false))).await;
    let (bus, sid, ready) = (rig.bus.clone(), rig.sid, rig.ready.clone());
    let provider = rig.provider.clone();
    let total = left(&rig);
    tokio::spawn(async move {
        first_round_started(&provider, total).await;
        // Aviso preparado: si el cancelado despertara al padre, se entregaría y se vería.
        ready.store(true, Ordering::SeqCst);
        bus.publish(settled(sid, TaskState::Canceled));
    });
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        rig.agent
            .run_until_settled(rig.sid, "hi".into(), CancellationToken::new()),
    )
    .await
    .expect("sin hijos vivos no hay nada que esperar")
    .unwrap();
    assert_eq!(outcome.final_text, "lanzado");
    assert_eq!(left(&rig), 1, "el proveedor se consulta una sola vez");
}

#[tokio::test]
async fn wake_turns_are_capped() {
    let scripts: Vec<_> = (0..20).map(|n| text(&format!("t{n}"))).collect();
    let rig = rig(scripts, Some((true, true))).await;
    let (bus, sid) = (rig.bus.clone(), rig.sid);
    // Cabe señalar que el aviso no puede ser finito: bajo carga el consumidor agrupa los que
    // encuentra en cola y, si se acabaran antes de los despertares pedidos, esperaría para siempre.
    let publisher = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(15)).await;
            bus.publish(settled(sid, TaskState::Completed));
        }
    });
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        rig.agent
            .run_until_settled(rig.sid, "hi".into(), CancellationToken::new()),
    )
    .await;
    publisher.abort();
    result
        .expect("el tope de despertares debía cortar el bucle")
        .unwrap();
    let consulted = 20 - left(&rig);
    assert_eq!(consulted, 1 + MAX_WAKE_TURNS as usize);
}

#[tokio::test]
async fn a_job_watch_batch_wakes_the_parent_like_a_child() {
    let rig = rig(
        vec![text("lanzado"), text("resumen"), text("sobra")],
        Some((true, false)),
    )
    .await;
    let (bus, sid, ready) = (rig.bus.clone(), rig.sid, rig.ready.clone());
    let provider = rig.provider.clone();
    let total = left(&rig);
    tokio::spawn(async move {
        first_round_started(&provider, total).await;
        // El lote tiene avisos que entregar: el despertar produce un turno real.
        ready.store(true, Ordering::SeqCst);
        bus.publish(LiveEvent::JobWatchReady { session: sid });
    });
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        rig.agent
            .run_until_settled(rig.sid, "hi".into(), CancellationToken::new()),
    )
    .await
    .expect("un lote de vigilancia listo debía despertar al padre")
    .unwrap();
    assert_eq!(outcome.final_text, "resumen");
    assert_eq!(
        left(&rig),
        1,
        "el proveedor se consulta exactamente dos veces"
    );
}

#[tokio::test]
async fn job_watch_wake_turns_share_the_cap() {
    let scripts: Vec<_> = (0..20).map(|n| text(&format!("t{n}"))).collect();
    let rig = rig(scripts, Some((true, true))).await;
    let (bus, sid) = (rig.bus.clone(), rig.sid);
    // Igual que con los hijos: el aviso puede repetirse sin fin y el tope debe cortar el bucle.
    let publisher = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(15)).await;
            bus.publish(LiveEvent::JobWatchReady { session: sid });
        }
    });
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        rig.agent
            .run_until_settled(rig.sid, "hi".into(), CancellationToken::new()),
    )
    .await;
    publisher.abort();
    result
        .expect("el tope de despertares debía cortar el bucle")
        .unwrap();
    let consulted = 20 - left(&rig);
    assert_eq!(consulted, 1 + MAX_WAKE_TURNS as usize);
}

#[tokio::test]
async fn cancellation_interrupts_the_wait() {
    let rig = rig(vec![text("lanzado"), text("sobra")], Some((true, false))).await;
    let cancel = CancellationToken::new();
    let stopper = cancel.clone();
    let provider = rig.provider.clone();
    let total = left(&rig);
    tokio::spawn(async move {
        first_round_started(&provider, total).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        stopper.cancel();
    });
    tokio::time::timeout(
        Duration::from_secs(2),
        rig.agent.run_until_settled(rig.sid, "hi".into(), cancel),
    )
    .await
    .expect("la cancelación debía interrumpir la espera")
    .unwrap();
    assert_eq!(left(&rig), 1);
}
