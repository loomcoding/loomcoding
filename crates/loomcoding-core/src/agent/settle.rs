//! Bucle acotado del padre sin interfaz: tras el turno del usuario espera a sus hijos y se despierta.
//!
//! Cabe señalar que en la TUI quien despierta al padre es la propia TUI (`App::take_wake`); sin ella
//! (`loomcoding --print`) nadie lo hacía y los hijos vivos quedaban abandonados. Las causas son las
//! mismas que las de la TUI para no inventar semántica: hijo `Completed` o `Failed`, hijo que pide
//! entrada y lote de vigilancia listo (`JobWatchReady`). `Canceled` y `Rejected` no despiertan.

use super::*;
use crate::task::TaskState;
use std::time::Duration;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::time::{sleep_until, Instant};

/// Despertares del padre que se toleran tras el turno del usuario.
pub const MAX_WAKE_TURNS: u32 = 8;

/// Espera total, compartida entre vueltas, a que un hijo vivo dé señales.
pub const MAX_SETTLE_WAIT: Duration = Duration::from_secs(30 * 60);

/// ¿Este evento obliga a despertar al padre `session`?
fn is_wake_cause(event: &LiveEvent, session: SessionId) -> bool {
    match event {
        LiveEvent::SubagentSettled { parent, status, .. } => {
            *parent == session && matches!(status, TaskState::Completed | TaskState::Failed)
        }
        LiveEvent::SubagentInputRequested { parent, .. } => *parent == session,
        // Un lote de vigilancia listo cuenta como un hijo: mismo presupuesto y mismo tope.
        LiveEvent::JobWatchReady { session: s } => *s == session,
        _ => false,
    }
}

impl AgentLoop {
    /// Turno del usuario seguido de los despertares que pidan sus hijos, con cotas de número,
    /// de plazo y de cancelación. Sin lanzador de subagentes se comporta como `run`.
    pub async fn run_until_settled(
        &self,
        session: SessionId,
        prompt: String,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome> {
        // Se suscribe antes del primer turno: los eventos emitidos durante él no se pierden.
        let mut rx = self.bus.subscribe();
        let mut last = self.run(session, prompt, cancel.clone()).await?;
        let Some(subagents) = self.subagents_visibles().cloned() else {
            return Ok(last);
        };
        let mut wakes = 0u32;
        let mut deadline: Option<Instant> = None;
        while wakes < MAX_WAKE_TURNS && !cancel.is_cancelled() {
            let mut cause = false;
            loop {
                match rx.try_recv() {
                    Ok(event) => cause |= is_wake_cause(&event, session),
                    Err(TryRecvError::Lagged(_)) => cause = true,
                    Err(_) => break,
                }
            }
            if !cause {
                let alive = subagents
                    .list(session)
                    .await?
                    .iter()
                    .any(|d| !d.status.state.is_terminal());
                if !alive {
                    break;
                }
                let limit = *deadline.get_or_insert_with(|| Instant::now() + MAX_SETTLE_WAIT);
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => return Ok(last),
                        _ = sleep_until(limit) => {
                            self.bus.publish(LiveEvent::RuntimeNotice {
                                message: "se agotó la espera a los subagentes vivos".into(),
                            });
                            return Ok(last);
                        }
                        event = rx.recv() => match event {
                            Ok(event) if !is_wake_cause(&event, session) => {}
                            Ok(_) | Err(RecvError::Lagged(_)) => break,
                            Err(RecvError::Closed) => return Ok(last),
                        },
                    }
                }
            }
            wakes += 1;
            if let Some(outcome) = self.run_woken(session, cancel.clone()).await? {
                last = outcome;
            }
        }
        Ok(last)
    }
}
