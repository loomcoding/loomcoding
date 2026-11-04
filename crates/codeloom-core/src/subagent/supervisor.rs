//! Supervisor puro de subagentes: lleva la cuenta de la fase y del último latido de cada hijo a
//! partir de los eventos del bus del padre y, en cada tick, decide qué marcar, avisar o cancelar.
//!
//! Cabe señalar que aquí no hay reloj, hilos ni E/S: el instante entra por parámetro y las
//! decisiones salen como datos. Quien lo conduce (`supervisor_driver`) ejecuta las acciones.

use super::watchdog::{evaluate, WatchdogDecision};
use crate::event::LiveEvent;
use crate::session::SessionId;
use crate::subagent_phase::SubagentPhase;
use crate::task::TaskState;
use std::collections::BTreeMap;

/// Qué se hace con un hijo que se da por perdido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LostPolicy {
    /// Marcarlo y avisar a la persona usuaria; no se toca su tarea.
    #[default]
    MarkAndNotify,
    /// Además, darlo por fallido y detener su trabajo.
    FailAndStop,
}

/// Decisión del supervisor, para que la ejecute el driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorAction {
    /// Publicar el cambio de fase del hijo.
    Mark {
        parent: SessionId,
        child: SessionId,
        phase: SubagentPhase,
    },
    /// Avisar a la persona usuaria de que el hijo está atascado o perdido.
    Notify {
        parent: SessionId,
        child: SessionId,
        phase: SubagentPhase,
    },
    /// Dar al hijo por fallido y detener su trabajo (solo con `LostPolicy::FailAndStop`).
    Fail { parent: SessionId, child: SessionId },
}

/// Lo que el supervisor sabe de un hijo vivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildHealth {
    pub parent: SessionId,
    pub child: SessionId,
    pub phase: SubagentPhase,
    pub last_activity_ms: u64,
}

/// Recuento de la salud de los hijos de un padre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HealthSummary {
    pub active: usize,
    pub waiting: usize,
    pub stalled: usize,
    pub lost: usize,
}

#[derive(Debug)]
pub struct SubagentSupervisor {
    stall_ms: u64,
    lost_ms: u64,
    policy: LostPolicy,
    children: BTreeMap<i64, ChildHealth>,
}

impl SubagentSupervisor {
    pub fn new(stall_ms: u64, lost_ms: u64, policy: LostPolicy) -> Self {
        Self {
            stall_ms,
            lost_ms,
            policy,
            children: BTreeMap::new(),
        }
    }

    /// Hijos vivos, por identificador.
    pub fn children(&self) -> Vec<ChildHealth> {
        self.children.values().copied().collect()
    }

    /// Salud agregada de los hijos directos de `parent`.
    pub fn summary(&self, parent: SessionId) -> HealthSummary {
        let mut summary = HealthSummary::default();
        for health in self.children.values().filter(|h| h.parent == parent) {
            match health.phase {
                SubagentPhase::Lost => summary.lost += 1,
                SubagentPhase::Stalled => summary.stalled += 1,
                SubagentPhase::AwaitingPermission
                | SubagentPhase::AwaitingParentInput
                | SubagentPhase::AwaitingCredentials => summary.waiting += 1,
                _ => summary.active += 1,
            }
        }
        summary
    }

