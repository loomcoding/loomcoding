//! Observador de herramientas: punto de extensión genérico que el padre consulta antes de despachar
//! las herramientas de una ronda, para publicar en la interfaz el flujo de trabajo activo.
//!
//! Cabe señalar que el núcleo no sabe qué observa quien lo implementa: solo lo consulta, lo acota y
//! publica el resultado como `LiveEvent::FlowChanged`. Las reglas que cumple el núcleo, sea cual sea
//! el observador:
//! - la cancelación gana siempre: con el turno cancelado no se publica nada;
//! - un observador que tarda más de [`TOOL_OBSERVER_TIMEOUT`], entra en pánico o falla se descarta;
//!   el turno sigue igual;
//! - `label` y `phase` se acotan antes de llegar a la interfaz.
//!
//! Los subagentes locales no heredan el observador: solo lo tiene el agente al que se le configura
//! con `with_tool_observer`.

use super::*;
use crate::flow::FlowSummary;
use async_trait::async_trait;
use std::time::Duration;

/// Plazo de una consulta al observador. Vencido, la consulta se descarta.
pub const TOOL_OBSERVER_TIMEOUT: Duration = Duration::from_millis(500);

/// Bytes máximos de `label` y `phase` en el evento.
const MAX_FLOW_TEXT_BYTES: usize = 128;

/// Bytes que se reservan al marcador de recorte que `limit_bytes` añade.
const MARCADOR_DE_RECORTE: usize = 64;

/// Lo que el observador recibe: la sesión, el nombre de la herramienta y sus argumentos.
#[derive(Clone)]
pub struct ToolObserveCtx {
    pub session: SessionId,
    pub name: String,
    pub input: serde_json::Value,
}

/// Observador de herramientas. Devuelve el resumen del flujo si hay algo que publicar.
#[async_trait]
pub trait ToolObserver: Send + Sync {
    async fn observe(&self, ctx: &ToolObserveCtx) -> Option<FlowSummary>;
}

/// Consulta al observador aplicando las reglas del núcleo: la cancelación gana, el plazo y el pánico
/// descartan la consulta.
///
/// El observador corre en su propia tarea: un pánico queda en el `JoinHandle` y no llega al turno, y
/// un plazo vencido aborta la tarea (soltar el `JoinHandle` no la detiene).
pub async fn consult_observer(
    observer: Arc<dyn ToolObserver>,
    ctx: ToolObserveCtx,
    cancel: &CancellationToken,
) -> Option<FlowSummary> {
    if cancel.is_cancelled() {
        return None;
    }
    let tarea = tokio::spawn(async move { observer.observe(&ctx).await });
    let aborto = tarea.abort_handle();
    tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            aborto.abort();
            None
        }
        resultado = tokio::time::timeout(TOOL_OBSERVER_TIMEOUT, tarea) => match resultado {
            Ok(Ok(resumen)) if !cancel.is_cancelled() => resumen,
            Ok(Ok(_)) | Ok(Err(_)) => None,
            Err(_) => {
                aborto.abort();
                None
            }
        },
    }
}

/// Acota los textos del resumen antes de publicarlos. El límite descuenta el marcador de recorte.
fn acotar(mut resumen: FlowSummary) -> FlowSummary {
    let tope = MAX_FLOW_TEXT_BYTES - MARCADOR_DE_RECORTE;
    resumen.label = limit_bytes(&resumen.label, tope).into_owned();
    resumen.phase = limit_bytes(&resumen.phase, tope).into_owned();
    resumen
}

impl AgentLoop {
    /// Consulta al observador para cada llamada de la ronda y publica un `FlowChanged` por cada
    /// resumen recibido. Sin observador, no hace nada.
    pub(super) async fn observar_herramientas(
        &self,
        calls: &[(String, String, serde_json::Value)],
        session: SessionId,
        cancel: &CancellationToken,
    ) {
        let Some(observer) = self.tool_observer.clone() else {
            return;
        };
        for (_id, name, input) in calls {
            let ctx = ToolObserveCtx {
                session,
                name: name.clone(),
                input: input.clone(),
            };
            if let Some(resumen) = consult_observer(observer.clone(), ctx, cancel).await {
                self.bus.publish(LiveEvent::FlowChanged {
                    summary: acotar(resumen),
                    actor: "loom".into(),
                    detail: String::new(),
                });
            }
        }
    }
}
