//! Parada de emergencia («modo Dios»): cancela los hijos y los jobs de una sesión de una vez.

use super::*;
use crate::job::JobStatus;

/// Lo que la parada de emergencia ha cancelado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KillReport {
    pub children: usize,
    pub jobs: usize,
}

impl AgentLoop {
    /// Cancela los hijos no terminales y los jobs en marcha de `session`, y lo publica como aviso.
    ///
    /// Cabe señalar que no toca el turno activo: eso lo hace quien lo lanzó, cancelando su token.
    /// Un fallo al cancelar uno de ellos no impide cancelar los demás; solo cuenta lo cancelado.
    pub async fn kill_everything(&self, session: SessionId) -> KillReport {
        let mut report = KillReport::default();
        if let Some(launcher) = self.subagents.as_ref() {
            if let Ok(children) = launcher.list(session).await {
                for child in children
                    .into_iter()
                    .filter(|child| !child.status.state.is_terminal())
                {
                    if launcher.cancel(session, child.child).await.is_ok() {
                        report.children += 1;
                    }
                }
            }
        }
        for job in self.jobs.list(session) {
            let live = matches!(job.status, JobStatus::Running | JobStatus::Stopping);
            if live && self.jobs.kill(session, &job.id).is_ok() {
                report.jobs += 1;
            }
        }
        self.bus.publish(LiveEvent::RuntimeNotice {
            message: format!(
                "emergency stop: {} children, {} jobs",
                report.children, report.jobs
            ),
        });
        report
    }
}
