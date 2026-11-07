//! Estado compartido de ODD: la feature activa por sesión, las tareas pendientes vistas en la última
//! reinyección del cierre, el interruptor y el cerrojo de E/S de los documentos.

use super::medicion::{Contadores, Medida, OddStats};
use loomcoding_core::session::SessionId;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Tope de sesiones recordadas en memoria. Cada entrada es una cadena corta o una lista acotada de
/// identificadores; el tope evita que un proceso de muchas sesiones crezca sin límite (decálogo de
/// contexto y coste).
pub const MAX_SESIONES_RECORDADAS: usize = 256;

/// Última publicación de ODD para una sesión: la fase y la etiqueta completa que vio la interfaz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Publicado {
    pub fase: &'static str,
    pub etiqueta: String,
}

struct Interno {
    activas: Mutex<BTreeMap<SessionId, String>>,
    /// Identificadores pendientes vistos en la última reinyección de cada sesión. Si el cierre
    /// vuelve a ver exactamente los mismos, el modelo no avanzó y no se insiste.
    pendientes: Mutex<BTreeMap<SessionId, Vec<String>>>,
    /// Última publicación de ODD por sesión (véase `observer`). La clave para no republicar es la
    /// etiqueta completa, que lleva el avance.
    publicados: Mutex<BTreeMap<SessionId, Publicado>>,
    /// Avance del documento activo por sesión: (tareas hechas, total).
    progresos: Mutex<BTreeMap<SessionId, (usize, usize)>>,
    habilitado: AtomicBool,
    io: Mutex<()>,
    contadores: Contadores,
}

impl Default for Interno {
    fn default() -> Self {
        Self {
            activas: Mutex::new(BTreeMap::new()),
            pendientes: Mutex::new(BTreeMap::new()),
            publicados: Mutex::new(BTreeMap::new()),
            progresos: Mutex::new(BTreeMap::new()),
            // ODD viene activo; el interruptor solo lo apaga.
            habilitado: AtomicBool::new(true),
            io: Mutex::new(()),
            contadores: Contadores::default(),
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

    /// Fotografía de los contadores de medición desde que arrancó el proceso.
    pub fn estadisticas(&self) -> OddStats {
        self.interno.contadores.fotografia()
    }

    /// Suma una medición y la emite al log (destino `odd`). Solo viajan el nombre fijo del evento y
    /// el total: ni rutas, ni texto de tareas, ni identificadores de sesión.
    pub(crate) fn contar(&self, medida: Medida) {
        let total = self.interno.contadores.sumar(medida);
        tracing::info!(target: "odd", evento = medida.nombre(), total = total, "medición de ODD");
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

    /// Última publicación de ODD para la sesión, si la hubo.
    pub(crate) fn publicado(&self, sesion: SessionId) -> Option<Publicado> {
        self.publicados().get(&sesion).cloned()
    }

    /// Anota lo publicado para la sesión, desalojando una entrada si se supera el tope.
    pub(crate) fn recordar_publicado(&self, sesion: SessionId, fase: &'static str, etiqueta: &str) {
        let publicado = Publicado {
            fase,
            etiqueta: etiqueta.to_string(),
        };
        insertar_acotado(&mut self.publicados(), sesion, publicado);
    }

    /// Avance del documento activo de la sesión (hechas, total), si hay documento.
    pub(crate) fn progreso_de(&self, sesion: SessionId) -> Option<(usize, usize)> {
        self.progresos().get(&sesion).copied()
    }

    /// Anota el avance tras una operación sobre el documento.
    pub(crate) fn recordar_progreso(&self, sesion: SessionId, avance: (usize, usize)) {
        insertar_acotado(&mut self.progresos(), sesion, avance);
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

    fn publicados(&self) -> MutexGuard<'_, BTreeMap<SessionId, Publicado>> {
        self.interno
            .publicados
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn progresos(&self) -> MutexGuard<'_, BTreeMap<SessionId, (usize, usize)>> {
        self.interno
            .progresos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
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
