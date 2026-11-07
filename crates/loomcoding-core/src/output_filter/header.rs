//! Cabecera que anuncia cada salida filtrada (R1): nunca en silencio.

use std::path::Path;

/// Cabecera `[filtrado: N líneas omitidas de M — regla X; el original sigue en <ruta>]`.
pub fn cabecera(omitidas: usize, total: usize, regla: &str, original: Option<&Path>) -> String {
    match original {
        Some(ruta) => format!(
            "[filtrado: {omitidas} líneas omitidas de {total} — regla {regla}; el original sigue en {}]",
            ruta.display()
        ),
        None => format!(
            "[filtrado: {omitidas} líneas omitidas de {total} — regla {regla}; el original no se pudo conservar]"
        ),
    }
}
