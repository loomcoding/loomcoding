//! Sección de prompt de ODD para el agente principal: cómo usar `odd_track` y cuándo cerrar.
//!
//! El texto es estático a propósito: la sección viaja en cada petición y un texto cambiante rompería
//! la caché del prompt. Con ODD apagado no aporta nada.

use super::state::OddState;
use loomcoding_core::agent::PromptContributor;

/// Sección que el agente principal recibe mientras ODD está encendido. Cota: 2 000 bytes.
pub const TEXTO: &str = "Desarrollo orgánico (ODD) está activo. Herramienta: odd_track.\n\
1. Antes de actuar, autoriza y explora: lee lo necesario del código y del historial antes de tocar nada.\n\
2. Prueba del resumen: si el trabajo cabe en un resumen de pocas líneas que se pueda retomar desde la petición y el git diff, no crees documento; hazlo directamente.\n\
3. Si es grande (varios pasos, varios ficheros o varias sesiones), crea el documento con odd_track (op create, con feature, objective y tasks: una lista de textos cortos, uno por paso). feature solo admite a-z, 0-9, _ y -.\n\
4. Marca cada tarea con odd_track (op done, con feature, id y evidence {text, call_id}). text es una cita corta de lo observado, por ejemplo «cargo test: 12 passed»; call_id es el id de una llamada de bash o de job de esta sesión que lo respalde. Marca solo con evidencia real: sin ella, el runtime rechaza la marca.\n\
5. Si no puedes avanzar en una tarea, márcala con odd_track (op block, con feature, id y reason).\n\
6. El cierre se bloquea mientras queden tareas sin hacer: no des el trabajo por terminado con tareas pendientes. Si necesitas la decisión de la persona usuaria, pídela y di «Necesita tu decisión».";

/// Aporta `TEXTO` al prompt de sistema del agente principal mientras ODD esté encendido.
pub struct OddPrompt {
    estado: OddState,
}

impl OddPrompt {
    pub fn new(estado: OddState) -> Self {
        Self { estado }
    }
}

impl PromptContributor for OddPrompt {
    fn section(&self) -> Option<String> {
        self.estado.is_enabled().then(|| TEXTO.to_string())
    }
}
