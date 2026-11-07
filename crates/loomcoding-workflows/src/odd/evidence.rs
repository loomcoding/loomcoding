//! Evidencia de una casilla: el identificador de una llamada de herramienta de esta sesión.

use super::doc::MAX_EVIDENCE_BYTES;
use loomcoding_core::message::ContentPart;
use loomcoding_core::session::{SessionId, SessionStore};
use std::sync::Arc;

/// Herramientas cuyas llamadas sirven de evidencia para marcar una tarea.
pub const HERRAMIENTAS_EVIDENCIA: &[&str] = &["bash", "job_start", "job_output"];

/// Mensajes recientes de la sesión en los que se busca la llamada que respalda una evidencia. Es una
/// cota constante (decálogo de contexto y coste): una llamada más antigua se rechaza, con un motivo que
/// dice que está fuera de la ventana.
pub const VENTANA_EVIDENCIA: usize = 500;

/// Comprueba que `call_id` es una llamada de herramienta de `sesion` dentro de la ventana reciente
/// ([`VENTANA_EVIDENCIA`] mensajes) y que su herramienta está en [`HERRAMIENTAS_EVIDENCIA`]. Devuelve el
/// nombre de la herramienta.
///
/// NOTA: el runtime comprueba que la llamada existe y que su herramienta es de las permitidas. NO
/// comprueba que el texto de la evidencia sea cierto: eso es responsabilidad del modelo.
///
/// Sin almacén no hay manera de comprobar nada, así que se rechaza: nunca se inventa un éxito.
pub async fn comprobar_llamada(
    store: Option<&Arc<dyn SessionStore>>,
    sesion: SessionId,
    call_id: &str,
) -> Result<String, String> {
    let Some(store) = store else {
        return Err(
            "no hay historial de sesión disponible, así que no puedo comprobar la evidencia; \
             la casilla no se marca"
                .to_string(),
        );
    };
    // Solo la ventana reciente: la llamada tiene que ser de esta sesión y estar a la vista.
    let mensajes = store
        .recent_messages(sesion, VENTANA_EVIDENCIA)
        .await
        .map_err(|e| format!("no pude leer el historial de la sesión: {e}"))?;
    let nombre = mensajes
        .iter()
        .flat_map(|m| m.parts.iter())
        .find_map(|parte| match parte {
            ContentPart::ToolCall { id, name, .. } if id == call_id => Some(name.clone()),
            _ => None,
        });
    let Some(nombre) = nombre else {
        return Err(format!(
            "la llamada `{call_id}` no existe en esta sesión (llamada fuera de la ventana reciente \
             de {VENTANA_EVIDENCIA} mensajes, o identificador inexistente); use el id de una llamada \
             de herramienta reciente (bash, job_start o job_output)"
        ));
    };
    if HERRAMIENTAS_EVIDENCIA.contains(&nombre.as_str()) {
        Ok(nombre)
    } else {
        Err(format!(
            "la llamada `{call_id}` usó la herramienta `{nombre}`, que no sirve de evidencia; \
             valen: {}",
            HERRAMIENTAS_EVIDENCIA.join(", ")
        ))
    }
}

/// Bytes máximos del identificador de una llamada.
pub const MAX_IDENTIFICADOR_LLAMADA: usize = 128;

/// Forma guardada de la evidencia: «texto [call: ID]». Acota el conjunto a [`MAX_EVIDENCE_BYTES`].
pub fn formato(texto: &str, call_id: &str) -> Result<String, String> {
    let guardada = format!("{texto} [call: {call_id}]");
    if guardada.len() > MAX_EVIDENCE_BYTES {
        return Err(format!(
            "la evidencia con su identificador ocupa {} bytes; el máximo guardado es \
             {MAX_EVIDENCE_BYTES}. Acorte el texto",
            guardada.len()
        ));
    }
    Ok(guardada)
}
