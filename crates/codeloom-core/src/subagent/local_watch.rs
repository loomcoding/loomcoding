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
        loop {
            match child_rx.recv().await {
                Ok(event) => {
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
                settle_bus.publish(LiveEvent::SubagentSettled {
                    parent,
                    child,
                    status: estado_final,
                });
                break;
            }
            if updates.changed().await.is_err() {
                break;
            }
        }
    });
}
