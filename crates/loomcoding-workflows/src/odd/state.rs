//! Estado compartido de ODD: la feature activa por sesión, las tareas pendientes vistas en la última
//! reinyección del cierre, el interruptor y el cerrojo de E/S de los documentos.

use loomcoding_core::session::SessionId;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Tope de sesiones recordadas en memoria. Cada entrada es una cadena corta o una lista acotada de
/// identificadores; el tope evita que un proceso de muchas sesiones crezca sin límite (decálogo de
/// contexto y coste).
pub const MAX_SESIONES_RECORDADAS: usize = 256;

struct Interno {
    activas: Mutex<BTreeMap<SessionId, String>>,
    /// Identificadores pendientes vistos en la última reinyección de cada sesión. Si el cierre
    /// vuelve a ver exactamente los mismos, el modelo no avanzó y no se insiste.
    pendientes: Mutex<BTreeMap<SessionId, Vec<String>>>,
    /// Última fase de ODD publicada por sesión (véase `observer`).
    fases: Mutex<BTreeMap<SessionId, &'static str>>,
    habilitado: AtomicBool,
    io: Mutex<()>,
}

impl Default for Interno {
    fn default() -> Self {
        Self {
            activas: Mutex::new(BTreeMap::new()),
            pendientes: Mutex::new(BTreeMap::new()),
            fases: Mutex::new(BTreeMap::new()),
            // ODD viene activo; el interruptor solo lo apaga.
            habilitado: AtomicBool::new(true),
            io: Mutex::new(()),
        }
    }
}

/// Estado de ODD que comparten la herramienta y la política de cierre.
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

    /// Enciende o apaga ODD. Apagado, la política de cierre nunca bloquea el turno.
    pub fn set_enabled(&self, habilitado: bool) {
        self.interno.habilitado.store(habilitado, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.interno.habilitado.load(Ordering::SeqCst)
    }

    /// Última feature tocada por la sesión en esta memoria, si la hay. No mira el historial: quien
    /// necesite recuperarla tras un reinicio lo hace con el historial de la sesión (véase `gate`).
    pub fn active_feature(&self, sesion: SessionId) -> Option<String> {
        self.activas().get(&sesion).cloned()
    }

    /// Anota la feature tocada por la sesión, desalojando una entrada si se supera el tope.
    pub(crate) fn recordar(&self, sesion: SessionId, feature: &str) {
        insertar_acotado(&mut self.activas(), sesion, feature.to_string());
    }

    /// Identificadores pendientes de la última reinyección de la sesión, si la hubo.
    pub(crate) fn pendientes_de(&self, sesion: SessionId) -> Option<Vec<String>> {
        self.pendientes().get(&sesion).cloned()
    }

    /// Anota los pendientes de una reinyección. Quien llama acota la lista: el documento tiene como
    /// mucho `MAX_TASKS` tareas.
    pub(crate) fn recordar_pendientes(&self, sesion: SessionId, ids: Vec<String>) {
        insertar_acotado(&mut self.pendientes(), sesion, ids);
    }

    /// Olvida los pendientes: el siguiente cierre empieza de cero.
    pub(crate) fn olvidar_pendientes(&self, sesion: SessionId) {
        self.pendientes().remove(&sesion);
    }

    /// Última fase publicada para la sesión, si la hubo.
    pub(crate) fn fase_publicada(&self, sesion: SessionId) -> Option<&'static str> {
        self.fases().get(&sesion).copied()
    }

    /// Anota la fase publicada para la sesión, desalojando una entrada si se supera el tope.
    pub(crate) fn recordar_fase(&self, sesion: SessionId, fase: &'static str) {
        insertar_acotado(&mut self.fases(), sesion, fase);
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

    fn pendientes(&self) -> MutexGuard<'_, BTreeMap<SessionId, Vec<String>>> {
        self.interno
            .pendientes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn fases(&self) -> MutexGuard<'_, BTreeMap<SessionId, &'static str>> {
        self.interno.fases.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Inserta `valor` para `sesion`. Si la sesión es nueva y el mapa está lleno, desaloja la de
/// identificador más bajo, que es la más antigua: los identificadores de sesión crecen con el tiempo.
fn insertar_acotado<V>(mapa: &mut BTreeMap<SessionId, V>, sesion: SessionId, valor: V) {
    if !mapa.contains_key(&sesion) && mapa.len() >= MAX_SESIONES_RECORDADAS {
        if let Some(desalojada) = mapa.keys().next().copied() {
            mapa.remove(&desalojada);
        }
    }
    mapa.insert(sesion, valor);
}