    /// Absorbe un evento del bus del padre en el instante `now_ms`.
    pub fn on_event(&mut self, event: &LiveEvent, now_ms: u64) -> Vec<SupervisorAction> {
        match event {
            LiveEvent::SubagentChanged(descriptor) => {
                let state = descriptor.status.state;
                // Un remoto no emite latidos (su espejo lo sondea y tiene sus propios plazos): vigilarlo
                // lo daría por perdido sin motivo.
                let remoto = descriptor
                    .agent_origin
                    .as_deref()
                    .is_some_and(|origen| origen.starts_with("remote:"));
                if remoto || state.is_terminal() {
                    self.children.remove(&descriptor.child.0);
                    return Vec::new();
                }
                let health = self
                    .children
                    .entry(descriptor.child.0)
                    .or_insert(ChildHealth {
                        parent: descriptor.parent,
                        child: descriptor.child,
                        phase: SubagentPhase::Queued,
                        last_activity_ms: now_ms,
                    });
                // `Lost` y `Stalled` los decide el supervisor; el estado A2A solo fija las esperas
                // y el arranque. La fase del descriptor no se usa: nadie la mantiene al día.
                if health.phase != SubagentPhase::Lost {
                    match state {
                        TaskState::InputRequired => {
                            health.phase = SubagentPhase::AwaitingParentInput
                        }
                        // Los remotos, únicos con credenciales de verdad, no se vigilan: un hijo
                        // local pide `auth-required` solo para que el padre apruebe un permiso.
                        TaskState::AuthRequired => health.phase = SubagentPhase::AwaitingPermission,
                        TaskState::Working
                            if matches!(
                                health.phase,
                                SubagentPhase::Queued
                                    | SubagentPhase::AwaitingParentInput
                                    | SubagentPhase::AwaitingCredentials
                                    | SubagentPhase::AwaitingPermission
                            ) =>
                        {
                            health.phase = SubagentPhase::Working
                        }
                        _ => {}
                    }
                    health.last_activity_ms = now_ms;
                }
                Vec::new()
            }
            LiveEvent::SubagentPhaseChanged { child, phase, .. } => {
                if let Some(health) = self.children.get_mut(&child.0) {
                    if health.phase != SubagentPhase::Lost {
                        health.phase = *phase;
                        // El eco de lo que publica el propio supervisor no es un latido del hijo.
                        if !matches!(phase, SubagentPhase::Stalled | SubagentPhase::Lost) {
                            health.last_activity_ms = now_ms;
                        }
                    }
                }
                Vec::new()
            }
            LiveEvent::ChildEvent { child, .. } => {
                let Some(health) = self.children.get_mut(&child.0) else {
                    return Vec::new();
                };
                if health.phase == SubagentPhase::Lost {
                    return Vec::new();
                }
                health.last_activity_ms = now_ms;
                if health.phase == SubagentPhase::Stalled {
                    health.phase = SubagentPhase::Working;
                    return vec![SupervisorAction::Mark {
                        parent: health.parent,
                        child: health.child,
                        phase: SubagentPhase::Working,
                    }];
                }
                Vec::new()
            }
            LiveEvent::SubagentInputRequested { child, .. } => {
                if let Some(health) = self.children.get_mut(&child.0) {
                    if health.phase != SubagentPhase::Lost {
                        health.phase = SubagentPhase::AwaitingParentInput;
                        health.last_activity_ms = now_ms;
                    }
                }
                Vec::new()
            }
            LiveEvent::SubagentInputResolved { child, .. } => {
                if let Some(health) = self.children.get_mut(&child.0) {
                    if health.phase == SubagentPhase::AwaitingParentInput {
                        health.phase = SubagentPhase::Working;
                    }
                    if health.phase != SubagentPhase::Lost {
                        health.last_activity_ms = now_ms;
                    }
                }
                Vec::new()
            }
            LiveEvent::SubagentSettled { child, .. } => {
                self.children.remove(&child.0);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Evalúa a todos los hijos en el instante `now_ms`.
    ///
    /// `ToolRunning` queda fuera: una herramienta larga (un `cargo build` de minutos) no emite
    /// latidos y marcarla sería un falso positivo; el spec (§8.2) admite esta salida mientras no
    /// haya latido periódico del ejecutor.
    pub fn tick(&mut self, now_ms: u64) -> Vec<SupervisorAction> {
        let mut actions = Vec::new();
        for health in self.children.values_mut() {
            if health.phase == SubagentPhase::ToolRunning {
                continue;
            }
            let Some(decision) = evaluate(
                health.phase,
                health.last_activity_ms,
                now_ms,
                self.stall_ms,
                self.lost_ms,
            ) else {
                continue;
            };
            let phase = match decision {
                WatchdogDecision::Stall => SubagentPhase::Stalled,
                WatchdogDecision::Lost => SubagentPhase::Lost,
            };
            health.phase = phase;
            let (parent, child) = (health.parent, health.child);
            actions.push(SupervisorAction::Mark {
                parent,
                child,
                phase,
            });
            actions.push(SupervisorAction::Notify {
                parent,
                child,
                phase,
            });
            if phase == SubagentPhase::Lost && self.policy == LostPolicy::FailAndStop {
                actions.push(SupervisorAction::Fail { parent, child });
            }
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SubagentDescriptor;
    use crate::task::TaskStatus;

    const STALL: u64 = 120_000;
    const LOST: u64 = 300_000;
    const P: SessionId = SessionId(1);

    fn sup() -> SubagentSupervisor {
        SubagentSupervisor::new(STALL, LOST, LostPolicy::MarkAndNotify)
    }

    fn descriptor(child: i64, parent: SessionId, state: TaskState) -> SubagentDescriptor {
        SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child: SessionId(child),
            job_id: format!("subagent-{child}"),
            label: format!("hijo-{child}"),
            status: TaskStatus::new(state),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        }
    }

    fn registra(s: &mut SubagentSupervisor, child: i64, state: TaskState, now: u64) {
        s.on_event(
            &LiveEvent::SubagentChanged(descriptor(child, P, state)),
            now,
        );
    }

    fn fase(child: i64, phase: SubagentPhase) -> LiveEvent {
        LiveEvent::SubagentPhaseChanged {
            parent: P,
            child: SessionId(child),
            phase,
        }
    }

    fn latido(child: i64) -> LiveEvent {
        LiveEvent::ChildEvent {
            parent: P,
            child: SessionId(child),
            event: Box::new(LiveEvent::Delta { text: "x".into() }),
        }
    }

    fn fase_de(s: &SubagentSupervisor, child: i64) -> Option<SubagentPhase> {
        s.children()
            .into_iter()
            .find(|c| c.child == SessionId(child))
            .map(|c| c.phase)
    }

    #[test]
    fn un_hijo_que_trabaja_se_registra_como_working() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        assert_eq!(fase_de(&s, 7), Some(SubagentPhase::Working));
    }

    #[test]
    fn dentro_del_umbral_no_se_hace_nada() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        assert!(s.tick(STALL - 1).is_empty());
    }

    #[test]
    fn stall_y_recuperacion_por_latido() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        let acciones = s.tick(STALL);
        assert_eq!(
            acciones,
            vec![
                SupervisorAction::Mark {
                    parent: P,
                    child: SessionId(7),
                    phase: SubagentPhase::Stalled
                },
                SupervisorAction::Notify {
                    parent: P,
                    child: SessionId(7),
                    phase: SubagentPhase::Stalled
                },
            ]
        );
        // Un latido real del hijo lo devuelve a `Working`.
        let acciones = s.on_event(&latido(7), STALL + 10);
        assert_eq!(
            acciones,
            vec![SupervisorAction::Mark {
                parent: P,
                child: SessionId(7),
                phase: SubagentPhase::Working
            }]
        );
        assert_eq!(fase_de(&s, 7), Some(SubagentPhase::Working));
        assert!(s.tick(STALL + 20).is_empty(), "el latido reinició el reloj");
    }

    #[test]
    fn lost_produce_mark_y_notify_pero_no_falla_con_la_politica_de_solo_avisar() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        s.tick(STALL);
        let acciones = s.tick(LOST);
        assert_eq!(
            acciones,
            vec![
                SupervisorAction::Mark {
                    parent: P,
                    child: SessionId(7),
                    phase: SubagentPhase::Lost
                },
                SupervisorAction::Notify {
                    parent: P,
                    child: SessionId(7),
                    phase: SubagentPhase::Lost
                },
            ]
        );
        assert!(
            s.tick(LOST + 1_000).is_empty(),
            "Lost es terminal: no se repite"
        );
    }

