//! Cotas del anfitrión: límites configurados y acotado de eventos, textos y partes.

use super::*;

/// Cada cuánto se consulta el estado mientras se espera.
pub(super) const SONDEO: Duration = Duration::from_millis(50);

/// Flujos SSE abiertos a la vez (cota propia, a revisar; véase la spec de A8c, D4).
pub const MAX_STREAMS: usize = 16;

/// Texto máximo que se devuelve por tarea (decálogo: nada sin cota llega al cliente).
pub const MAX_RESULT_BYTES: usize = 32 * 1024;

/// Etiqueta de las tareas entrantes en el almacén.
pub(super) const ETIQUETA: &str = "A2A entrante";

/// Herramientas del agente servido: solo lectura, y la lectura de ficheros confinada al proyecto.
/// Ni escritura, ni `bash`, ni jobs, ni subagentes, ni preguntas (D4 de la spec).
pub fn served_tools() -> ToolRegistry {
    let mut herramientas = ToolRegistry::new();
    herramientas.register(Arc::new(ConfinedView(ViewTool)));
    herramientas.register(Arc::new(GlobTool));
    herramientas.register(Arc::new(GrepTool));
    herramientas
}

/// Cotas del anfitrión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostLimits {
    pub max_concurrent: usize,
    pub max_task: Duration,
    /// Bytes de texto que se devuelven como máximo por tarea.
    pub max_result_bytes: usize,
}

impl HostLimits {
    pub fn from_config(serve: &ServeConfig, max_result_bytes: usize) -> Self {
        HostLimits {
            max_concurrent: serve.max_concurrent_tasks,
            max_task: Duration::from_secs(serve.max_task_secs),
            max_result_bytes,
        }
    }
}

/// Acota el texto de un evento del flujo, con el mismo criterio que `GetTask`.
pub(super) fn acotar_evento(evento: &mut WireStreamResponse, max: usize) {
    if let Some(tarea) = evento.task.as_mut() {
        acotar_texto(tarea, max);
    }
    if let Some(mensaje) = evento.message.as_mut() {
        acotar_partes(mensaje.parts.iter_mut(), max);
    }
    if let Some(cambio) = evento.status_update.as_mut() {
        if let Some(mensaje) = cambio.status.message.as_mut() {
            acotar_partes(mensaje.parts.iter_mut(), max);
        }
    }
    if let Some(cambio) = evento.artifact_update.as_mut() {
        acotar_partes(cambio.artifact.parts.iter_mut(), max);
    }
}

/// Acota el texto total de los artefactos y del mensaje de estado a `max` bytes.
pub(super) fn acotar_texto(tarea: &mut WireTask, max: usize) {
    let mensajes = tarea
        .status
        .message
        .iter_mut()
        .flat_map(|mensaje| mensaje.parts.iter_mut());
    let artefactos = tarea
        .artifacts
        .iter_mut()
        .flat_map(|artefacto| artefacto.parts.iter_mut());
    acotar_partes(artefactos.chain(mensajes), max);
}

/// Acota el texto de unas partes a `max` bytes en total, cortando en un límite de carácter.
pub(super) fn acotar_partes<'a>(partes: impl Iterator<Item = &'a mut WirePart>, max: usize) {
    let mut presupuesto = max;
    for parte in partes {
        let Some(texto) = parte.text.as_mut() else {
            continue;
        };
        if texto.len() <= presupuesto {
            presupuesto -= texto.len();
            continue;
        }
        let mut fin = presupuesto;
        while !texto.is_char_boundary(fin) {
            fin -= 1;
        }
        texto.truncate(fin);
        texto.push_str("… [truncated]");
        presupuesto = 0;
    }
}
