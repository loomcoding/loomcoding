//! Medición de la compresión de salida (Tarea 5): ahorro por regla y re-peticiones.
//!
//! Solo se guardan nombres de regla, cifras y una huella numérica de la última llamada filtrada.
//! Nunca el comando ni una ruta: la huella no permite reconstruir el texto y no sale del proceso.

use serde_json::Value;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
#[cfg(not(test))]
use std::sync::OnceLock;
use std::sync::{Mutex, MutexGuard};

/// Usos mínimos de una regla antes de juzgarla (spec, decisión 4).
pub const USOS_MINIMOS: u64 = 10;

/// Porcentaje de re-peticiones por encima del cual una regla se desactiva sola (spec, decisión 4).
pub const UMBRAL_REPETICION_PCT: u64 = 20;

/// Contadores de una regla. Sin datos personales: solo cifras y un indicador de estado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contadores {
    pub usos: u64,
    pub repeticiones: u64,
    pub bytes_originales: u64,
    pub bytes_enviados: u64,
    pub desactivada: bool,
    avisada: bool,
}

/// Huella numérica del comando (o del identificador de job), solo en memoria. No permite recuperar
/// el texto y nunca se muestra ni se persiste.
pub fn huella_de(input: &Value) -> u64 {
    let clave = input["command"]
        .as_str()
        .or_else(|| input["id"].as_str())
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    clave.hash(&mut hasher);
    hasher.finish()
}

/// Estado compartido del filtro. Seguro entre llamadas concurrentes gracias al `Mutex`.
#[derive(Debug, Default)]
pub struct FilterStats {
    estado: Mutex<Estado>,
}

#[derive(Debug, Default)]
struct Estado {
    reglas: BTreeMap<&'static str, Contadores>,
    /// Última salida filtrada: si la siguiente llamada la repite o pide `raw`, es una re-petición.
    ultima: Option<Ultima>,
}

#[derive(Debug, Clone, Copy)]
struct Ultima {
    regla: &'static str,
    huella: u64,
}

impl FilterStats {
    fn cerrojo(&self) -> MutexGuard<'_, Estado> {
        self.estado
            .lock()
            .unwrap_or_else(|veneno| veneno.into_inner())
    }

    /// Indica si la regla quedó desactivada para el resto del proceso.
    pub fn desactivada(&self, regla: &str) -> bool {
        self.cerrojo()
            .reglas
            .get(regla)
            .is_some_and(|c| c.desactivada)
    }

    /// Registra una llamada nueva. Si pide `raw` o repite la huella de la última salida filtrada,
    /// cuenta una re-petición de esa regla. Devuelve las reglas que se acaban de desactivar.
    pub fn observa_llamada(&self, huella: u64, raw: bool) -> Vec<&'static str> {
        let mut estado = self.cerrojo();
        let Some(previa) = estado.ultima.take() else {
            return Vec::new();
        };
        if !raw && previa.huella != huella {
            return Vec::new();
        }
        let contadores = estado.reglas.entry(previa.regla).or_default();
        contadores.repeticiones += 1;
        evalua(contadores, previa.regla).into_iter().collect()
    }

    /// Registra un uso filtrado de `regla` con los bytes del original y de lo enviado. Devuelve las
    /// reglas que se acaban de desactivar.
    pub fn registra_uso(
        &self,
        regla: &'static str,
        huella: u64,
        original_bytes: usize,
        sent_bytes: usize,
    ) -> Vec<&'static str> {
        tracing::info!(
            target: "output_filter",
            regla,
            original_bytes,
            sent_bytes,
            "salida filtrada"
        );
        let mut estado = self.cerrojo();
        estado.ultima = Some(Ultima { regla, huella });
        let contadores = estado.reglas.entry(regla).or_default();
        contadores.usos += 1;
        contadores.bytes_originales += original_bytes as u64;
        contadores.bytes_enviados += sent_bytes as u64;
        evalua(contadores, regla).into_iter().collect()
    }

    /// Copia de los contadores de `regla` (ceros si aún no ha habido usos).
    pub fn contadores(&self, regla: &str) -> Contadores {
        self.cerrojo()
            .reglas
            .get(regla)
            .copied()
            .unwrap_or_default()
    }

    /// Copia de los contadores de todas las reglas con algún uso o re-petición.
    pub fn resumen(&self) -> Vec<(&'static str, Contadores)> {
        self.cerrojo()
            .reglas
            .iter()
            .map(|(regla, c)| (*regla, *c))
            .collect()
    }
}

/// Decide si la regla pasa el umbral. Lo marca como desactivada en cuanto lo cumple, pero solo
/// devuelve la regla una vez: el aviso (`RuntimeNotice`) no se repite aunque sigan las re-peticiones.
fn evalua(contadores: &mut Contadores, regla: &'static str) -> Option<&'static str> {
    let cumple = contadores.usos >= USOS_MINIMOS
        && contadores.repeticiones * 100 > contadores.usos * UMBRAL_REPETICION_PCT;
    if !cumple {
        return None;
    }
    contadores.desactivada = true;
    if contadores.avisada {
        return None;
    }
    contadores.avisada = true;
    Some(regla)
}

/// Texto del aviso (`RuntimeNotice`) que se publica una sola vez al desactivarse una regla.
pub fn aviso_desactivacion(regla: &str) -> String {
    format!(
        "El filtro de salida «{regla}» se ha desactivado en este proceso: el modelo ha vuelto a pedir la salida sin filtrar en más del {UMBRAL_REPETICION_PCT} % de sus usos."
    )
}

/// Estado del proceso: la desactivación de una regla vale para todo el proceso, como pide la spec.
#[cfg(not(test))]
pub fn global() -> &'static FilterStats {
    static GLOBAL: OnceLock<FilterStats> = OnceLock::new();
    GLOBAL.get_or_init(FilterStats::default)
}

/// En las pruebas cada hilo (es decir, cada prueba) tiene su propio estado: si no, repetir un mismo
/// comando en varias pruebas desactivaría una regla para todas las demás y el resultado dependería
/// del orden de ejecución.
#[cfg(test)]
pub fn global() -> &'static FilterStats {
    thread_local! {
        static LOCAL: &'static FilterStats = Box::leak(Box::default());
    }
    LOCAL.with(|estado| *estado)
}
