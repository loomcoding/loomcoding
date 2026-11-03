//! Las dos tareas que acompañan a un hijo local desde fuera de su trabajo: el reenvío filtrado de
//! sus eventos al padre y la vigilancia de su asiento final.

use super::*;
use crate::job::JobSnapshot;
use tokio::sync::{broadcast, watch};

/// Reenvío filtrado: solo los eventos relevantes del hijo llegan al padre, etiquetados con su
/// identidad. La tarea termina cuando el bus del hijo se cierra.
pub(super) fn reenviar_eventos_del_hijo(
    forward_bus: EventBus,
    mut child_rx: broadcast::Receiver<LiveEvent>,
    parent: SessionId,
    child: SessionId,
) {
    tokio::spawn(async move {
        let mut fases = phase_observer::PhaseObserver::new();
        loop {
            match child_rx.recv().await {
                Ok(event) => {
                    // La fase se deduce de todo lo que ocurre en el hijo, también de lo que no se
                    // reenvía (el razonamiento); el observador ya coalesce y acota los cambios.
                    for phase in fases.observe(&event, crate::message::now_ms().max(0) as u64) {
                        forward_bus.publish(LiveEvent::SubagentPhaseChanged {
                            parent,
                            child,
                            phase,
                        });
                    }
                    // El avance que publica el propio hijo cambia su descriptor: sube tal cual, sin
                    // envolver, como el resto de cambios de tarea que publica el lanzador.
                    if matches!(&event, LiveEvent::SubagentChanged(d) if d.parent == parent && d.child == child)
                    {
                        forward_bus.publish(event);
                    } else if event.is_child_relevant() {
                        forward_bus.publish(LiveEvent::ChildEvent {
                            parent,
                            child,
                            event: Box::new(event),
                        });
                    }
                }
                // Un hijo muy verboso puede adelantar el búfer: se pierden eventos, no el reenvío.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Tope del resumen que acompaña al asiento: es un extracto, no el resultado entero.
const MAX_SUMMARY_BYTES: usize = 500;

/// Extracto del hijo asentado: el motivo si falló, o el texto de su primer artefacto.
fn resumen_del_hijo(hijo: &crate::session::SubagentDescriptor) -> Option<String> {
    let texto = match &hijo.status.message {
        Some(mensaje) => mensaje.text(),
        None => hijo.artifacts.first()?.text(),
    };
    let texto = texto.trim();
    (!texto.is_empty()).then(|| crate::tools::limit_bytes(texto, MAX_SUMMARY_BYTES).into_owned())
}

/// Vigila el trabajo del hijo y, cuando acaba, asienta su estado final y publica `SubagentSettled`.
pub(super) fn vigilar_asiento(
    store: Arc<dyn SessionStore>,
    settle_bus: EventBus,
    tracker: Arc<TaskTracker>,
    mut updates: watch::Receiver<JobSnapshot>,
    parent: SessionId,
    child: SessionId,
) {
    tokio::spawn(async move {
        loop {
            let snapshot = updates.borrow_and_update().clone();
            let state = TaskState::from(snapshot.status);
            if state.is_terminal() {
                // `complete_subagent` ya fijó `Completed` junto con el resultado: aquí solo se
                // escribe lo que el almacén aún no tiene asentado (muerte, fallo o cierre sin resultado).
                let asentado = match store.list_subagents(parent).await {
                    Ok(hijos) => hijos
                        .iter()
                        .find(|hijo| hijo.child == child && hijo.status.state.is_terminal())
                        .map(|hijo| hijo.status.clone()),
                    Err(error) => {
                        tracing::warn!("read subagent status: {error}");
                        None
                    }
                };
                // Manda el estado ya persistido: un espejo remoto fija el suyo (p. ej. `rejected`)
                // antes de que el trabajo acabe, y el aviso no debe contradecirlo.
                let estado_final = match asentado {
                    Some(persistido) => persistido.state,
                    None => {
                        let mut status = TaskStatus::new(state);
                        if state == TaskState::Failed {
                            if let Some(detail) = snapshot.detail.as_deref() {
                                status.message = Some(Message::agent_text(
                                    crate::tools::limit_bytes(
                                        detail,
                                        crate::tools::MAX_NOTICE_BYTES,
                                    )
                                    .into_owned(),
                                ));
                            }
                        }
                        if let Err(error) = tracker.transition(parent, child, status).await {
                            tracing::warn!("persist subagent status: {error}");
                        }
                        state
                    }
                };
                // Se publica tras el intento de persistir, también si falló: así el
                // padre nunca lee el aviso antes del estado final.
                let summary = match store.list_subagents(parent).await {
                    Ok(hijos) => hijos
                        .iter()
                        .find(|hijo| hijo.child == child)
                        .and_then(resumen_del_hijo),
                    Err(_) => None,
                };
                settle_bus.publish(LiveEvent::SubagentSettled {
                    parent,
                    child,
                    status: estado_final,
                    summary,
                });
                break;
            }
            if updates.changed().await.is_err() {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn siguiente(rx: &mut broadcast::Receiver<LiveEvent>) -> LiveEvent {
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("el padre debía recibir un evento")
            .unwrap()
    }

    #[tokio::test]
    async fn estado_de_hijo_llega_al_padre() {
        let child_bus = EventBus::new(64);
        let parent_bus = EventBus::new(64);
        let mut parent_rx = parent_bus.subscribe();
        reenviar_eventos_del_hijo(
            parent_bus.clone(),
            child_bus.subscribe(),
            SessionId(1),
            SessionId(7),
        );
        child_bus.publish(LiveEvent::TurnStarted);
        child_bus.publish(LiveEvent::Delta { text: "x".into() });
        let mut fases = Vec::new();
        let mut envueltos = 0;
        while fases.len() < 2 || envueltos < 2 {
            match siguiente(&mut parent_rx).await {
                LiveEvent::SubagentPhaseChanged {
                    parent,
                    child,
                    phase,
                } => {
                    assert_eq!((parent, child), (SessionId(1), SessionId(7)));
                    fases.push(phase);
                }
                LiveEvent::ChildEvent { .. } => envueltos += 1,
                other => panic!("evento inesperado: {other:?}"),
            }
        }
        assert_eq!(
            fases,
            vec![SubagentPhase::Starting, SubagentPhase::Streaming]
        );
    }

    #[tokio::test]
    async fn el_razonamiento_alimenta_la_fase_aunque_no_se_reenvie() {
        let child_bus = EventBus::new(64);
        let parent_bus = EventBus::new(64);
        let mut parent_rx = parent_bus.subscribe();
        reenviar_eventos_del_hijo(
            parent_bus.clone(),
            child_bus.subscribe(),
            SessionId(1),
            SessionId(7),
        );
        child_bus.publish(LiveEvent::TurnStarted);
        child_bus.publish(LiveEvent::ReasoningDelta { text: "x".into() });
        let mut fases = Vec::new();
        while fases.len() < 3 {
            match siguiente(&mut parent_rx).await {
                LiveEvent::SubagentPhaseChanged { phase, .. } => fases.push(phase),
                LiveEvent::ChildEvent { event, .. } => {
                    assert!(
                        !matches!(*event, LiveEvent::ReasoningDelta { .. }),
                        "el razonamiento no se reenvía"
                    );
                }
                other => panic!("evento inesperado: {other:?}"),
            }
        }
        assert_eq!(
            fases,
            vec![
                SubagentPhase::Starting,
                SubagentPhase::Streaming,
                SubagentPhase::Reasoning
            ]
        );
    }
}
