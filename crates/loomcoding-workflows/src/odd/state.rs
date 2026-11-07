//! Estado compartido de ODD: la feature activa por sesión y el cerrojo de E/S de los documentos.

use loomcoding_core::session::SessionId;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// Tope de sesiones recordadas en memoria. Cada entrada es una cadena corta; el tope evita que
/// un proceso de muchas sesiones crezca sin límite (decálogo de contexto y coste).
pub const MAX_SESIONES_RECORDADAS: usize = 256;

#[derive(Default)]
struct Interno {
    activas: Mutex<BTreeMap<SessionId, String>>,
    io: Mutex<()>,
}

/// Estado de ODD que comparten la herramienta y, en la tarea 3, el cierre del turno.
///
/// Es un manejador barato de clonar: todos los clones ven el mismo estado.
#[derive(Clone, Default)]
pub struct OddState {
    interno: Arc<Interno>,
}

impl OddState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Última feature tocada por la sesión, si la hay.
    ///
    // NOTA (tarea 3): este mapa vive solo en memoria. Si la feature activa debe sobrevivir a un
    // reinicio, la persistencia la decide la tarea 3; aquí no se guarda nada en disco.
    pub fn active_feature(&self, sesion: SessionId) -> Option<String> {
        self.activas().get(&sesion).cloned()
    }

    /// Anota la feature tocada por la sesión, desalojando una entrada si se supera el tope.
    pub(crate) fn recordar(&self, sesion: SessionId, feature: &str) {
        let mut activas = self.activas();
        if !activas.contains_key(&sesion) && activas.len() >= MAX_SESIONES_RECORDADAS {
            // Se desaloja la sesión con el identificador más bajo, que es la más antigua: los
            // identificadores de sesión crecen con el tiempo.
            if let Some(desalojada) = activas.keys().next().copied() {
                activas.remove(&desalojada);
            }
        }
        activas.insert(sesion, feature.to_string());
    }

    /// Cerrojo que serializa las lecturas y escrituras de documentos dentro del proceso.
    pub(crate) fn bloqueo_io(&self) -> MutexGuard<'_, ()> {
        self.interno.io.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn activas(&self) -> MutexGuard<'_, BTreeMap<SessionId, String>> {
        self.interno
            .activas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}
