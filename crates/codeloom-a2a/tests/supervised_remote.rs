//! Un agente remoto vigilado por el supervisor, de extremo a extremo: lanzador → espejo → remoto
//! simulado con `wiremock` → supervisor sobre el bus del padre.
//!
//! Cabe señalar que estas pruebas cierran el hueco que dejaba el plan de estados: hasta ahora el
//! supervisor y el espejo se probaban por separado. Aquí se comprueba lo que importa a la persona:
//! un remoto vivo no se marca como perdido, uno que se calla sí, y con `FailAndStop` se da por
//! fallido y se le manda `CancelTask`.

mod support;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codeloom_core::event::LiveEvent;
use codeloom_core::session::{SessionId, SessionStore};
use codeloom_core::subagent::phase::SubagentPhase;
use codeloom_core::subagent::supervisor::LostPolicy;
use codeloom_core::subagent::supervisor_driver::{spawn_supervisor, LostHandler, SupervisorHandle};
use codeloom_core::subagent::SubagentLauncher;
use codeloom_core::task::TaskState;
use support::*;
use tokio_util::sync::CancellationToken;

/// Sin latido durante este tiempo, el hijo figura como atascado.
const STALL_MS: u64 = 400;
/// Sin latido durante este tiempo, el hijo figura como perdido.
const LOST_MS: u64 = 900;

/// El `LostHandler` real vive en el agente; aquí delega directamente en el lanzador.
struct DelegaEnElLanzador(Arc<dyn SubagentLauncher>);

#[async_trait]
impl LostHandler for DelegaEnElLanzador {
    async fn fail_lost(&self, parent: SessionId, child: SessionId, reason: String) {
        self.0.fail_lost(parent, child, reason).await.unwrap();
    }
}

fn vigilar(
    montaje: &Montaje,
    policy: LostPolicy,
    lanzador: Option<Arc<dyn SubagentLauncher>>,
) -> SupervisorHandle {
    let store: Arc<dyn SessionStore> = montaje.store.clone();
    let handler = lanzador.map(|l| Arc::new(DelegaEnElLanzador(l)) as Arc<dyn LostHandler>);
    spawn_supervisor(
        montaje.bus.clone(),
        STALL_MS,
        LOST_MS,
        Duration::from_millis(50),
        policy,
        handler,
        Some(store),
    )
}

async fn lanzar(montaje: &Montaje) -> SessionId {
    montaje.registro.approve("lejano").unwrap();
    let (hijo, _) = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    espera(montaje, hijo, TaskState::Working).await;
    hijo
}

/// Espera a que el supervisor vea al hijo en `fase`.
async fn espera_fase(supervisor: &SupervisorHandle, hijo: SessionId, fase: SubagentPhase) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if supervisor
                .children()
                .iter()
                .any(|c| c.child == hijo && c.phase == fase)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "el supervisor debería ver al hijo en {fase:?}; ve {:?}",
            supervisor.children()
        )
    });
}

async fn estado_de(montaje: &Montaje, hijo: SessionId) -> TaskState {
    montaje
        .store
        .list_subagents(montaje.parent)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.child == hijo)
        .map(|d| d.status.state)
        .unwrap()
}

#[tokio::test]
async fn a_remote_that_keeps_answering_is_never_marked_stalled_or_lost() {
    let montaje = montar(0).await;
    let supervisor = vigilar(&montaje, LostPolicy::FailAndStop, None);
    let hijo = lanzar(&montaje).await;

    // Más del doble del umbral de pérdida: si el sondeo no contara como latido, ya estaría perdido.
    tokio::time::sleep(Duration::from_millis(LOST_MS * 2)).await;

    let resumen = supervisor.summary(montaje.parent);
    assert_eq!((resumen.stalled, resumen.lost), (0, 0), "{resumen:?}");
    assert_eq!(resumen.active, 1);
    assert_eq!(estado_de(&montaje, hijo).await, TaskState::Working);
}

#[tokio::test]
async fn a_remote_that_goes_silent_is_marked_stalled_then_lost_and_the_person_is_told() {
    let montaje = montar_colgado(3).await;
    let mut eventos = montaje.bus.subscribe();
    let supervisor = vigilar(&montaje, LostPolicy::MarkAndNotify, None);
    let hijo = lanzar(&montaje).await;

    espera_fase(&supervisor, hijo, SubagentPhase::Stalled).await;
    espera_fase(&supervisor, hijo, SubagentPhase::Lost).await;

    // Solo se marca y se avisa: la tarea no se toca y al remoto no se le cancela nada.
    assert_eq!(estado_de(&montaje, hijo).await, TaskState::Working);
    assert!(!metodos_recibidos(&montaje.servidor)
        .await
        .contains(&"CancelTask".to_string()));
    let mut avisos = Vec::new();
    while let Ok(evento) = eventos.try_recv() {
        if let LiveEvent::RuntimeNotice { message } = evento {
            avisos.push(message);
        }
    }
    assert!(
        avisos.iter().any(|m| m.contains("seems stuck")),
        "{avisos:?}"
    );
    assert!(avisos.iter().any(|m| m.contains("is lost")), "{avisos:?}");
}

#[tokio::test]
async fn with_fail_and_stop_a_lost_remote_is_failed_and_sent_cancel_task() {
    let montaje = montar_colgado(3).await;
    let lanzador: Arc<dyn SubagentLauncher> = montaje.lanzador.clone();
    let supervisor = vigilar(&montaje, LostPolicy::FailAndStop, Some(lanzador));
    let hijo = lanzar(&montaje).await;

    espera(&montaje, hijo, TaskState::Failed).await;

    let descriptor = montaje
        .store
        .list_subagents(montaje.parent)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.child == hijo)
        .unwrap();
    let motivo = descriptor
        .status
        .message
        .as_ref()
        .map(|m| format!("{m:?}"))
        .unwrap_or_default();
    assert!(
        motivo.contains("lost"),
        "el motivo debe decir que se perdió: {motivo}"
    );
    // Dejó de dar señales: figura `failed`, no `canceled`, y ya no está vivo para el supervisor.
    assert_ne!(descriptor.status.state, TaskState::Canceled);
    // Matar el trabajo hace que el espejo cancele la tarea en el remoto.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !metodos_recibidos(&montaje.servidor)
            .await
            .contains(&"CancelTask".to_string())
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("al darlo por perdido se debe mandar CancelTask al remoto");
    drop(supervisor);
}
