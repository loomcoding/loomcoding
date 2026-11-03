//! Driver del supervisor: lo alimenta con los eventos del bus del padre, le da el tick y ejecuta lo
//! que decide. Es la única parte con reloj y con E/S; las decisiones viven en `supervisor`.

use super::supervisor::{
    ChildHealth, HealthSummary, LostPolicy, SubagentSupervisor, SupervisorAction,
};
use crate::event::{EventBus, LiveEvent};
use crate::session::{SessionId, SessionStore};
use crate::subagent_phase::SubagentPhase;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

/// Cada cuánto se evalúa a los hijos. Con umbrales de decenas de segundos, cinco bastan.
pub const DEFAULT_TICK: Duration = Duration::from_secs(5);

/// Cada cuánto, como mucho, se guarda el latido de un hijo cuya fase no cambia: el almacén no
/// necesita cada token, solo saber que sigue vivo.
const HEARTBEAT_SAVE_MS: u64 = 10_000;

/// Quien ejecuta el paso a `failed` de un hijo perdido (lo implementa el agente).
#[async_trait::async_trait]
pub trait LostHandler: Send + Sync {
    async fn fail_lost(&self, parent: SessionId, child: SessionId, reason: String);
}

/// Vista compartida de la salud de los hijos, para quien necesite mostrarla (`runtime_snapshot`).
#[derive(Clone)]
pub struct SupervisorHandle {
    inner: Arc<Mutex<SubagentSupervisor>>,
}

impl SupervisorHandle {
    /// Hijos vivos con su fase y su último latido.
    pub fn children(&self) -> Vec<ChildHealth> {
        self.inner.lock().map(|s| s.children()).unwrap_or_default()
    }

    /// Salud agregada de los hijos directos de `parent`.
    pub fn summary(&self, parent: SessionId) -> HealthSummary {
        self.inner
            .lock()
            .map(|s| s.summary(parent))
            .unwrap_or_default()
    }
}

/// Arranca el supervisor sobre `bus` y devuelve su vista. La tarea acaba cuando el bus se cierra.
pub fn spawn_supervisor(
    bus: EventBus,
    stall_ms: u64,
    lost_ms: u64,
    tick: Duration,
    policy: LostPolicy,
    handler: Option<Arc<dyn LostHandler>>,
    store: Option<Arc<dyn SessionStore>>,
) -> SupervisorHandle {
    let inner = Arc::new(Mutex::new(SubagentSupervisor::new(
        stall_ms, lost_ms, policy,
    )));
    let handle = SupervisorHandle {
        inner: inner.clone(),
    };
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        let mut saved: HashMap<i64, (SubagentPhase, u64)> = HashMap::new();
        let mut interval = tokio::time::interval(tick);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let actions = tokio::select! {
                event = rx.recv() => match event {
                    Ok(event) => lock(&inner).on_event(&event, now()),
                    // Un padre muy verboso puede adelantar el búfer: se pierden eventos, no la vigilancia.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = interval.tick() => lock(&inner).tick(now()),
            };
            for action in actions {
                execute(&bus, action, stall_ms, lost_ms, handler.as_ref());
            }
            if let Some(store) = &store {
                persist(&inner, store, &mut saved);
            }
        }
    });
    handle
}

fn now() -> u64 {
    crate::message::now_ms().max(0) as u64
}

fn lock(inner: &Mutex<SubagentSupervisor>) -> std::sync::MutexGuard<'_, SubagentSupervisor> {
    inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Guarda en el almacén la fase y el latido de los hijos que cambiaron desde la última vez. La
/// escritura va en una tarea aparte: el reloj del supervisor no espera a la base de datos.
fn persist(
    inner: &Mutex<SubagentSupervisor>,
    store: &Arc<dyn SessionStore>,
    saved: &mut HashMap<i64, (SubagentPhase, u64)>,
) {
    let children = lock(inner).children();
    saved.retain(|id, _| children.iter().any(|c| c.child.0 == *id));
    for health in children {
        let due = match saved.get(&health.child.0) {
            None => true,
            Some((phase, written)) => {
                *phase != health.phase
                    || health.last_activity_ms >= written.saturating_add(HEARTBEAT_SAVE_MS)
            }
        };
        if !due {
            continue;
        }
        saved.insert(health.child.0, (health.phase, health.last_activity_ms));
        let store = store.clone();
        tokio::spawn(async move {
            if let Err(error) = store
                .update_subagent_runtime(
                    health.parent,
                    health.child,
                    health.phase,
                    health.last_activity_ms,
                )
                .await
            {
                tracing::warn!("could not save subagent runtime: {error}");
            }
        });
    }
}

