//! Fase local del subagente: la vista rica que el runtime mantiene y que se
//! proyecta a A2A al publicar o guardar.
//!
//! Cabe señalar que este enumerado NO sustituye a [`TaskState`]: convive con él.
//! `TaskState` es el modelo de tarea de A2A, con sus ocho variantes; `SubagentPhase`
//! es lo que la interfaz necesita para no colapsar un turno entero en un único
//! `working`. La proyección va en un solo sentido por cada consulta, y el inverso
//! existe para los hijos remotos, de los que solo llega el estado del protocolo.
//!
//! El módulo es puro: sin E/S, sin red y sin reloj. El vigilante de latidos
//! (`Stalled`/`Lost`) vive fuera y aquí solo se declara qué transiciones admite.

use crate::task::TaskState;
use serde::{Deserialize, Serialize};

/// Fase local del subagente.
///
/// Cabe señalar que `Unknown` no es terminal a propósito: un agente remoto puede
/// ir por delante de esta versión del protocolo y reportar después algo que sí
/// entendemos, así que la fase se actualiza con normalidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubagentPhase {
    /// Creado, aún no lanzado.
    Queued,
    /// Arrancando el turno: sesión o tarea creada, aún sin primer token.
    Starting,
    /// Turno del modelo en curso (esperando o recibiendo tokens).
    Streaming,
    /// El modelo está razonando (bloques de razonamiento explícitos).
    Reasoning,
    /// Ejecutando una herramienta (el subagente no consume tokens mientras tanto).
    ToolRunning,
    /// Bloqueado esperando aprobación de permiso del usuario.
    AwaitingPermission,
    /// Esperando entrada del padre (petición A2A `input-required`).
    AwaitingParentInput,
    /// Esperando credenciales de un agente remoto A2A (`auth-required`).
    AwaitingCredentials,
    /// En ejecución activa sin sub-fase conocida (fallback de motores que no la exponen).
    Working,
    /// Asentado con éxito (resumen disponible).
    SettledOk,
    /// Asentado con error (resumen o error disponible).
    SettledErr,
    /// El agente remoto rechazó la tarea (`rejected`).
    Rejected,
    /// Cancelado por el padre o el runtime.
    Canceled,
    /// Sin latido durante más del umbral: posible atasco.
    Stalled,
    /// Sin señal y sin forma de recuperar: perdido.
    Lost,
    /// Estado A2A remoto no reconocido por esta versión (compatibilidad futura).
    Unknown,
}

impl SubagentPhase {
    /// Las dieciséis fases, en el orden del spec.
    pub const ALL: [SubagentPhase; 16] = [
        SubagentPhase::Queued,
        SubagentPhase::Starting,
        SubagentPhase::Streaming,
        SubagentPhase::Reasoning,
        SubagentPhase::ToolRunning,
        SubagentPhase::AwaitingPermission,
        SubagentPhase::AwaitingParentInput,
        SubagentPhase::AwaitingCredentials,
        SubagentPhase::Working,
        SubagentPhase::SettledOk,
        SubagentPhase::SettledErr,
        SubagentPhase::Rejected,
        SubagentPhase::Canceled,
        SubagentPhase::Stalled,
        SubagentPhase::Lost,
        SubagentPhase::Unknown,
    ];