    #[test]
    fn con_la_politica_de_fallar_lost_pide_darlo_por_fallido() {
        let mut s = SubagentSupervisor::new(STALL, LOST, LostPolicy::FailAndStop);
        registra(&mut s, 7, TaskState::Working, 0);
        s.tick(STALL);
        let acciones = s.tick(LOST);
        assert!(acciones.contains(&SupervisorAction::Fail {
            parent: P,
            child: SessionId(7)
        }));
    }

    #[test]
    fn lo_que_publica_el_propio_supervisor_no_cuenta_como_latido() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        s.tick(STALL);
        // El driver publica `Stalled` en el bus y el supervisor lo ve de vuelta.
        s.on_event(&fase(7, SubagentPhase::Stalled), STALL + 1);
        let acciones = s.tick(LOST);
        assert!(
            acciones.iter().any(|a| matches!(
                a,
                SupervisorAction::Mark {
                    phase: SubagentPhase::Lost,
                    ..
                }
            )),
            "el eco de Stalled no reinició el reloj: {acciones:?}"
        );
    }

    #[test]
    fn esperar_al_humano_no_es_atasco() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::InputRequired, 0);
        registra(&mut s, 8, TaskState::AuthRequired, 0);
        assert_eq!(fase_de(&s, 7), Some(SubagentPhase::AwaitingParentInput));
        // Un hijo local solo pide `auth-required` para que el padre apruebe un permiso.
        assert_eq!(fase_de(&s, 8), Some(SubagentPhase::AwaitingPermission));
        assert!(s.tick(LOST * 10).is_empty());
    }

    #[test]
    fn una_herramienta_larga_no_se_marca_porque_no_emite_latidos() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        s.on_event(&fase(7, SubagentPhase::ToolRunning), 1);
        assert!(s.tick(LOST * 10).is_empty());
    }

    #[test]
    fn un_cambio_de_fase_real_es_un_latido() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        s.on_event(&fase(7, SubagentPhase::Streaming), STALL - 1);
        assert!(s.tick(STALL + 1).is_empty());
        assert_eq!(fase_de(&s, 7), Some(SubagentPhase::Streaming));
    }

    #[test]
    fn la_peticion_de_entrada_y_su_respuesta_cambian_la_fase() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        s.on_event(
            &LiveEvent::SubagentInputRequested {
                parent: P,
                child: SessionId(7),
                label: "q".into(),
            },
            1,
        );
        assert_eq!(fase_de(&s, 7), Some(SubagentPhase::AwaitingParentInput));
        s.on_event(
            &LiveEvent::SubagentInputResolved {
                parent: P,
                child: SessionId(7),
            },
            2,
        );
        assert_eq!(fase_de(&s, 7), Some(SubagentPhase::Working));
    }

    #[test]
    fn un_hijo_asentado_o_terminal_se_olvida() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 0);
        registra(&mut s, 8, TaskState::Working, 0);
        s.on_event(
            &LiveEvent::SubagentSettled {
                parent: P,
                child: SessionId(7),
                status: TaskState::Completed,
                summary: None,
            },
            1,
        );
        registra(&mut s, 8, TaskState::Failed, 2);
        assert!(s.children().is_empty());
        assert!(s.tick(LOST * 10).is_empty());
    }

    #[test]
    fn un_reloj_que_va_hacia_atras_no_dispara_nada() {
        let mut s = sup();
        registra(&mut s, 7, TaskState::Working, 1_000_000);
        assert!(s.tick(5).is_empty());
    }

    #[test]
    fn agrega_la_salud_por_padre() {
        let mut s = sup();
        let otro = SessionId(2);
        registra(&mut s, 7, TaskState::Working, 0);
        registra(&mut s, 8, TaskState::InputRequired, 0);
        registra(&mut s, 9, TaskState::Working, 0);
        s.on_event(
            &LiveEvent::SubagentChanged(descriptor(20, otro, TaskState::Working)),
            0,
        );
        s.tick(STALL);
        s.on_event(&latido(9), STALL + 1);
        let resumen = s.summary(P);
        assert_eq!(
            resumen,
            HealthSummary {
                active: 1,
                waiting: 1,
                stalled: 1,
                lost: 0
            }
        );
        assert_eq!(s.summary(otro).stalled, 1, "el otro padre tiene lo suyo");
    }

    #[test]
    fn un_evento_de_un_hijo_desconocido_se_ignora() {
        let mut s = sup();
        assert!(s.on_event(&latido(99), 0).is_empty());
        assert!(s.children().is_empty());
    }

    #[test]
    fn un_hijo_remoto_no_se_vigila_porque_no_emite_latidos() {
        let mut s = sup();
        let mut remoto = descriptor(7, P, TaskState::Working);
        remoto.agent_origin = Some("remote:https://agente.example/card".into());
        s.on_event(&LiveEvent::SubagentChanged(remoto), 0);
        assert!(
            s.children().is_empty(),
            "el remoto lo sigue su propio espejo"
        );
        assert!(s.tick(LOST * 10).is_empty());
        // Uno local con origen de proyecto sí se vigila.
        let mut local = descriptor(8, P, TaskState::Working);
        local.agent_origin = Some("project:/p/.codeloom/agents/r.md".into());
        s.on_event(&LiveEvent::SubagentChanged(local), 0);
        assert_eq!(s.children().len(), 1);
    }
}
