//! Política de cierre de ODD: decide si el turno del padre puede acabar o si vuelve al modelo con las
//! tareas pendientes del documento de feature.
//!
//! Principio: en la duda, se cierra. Un documento ausente, roto o ilegible, una feature que no se
//! puede identificar y un interruptor apagado nunca bloquean el cierre; un fallo del propio
//! mecanismo no debe encerrar a la persona usuaria.

use super::doc::{FeatureDoc, Task};
use super::medicion::Medida;
use super::state::OddState;
use super::store;
use async_trait::async_trait;
use loomcoding_core::agent::{CloseDecision, CloseGateCtx, TurnCloseGate, MAX_CLOSE_REINJECTIONS};
use loomcoding_core::message::ContentPart;
use std::path::PathBuf;

/// Mensajes que se miran hacia atrás, desde el final, al buscar la feature de una sesión retomada.
pub const VENTANA_HISTORIAL: usize = 200;

/// Tareas pendientes que se listan en el mensaje de reinyección; el resto se resume con «+N más».
pub const MAX_PENDIENTES_LISTADAS: usize = 10;

/// Nombre de la herramienta de ODD en el historial de la sesión.
const HERRAMIENTA: &str = "odd_track";

/// Política de cierre de ODD. Comparte el estado con la herramienta `odd_track`.
pub struct OddGate {
    estado: OddState,
}

impl OddGate {
    pub fn new(estado: OddState) -> Self {
        Self { estado }
    }
}

#[async_trait]
impl TurnCloseGate for OddGate {
    async fn check(&self, ctx: &CloseGateCtx, attempt: u32) -> CloseDecision {
        decidir(&self.estado, ctx, attempt).await
    }
}

async fn decidir(estado: &OddState, ctx: &CloseGateCtx, intento: u32) -> CloseDecision {
    if !estado.is_enabled() {
        return cerrar(None);
    }
    let Some(feature) = feature_de_sesion(estado, ctx).await else {
        return cerrar(None);
    };
    let texto = match leer_documento(estado.clone(), store::ruta(&ctx.cwd, &feature)).await {
        Ok(Some(texto)) => texto,
        // Sin documento no hay tareas que exigir: se cierra.
        Ok(None) => return cerrar(None),
        Err(motivo) => return ilegible(estado, &motivo),
    };
    let doc = match FeatureDoc::parse(&texto) {
        Ok(doc) => doc,
        Err(error) => return ilegible(estado, &error.to_string()),
    };

    let pendientes = doc.pending();
    if pendientes.is_empty() {
        estado.olvidar_pendientes(ctx.session);
        let bloqueadas = doc.blocked().len();
        if bloqueadas > 0 {
            estado.contar(Medida::CierreDecision);
            return cerrar(Some(format!(
                "Necesita tu decisión: {bloqueadas} bloqueadas"
            )));
        }
        return cerrar(None);
    }

    let ids: Vec<String> = pendientes.iter().map(|tarea| tarea.id.clone()).collect();
    if estado.pendientes_de(ctx.session).as_deref() == Some(ids.as_slice()) {
        estado.contar(Medida::CierreSinProgreso);
        return cerrar(Some(
            "sin progreso en las tareas: no se insiste".to_string(),
        ));
    }

    let aviso = format!(
        "Tareas pendientes: {} — se devuelve el control al modelo (intento {}/{})",
        ids.len(),
        intento + 1,
        MAX_CLOSE_REINJECTIONS
    );
    let mensaje = mensaje_de_cierre(&feature, &pendientes);
    estado.recordar_pendientes(ctx.session, ids);
    estado.contar(Medida::CierreBloqueado);
    CloseDecision::Reinject {
        message: mensaje,
        notice: Some(aviso),
    }
}

fn cerrar(notice: Option<String>) -> CloseDecision {
    CloseDecision::Close { notice }
}

/// Documento ilegible: se cuenta, se avisa y se cierra sin bloquear.
fn ilegible(estado: &OddState, motivo: &str) -> CloseDecision {
    estado.contar(Medida::CierreIlegible);
    cerrar(Some(aviso_ilegible(motivo)))
}

fn aviso_ilegible(motivo: &str) -> String {
    format!("documento de feature ilegible, no se bloquea el cierre: {motivo}")
}

/// Mensaje que vuelve al modelo: qué queda, y cómo cerrarlo con prueba o pidiendo decisión.
fn mensaje_de_cierre(feature: &str, pendientes: &[&Task]) -> String {
    let mut mensaje = format!(
        "[Desarrollo orgánico] La feature `{feature}` tiene {} tareas sin terminar. No des el trabajo por cerrado; continúa con ellas.\nPendientes:\n",
        pendientes.len()
    );
    for tarea in pendientes.iter().take(MAX_PENDIENTES_LISTADAS) {
        mensaje.push_str(&format!("- {}: {}\n", tarea.id, tarea.text));
    }
    let resto = pendientes.len().saturating_sub(MAX_PENDIENTES_LISTADAS);
    if resto > 0 {
        mensaje.push_str(&format!("(+{resto} más)\n"));
    }
    mensaje.push_str(
        "Para cada una: márcala con odd_track (op done, con evidence {text, call_id} de una llamada de bash o job de esta sesión; o op block, con reason, si no puedes avanzar). Si necesitas la decisión de la persona usuaria, pídela y di «Necesita tu decisión».",
    );
    mensaje
}

/// Feature de la sesión: la de la memoria o, tras un reinicio, la última `odd_track` del historial
/// reciente. Un nombre inválido en el historial no se usa. Sin feature, `None`.
async fn feature_de_sesion(estado: &OddState, ctx: &CloseGateCtx) -> Option<String> {
    if let Some(feature) = estado.active_feature(ctx.session) {
        return Some(feature);
    }
    let mensajes = ctx
        .store
        .recent_messages(ctx.session, VENTANA_HISTORIAL)
        .await
        .ok()?;
    let nombre = mensajes
        .iter()
        .rev()
        .flat_map(|mensaje| mensaje.parts.iter().rev())
        .find_map(|parte| match parte {
            ContentPart::ToolCall { name, input, .. } if name == HERRAMIENTA => Some(
                input
                    .get("feature")
                    .and_then(|valor| valor.as_str())
                    .unwrap_or_default()
                    .to_string(),
            ),
            _ => None,
        })?;
    store::validar_feature(&nombre).ok()?;
    estado.recordar(ctx.session, &nombre);
    Some(nombre)
}

/// Lee el documento bajo el cerrojo de E/S, fuera del hilo del ejecutor.
async fn leer_documento(estado: OddState, ruta: PathBuf) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let _cerrojo = estado.bloqueo_io();
        store::leer(&ruta)
    })
    .await
    .unwrap_or_else(|error| Err(format!("la lectura del documento no terminó: {error}")))
}