    /// Dícese de las fases que ya no admiten transiciones posteriores.
    ///
    /// `Unknown` queda fuera a propósito: no afirma ni éxito ni fallo, así que
    /// todavía puede resolverse a una fase terminal cuando llegue el estado real.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::SettledOk | Self::SettledErr | Self::Rejected | Self::Canceled | Self::Lost
        )
    }

    /// Proyección a las ocho variantes de A2A.
    ///
    /// Cabe señalar que `Queued` y `Starting` proyectan a `Submitted`, no a
    /// `Working`: la tarea está aceptada pero el turno aún no ha empezado, y
    /// afirmar `working` ahí sería mentir al observador remoto.
    pub fn to_task_state(self) -> TaskState {
        match self {
            Self::Queued | Self::Starting => TaskState::Submitted,
            Self::Streaming
            | Self::Reasoning
            | Self::ToolRunning
            | Self::AwaitingPermission
            | Self::Working
            | Self::Stalled
            | Self::Unknown => TaskState::Working,
            Self::AwaitingParentInput => TaskState::InputRequired,
            Self::AwaitingCredentials => TaskState::AuthRequired,
            Self::SettledOk => TaskState::Completed,
            Self::SettledErr | Self::Lost => TaskState::Failed,
            Self::Rejected => TaskState::Rejected,
            Self::Canceled => TaskState::Canceled,
        }
    }

    /// Fase a partir del estado A2A, para los hijos remotos.
    ///
    /// A2A no distingue sub-fases, así que `Working` es el destino conservador:
    /// sabemos que trabaja, no en qué punto está.
    pub fn from_task_state(state: TaskState) -> SubagentPhase {
        match state {
            TaskState::Submitted => Self::Queued,
            TaskState::Working => Self::Working,
            TaskState::InputRequired => Self::AwaitingParentInput,
            TaskState::AuthRequired => Self::AwaitingCredentials,
            TaskState::Completed => Self::SettledOk,
            TaskState::Failed => Self::SettledErr,
            TaskState::Canceled => Self::Canceled,
            TaskState::Rejected => Self::Rejected,
        }
    }

    /// Fase a partir del nombre de estado del cable A2A.
    ///
    /// Acepta tanto los nombres del proto v1.0 (`TASK_STATE_*`) como los del
    /// cable v0.3 (`input-required` y compañía). Lo no reconocido cae en
    /// `Unknown` en vez de fallar: un par más nuevo no debe romper la interfaz.
    pub fn from_remote_state(name: &str) -> SubagentPhase {
        if let Some(state) = TaskState::from_proto_name(name) {
            return Self::from_task_state(state);
        }
        match name {
            "submitted" => Self::Queued,
            "working" => Self::Working,
            "input-required" => Self::AwaitingParentInput,
            "auth-required" => Self::AwaitingCredentials,
            "completed" => Self::SettledOk,
            "failed" => Self::SettledErr,
            "canceled" | "cancelled" => Self::Canceled,
            "rejected" => Self::Rejected,
            _ => Self::Unknown,
        }
    }

    /// Nombre corto de la fase, con el que se persiste en SQLite: el nombre de la
    /// variante en minúsculas, p. ej. `queued`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Streaming => "streaming",
            Self::Reasoning => "reasoning",
            Self::ToolRunning => "toolrunning",
            Self::AwaitingPermission => "awaitingpermission",
            Self::AwaitingParentInput => "awaitingparentinput",
            Self::AwaitingCredentials => "awaitingcredentials",
            Self::Working => "working",
            Self::SettledOk => "settledok",
            Self::SettledErr => "settlederr",
            Self::Rejected => "rejected",
            Self::Canceled => "canceled",
            Self::Stalled => "stalled",
            Self::Lost => "lost",
            Self::Unknown => "unknown",
        }
    }

    /// Fase a partir de su nombre corto. Lo no reconocido cae en `Unknown` en vez
    /// de fallar: una fila escrita por una versión más nueva no debe romper la
    /// rehidratación.
    ///
    /// Cabe señalar que no se implementa `std::str::FromStr` a propósito: ese trait
    /// devuelve `Result`, y aquí un nombre irreconocible no es un error sino una
    /// versión más nueva del dato, que debe poder leerse igual.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(name: &str) -> SubagentPhase {
        Self::ALL
            .into_iter()
            .find(|phase| phase.as_str() == name)
            .unwrap_or(Self::Unknown)
    }

    /// Si la transición de esta fase a `next` es legal.
    ///
    /// Las reglas duras del spec: de una fase terminal no se sale nunca, y
    /// `Stalled → Working` es la única recuperación permitida (y solo la
    /// dispara un latido real del subagente, no el vigilante).
    pub fn can_transition_to(self, next: SubagentPhase) -> bool {
        if self.is_terminal() {
            return false;
        }
        if self == next {
            return true;
        }
        // Cualquier fase no terminal puede cancelarse o perderse de vista.
        if matches!(next, Self::Canceled | Self::Unknown) {
            return true;
        }
        match self {
            Self::Queued => matches!(next, Self::Starting | Self::Working),
            Self::Starting => matches!(
                next,
                Self::Streaming
                    | Self::ToolRunning
                    | Self::AwaitingCredentials
                    | Self::Stalled
                    | Self::Working
            ),
            Self::Streaming => matches!(
                next,
                Self::Reasoning
                    | Self::ToolRunning
                    | Self::AwaitingParentInput
                    | Self::AwaitingCredentials
                    | Self::SettledOk
                    | Self::SettledErr
                    | Self::Rejected
                    | Self::Stalled
                    | Self::Working
            ),
            Self::Reasoning => matches!(
                next,
                Self::Streaming
                    | Self::ToolRunning
                    | Self::AwaitingParentInput
                    | Self::AwaitingCredentials
                    | Self::SettledOk
                    | Self::SettledErr
                    | Self::Rejected
                    | Self::Stalled
                    | Self::Working
            ),
            Self::ToolRunning => matches!(
                next,
                Self::Streaming
                    | Self::Reasoning
                    | Self::AwaitingPermission
                    | Self::AwaitingParentInput
                    | Self::AwaitingCredentials
                    | Self::SettledOk
                    | Self::SettledErr
                    | Self::Rejected
                    | Self::Stalled
                    | Self::Working
            ),
            Self::AwaitingPermission => matches!(
                next,
                Self::ToolRunning | Self::Streaming | Self::Stalled | Self::Working
            ),
            Self::AwaitingParentInput => matches!(
                next,
                Self::Streaming | Self::ToolRunning | Self::Stalled | Self::Working
            ),
            Self::AwaitingCredentials => matches!(
                next,
                Self::Streaming | Self::ToolRunning | Self::Stalled | Self::Working
            ),
            // `Working` es el fallback: admite cualquier fase no terminal salvo
            // volver a `Queued`, que sería retroceder a antes de arrancar.
            Self::Working => !matches!(next, Self::Queued),
            // La única salida de `Stalled` es el latido (o perderse).
            Self::Stalled => matches!(next, Self::Working | Self::Lost),
            // `Unknown` se resuelve a cualquier fase, terminal incluida: no
            // afirma nada, así que el estado real que llegue después manda.
            Self::Unknown => true,
            Self::SettledOk | Self::SettledErr | Self::Rejected | Self::Canceled | Self::Lost => {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todas_las_fases_proyectan() {
        for phase in SubagentPhase::ALL {
            let state = phase.to_task_state();
            assert!(
                TaskState::ALL.contains(&state),
                "{phase:?} proyecta a un estado que no es de A2A"
            );
        }
    }

    #[test]
    fn todos_los_task_state_tienen_fase() {
        for state in TaskState::ALL {
            let phase = SubagentPhase::from_task_state(state);
            assert!(
                SubagentPhase::ALL.contains(&phase),
                "{state:?} no tiene fase"
            );
        }
        // Los dos que el plan nombra explícitamente por ser los que se olvidan.
        assert_eq!(
            SubagentPhase::from_task_state(TaskState::AuthRequired),
            SubagentPhase::AwaitingCredentials
        );
        assert_eq!(
            SubagentPhase::from_task_state(TaskState::Rejected),
            SubagentPhase::Rejected
        );
    }

    #[test]
    fn roundtrip_no_terminal_incompatible() {
        for phase in SubagentPhase::ALL {
            if phase.is_terminal() {
                continue;
            }
            let back = SubagentPhase::from_task_state(phase.to_task_state());
            assert!(
                !back.is_terminal(),
                "{phase:?} no es terminal pero su ida y vuelta da {back:?}, que sí lo es"
            );
        }
    }

    #[test]
    fn transiciones_ilegal() {
        // No se retrocede a `Queued` desde una fase ya arrancada.
        assert!(!SubagentPhase::Streaming.can_transition_to(SubagentPhase::Queued));
        assert!(!SubagentPhase::Working.can_transition_to(SubagentPhase::Queued));
        // De una terminal no se sale, ni siquiera a otra terminal.
        for terminal in SubagentPhase::ALL.into_iter().filter(|p| p.is_terminal()) {
            for next in SubagentPhase::ALL {
                assert!(
                    !terminal.can_transition_to(next),
                    "{terminal:?} no debería poder ir a {next:?}"
                );
            }
        }
        // `Stalled` solo sale por latido o por pérdida.
        assert!(!SubagentPhase::Stalled.can_transition_to(SubagentPhase::Streaming));
        assert!(!SubagentPhase::Stalled.can_transition_to(SubagentPhase::SettledOk));
    }

    #[test]
    fn desconocido_es_unknown() {
        assert_eq!(
            SubagentPhase::from_remote_state("estado-que-no-existe"),
            SubagentPhase::Unknown
        );
        assert_eq!(SubagentPhase::from_remote_state(""), SubagentPhase::Unknown);
        // Y no entra en pánico con nada raro.
        assert_eq!(
            SubagentPhase::from_remote_state("TASK_STATE_QUE_NO_EXISTE"),
            SubagentPhase::Unknown
        );
    }

    #[test]
    fn working_es_fallback() {
        assert_eq!(
            SubagentPhase::from_task_state(TaskState::Working),
            SubagentPhase::Working
        );
        assert_eq!(
            SubagentPhase::from_remote_state("working"),
            SubagentPhase::Working
        );
    }

    #[test]
    fn unknown_no_es_terminal_y_se_resuelve() {
        assert!(!SubagentPhase::Unknown.is_terminal());
        assert!(SubagentPhase::Unknown.can_transition_to(SubagentPhase::Streaming));
        assert!(SubagentPhase::Unknown.can_transition_to(SubagentPhase::SettledOk));
    }

    #[test]
    fn stalled_se_recupera_solo_con_latido() {
        assert!(SubagentPhase::Stalled.can_transition_to(SubagentPhase::Working));
        assert!(SubagentPhase::Stalled.can_transition_to(SubagentPhase::Lost));
        assert!(SubagentPhase::Stalled.can_transition_to(SubagentPhase::Canceled));
    }

    #[test]
    fn queued_y_starting_proyectan_a_submitted() {
        assert_eq!(SubagentPhase::Queued.to_task_state(), TaskState::Submitted);
        assert_eq!(
            SubagentPhase::Starting.to_task_state(),
            TaskState::Submitted
        );
        assert_eq!(
            SubagentPhase::from_task_state(TaskState::Submitted),
            SubagentPhase::Queued
        );
    }

    #[test]
    fn nombres_del_cable_v03() {
        assert_eq!(
            SubagentPhase::from_remote_state("input-required"),
            SubagentPhase::AwaitingParentInput
        );
        assert_eq!(
            SubagentPhase::from_remote_state("auth-required"),
            SubagentPhase::AwaitingCredentials
        );
        assert_eq!(
            SubagentPhase::from_remote_state("TASK_STATE_AUTH_REQUIRED"),
            SubagentPhase::AwaitingCredentials
        );
    }
}