fn execute(
    bus: &EventBus,
    action: SupervisorAction,
    stall_ms: u64,
    lost_ms: u64,
    handler: Option<&Arc<dyn LostHandler>>,
) {
    match action {
        SupervisorAction::Mark {
            parent,
            child,
            phase,
        } => bus.publish(LiveEvent::SubagentPhaseChanged {
            parent,
            child,
            phase,
        }),
        SupervisorAction::Notify { child, phase, .. } => {
            let message = match phase {
                SubagentPhase::Lost => format!(
                    "subagent #{} is lost: no activity for over {} s",
                    child.0,
                    lost_ms / 1000
                ),
                _ => format!(
                    "subagent #{} seems stuck: no activity for over {} s",
                    child.0,
                    stall_ms / 1000
                ),
            };
            bus.publish(LiveEvent::RuntimeNotice { message });
        }
        SupervisorAction::Fail { parent, child } => {
            if let Some(handler) = handler {
                let handler = handler.clone();
                let reason = format!("no activity for over {} s", lost_ms / 1000);
                tokio::spawn(async move { handler.fail_lost(parent, child, reason).await });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SubagentDescriptor;
    use crate::task::{TaskState, TaskStatus};

    fn working(child: i64) -> LiveEvent {
        LiveEvent::SubagentChanged(SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent: SessionId(1),
            child: SessionId(child),
            job_id: format!("subagent-{child}"),
            label: "hijo".into(),
            status: TaskStatus::new(TaskState::Working),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
    }

    #[tokio::test]
    async fn un_hijo_mudo_pasa_a_stalled_y_luego_a_lost_con_avisos() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let handle = spawn_supervisor(
            bus.clone(),
            30,
            90,
            Duration::from_millis(10),
            LostPolicy::MarkAndNotify,
            None,
            None,
        );
        bus.publish(working(7));
        let mut fases = Vec::new();
        let mut avisos = 0;
        let limite = tokio::time::Instant::now() + Duration::from_secs(5);
        while fases.len() < 2 || avisos < 2 {
            let evento = tokio::time::timeout_at(limite, rx.recv())
                .await
                .expect("el supervisor debía actuar")
                .unwrap();
            match evento {
                LiveEvent::SubagentPhaseChanged { phase, .. } => fases.push(phase),
                LiveEvent::RuntimeNotice { message } => {
                    assert!(message.contains("#7"), "{message}");
                    avisos += 1;
                }
                _ => {}
            }
        }
        assert_eq!(fases, vec![SubagentPhase::Stalled, SubagentPhase::Lost]);
        assert_eq!(handle.summary(SessionId(1)).lost, 1);
    }

    #[tokio::test]
    async fn la_vista_compartida_muestra_los_hijos_vivos() {
        let bus = EventBus::new(64);
        let handle = spawn_supervisor(
            bus.clone(),
            60_000,
            120_000,
            Duration::from_millis(10),
            LostPolicy::MarkAndNotify,
            None,
            None,
        );
        bus.publish(working(7));
        let limite = tokio::time::Instant::now() + Duration::from_secs(2);
        while handle.children().is_empty() {
            assert!(tokio::time::Instant::now() < limite, "no llegó el hijo");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(handle.children()[0].child, SessionId(7));
    }

    #[tokio::test]
    async fn la_fase_y_el_latido_se_guardan_en_el_almacen() {
        use crate::session::{sqlite::SqliteStore, SessionStore};
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("sup.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/proj").await.unwrap();
        let child = store.create_session("/proj").await.unwrap();
        let LiveEvent::SubagentChanged(mut descriptor) = working(0) else {
            unreachable!()
        };
        descriptor.parent = parent;
        descriptor.child = child;
        store.save_subagent(&descriptor).await.unwrap();
        let bus = EventBus::new(64);
        let _handle = spawn_supervisor(
            bus.clone(),
            60_000,
            120_000,
            Duration::from_millis(10),
            LostPolicy::MarkAndNotify,
            None,
            Some(store.clone()),
        );
        bus.publish(LiveEvent::SubagentChanged(descriptor));
        bus.publish(LiveEvent::SubagentPhaseChanged {
            parent,
            child,
            phase: SubagentPhase::Streaming,
        });
        let limite = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let hijos = store.list_subagents(parent).await.unwrap();
            if hijos[0].phase == SubagentPhase::Streaming && hijos[0].last_activity_at_ms > 0 {
                break;
            }
            assert!(tokio::time::Instant::now() < limite, "no se guardó la fase");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    struct Espia(tokio::sync::mpsc::UnboundedSender<(SessionId, SessionId, String)>);

    #[async_trait::async_trait]
    impl LostHandler for Espia {
        async fn fail_lost(&self, parent: SessionId, child: SessionId, reason: String) {
            let _ = self.0.send((parent, child, reason));
        }
    }

    #[tokio::test]
    async fn con_la_politica_de_fallar_el_manejador_recibe_al_hijo_perdido() {
        let bus = EventBus::new(64);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _handle = spawn_supervisor(
            bus.clone(),
            30,
            90,
            Duration::from_millis(10),
            LostPolicy::FailAndStop,
            Some(Arc::new(Espia(tx))),
            None,
        );
        bus.publish(working(7));
        let (parent, child, motivo) = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("el manejador debía recibir al hijo perdido")
            .unwrap();
        assert_eq!((parent, child), (SessionId(1), SessionId(7)));
        assert!(motivo.contains("no activity"), "{motivo}");
    }

    #[tokio::test]
    async fn con_la_politica_de_solo_avisar_no_se_llama_al_manejador() {
        let bus = EventBus::new(64);
        let mut avisos = bus.subscribe();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _handle = spawn_supervisor(
            bus.clone(),
            30,
            90,
            Duration::from_millis(10),
            LostPolicy::MarkAndNotify,
            Some(Arc::new(Espia(tx))),
            None,
        );
        bus.publish(working(7));
        // Espera a que se dé por perdido (segundo aviso) y comprueba que nadie fue llamado.
        let mut n = 0;
        while n < 2 {
            if let LiveEvent::RuntimeNotice { .. } =
                tokio::time::timeout(Duration::from_secs(5), avisos.recv())
                    .await
                    .unwrap()
                    .unwrap()
            {
                n += 1;
            }
        }
        assert!(rx.try_recv().is_err());
    }
}
