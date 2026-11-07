//! Política de cierre del turno: punto de extensión genérico que el padre consulta antes de dar
//! por terminado un turno que no pide herramientas.
//!
//! Cabe señalar que el núcleo no sabe qué hace una política: solo la consulta, la acota y la aplica.
//! Quien la implementa decide si el turno se cierra o si el control vuelve al modelo con un mensaje
//! interno. Las reglas que cumple el núcleo, sea cual sea la política:
//! - la cancelación gana siempre: con el turno cancelado no se reinyecta nada;
//! - una política que tarda más de [`CLOSE_GATE_TIMEOUT`], entra en pánico o falla cierra el turno;
//! - nunca hay más de [`MAX_CLOSE_REINJECTIONS`] reinyecciones por turno;
//! - el mensaje y el aviso se acotan antes de llegar al modelo o a la interfaz.
//!
//! Los subagentes locales no heredan la política: solo la tiene el agente al que se le configura
//! con `with_close_gate`.

use super::*;
use async_trait::async_trait;
use std::time::Duration;

/// Reinyecciones que se toleran en un turno antes de cerrarlo a la fuerza.
pub const MAX_CLOSE_REINJECTIONS: u32 = 3;

/// Plazo de una consulta a la política. Vencido, el turno se cierra.
pub const CLOSE_GATE_TIMEOUT: Duration = Duration::from_secs(2);

/// Bytes máximos del mensaje que la política devuelve al modelo.
pub const MAX_CLOSE_MESSAGE_BYTES: usize = 4_096;

/// Bytes que se reservan al marcador de recorte que `limit_bytes` añade.
const MARCADOR_DE_RECORTE: usize = 64;

/// Lo que la política recibe para decidir: la sesión, la carpeta del proyecto y el almacén.
#[derive(Clone)]
pub struct CloseGateCtx {
    pub session: SessionId,
    pub cwd: PathBuf,
    pub store: Arc<dyn SessionStore>,
}

/// Veredicto de la política. `notice` es un aviso para la interfaz, nunca para el modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseDecision {
    /// El turno termina. Un aviso opcional explica por qué se cerró sin más.
    Close { notice: Option<String> },
    /// El control vuelve al modelo: `message` entra al historial como mensaje interno.
    Reinject {
        message: String,
        notice: Option<String>,
    },
}

/// Política de cierre. `attempt` cuenta las reinyecciones que ya van en este turno (0, 1, 2…).
#[async_trait]
pub trait TurnCloseGate: Send + Sync {
    async fn check(&self, ctx: &CloseGateCtx, attempt: u32) -> CloseDecision;
}

/// Consulta la política aplicando las reglas del núcleo: la cancelación gana siempre, el plazo y el
/// pánico cierran el turno, el tope de reinyecciones se respeta y los textos se acotan.
///
/// La política corre en su propia tarea: un pánico queda en el `JoinHandle` y no llega al turno, y
/// un plazo vencido aborta la tarea (soltar el `JoinHandle` no la detiene).
pub async fn consult(
    gate: Arc<dyn TurnCloseGate>,
    ctx: CloseGateCtx,
    attempt: u32,
    cancel: &CancellationToken,
) -> CloseDecision {
    if cancel.is_cancelled() || attempt >= MAX_CLOSE_REINJECTIONS {
        return cerrar(None);
    }
    let tarea = tokio::spawn(async move { gate.check(&ctx, attempt).await });
    let aborto = tarea.abort_handle();
    let decision = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            aborto.abort();
            return cerrar(None);
        }
        resultado = tokio::time::timeout(CLOSE_GATE_TIMEOUT, tarea) => match resultado {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => {
                return cerrar(Some(
                    "la política de cierre falló; el turno se cierra".into(),
                ));
            }
            Err(_) => {
                aborto.abort();
                return cerrar(Some(
                    "la política de cierre no respondió a tiempo; el turno se cierra".into(),
                ));
            }
        },
    };
    // La cancelación que llegó durante la consulta gana también al resultado ya obtenido.
    if cancel.is_cancelled() {
        return cerrar(None);
    }
    acotar(decision)
}

fn cerrar(notice: Option<String>) -> CloseDecision {
    CloseDecision::Close { notice }
}

/// Acota mensaje y aviso. El límite descuenta el marcador de recorte, de modo que el texto final
/// nunca supera su tope aunque se recorte.
fn acotar(decision: CloseDecision) -> CloseDecision {
    match decision {
        CloseDecision::Close { notice } => CloseDecision::Close {
            notice: notice.map(|n| acotar_texto(&n, MAX_NOTICE_BYTES)),
        },
        CloseDecision::Reinject { message, notice } => CloseDecision::Reinject {
            message: acotar_texto(&message, MAX_CLOSE_MESSAGE_BYTES),
            notice: notice.map(|n| acotar_texto(&n, MAX_NOTICE_BYTES)),
        },
    }
}

fn acotar_texto(texto: &str, max: usize) -> String {
    limit_bytes(texto, max - MARCADOR_DE_RECORTE).into_owned()
}

impl AgentLoop {
    /// Consulta la política de cierre. `Some(mensaje)` si el control vuelve al modelo, `None` si el
    /// turno se cierra. Los avisos se publican aquí. Sin política, siempre `None`.
    pub(super) async fn consultar_cierre(
        &self,
        session: SessionId,
        attempt: u32,
        cancel: &CancellationToken,
    ) -> Result<Option<Message>> {
        let Some(gate) = self.close_gate.clone() else {
            return Ok(None);
        };
        let ctx = CloseGateCtx {
            session,
            cwd: self.cwd.clone(),
            store: self.store.clone(),
        };
        let (mensaje, aviso) = match consult(gate, ctx, attempt, cancel).await {
            CloseDecision::Close { notice } => (None, notice),
            CloseDecision::Reinject { message, notice } => (Some(message), notice),
        };
        if let Some(message) = aviso {
            self.bus.publish(LiveEvent::RuntimeNotice { message });
        }
        let Some(texto) = mensaje else {
            return Ok(None);
        };
        let mensaje = Message::user(texto);
        self.store.append_message(session, &mensaje).await?;
        self.bus.publish(LiveEvent::MessageAdded(mensaje.clone()));
        Ok(Some(mensaje))
    }
}
