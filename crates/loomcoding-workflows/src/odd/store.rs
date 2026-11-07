//! Disco de los documentos de feature: validación del nombre, ruta y escritura atómica.

use super::doc::MAX_DOC_BYTES;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Carpeta, relativa al proyecto, donde viven los documentos de feature.
pub const DIR_RELATIVO: &str = ".loomcoding/odd";
/// Longitud máxima del nombre de una feature, en caracteres.
pub const MAX_NOMBRE_FEATURE: usize = 64;

/// Comprueba el nombre de la feature. No lo transforma: un nombre inválido se rechaza.
///
/// Solo se admiten `a-z`, `0-9`, `_` y `-`, de 1 a 64 caracteres. Así no hay rutas, ni `..`, ni
/// mayúsculas que dependan del sistema de ficheros.
pub fn validar_feature(feature: &str) -> Result<(), String> {
    let valido = (1..=MAX_NOMBRE_FEATURE).contains(&feature.chars().count())
        && feature
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-'));
    if valido {
        Ok(())
    } else {
        Err(format!(
            "el nombre de feature `{}` no es válido: use de 1 a {MAX_NOMBRE_FEATURE} caracteres, \
             solo `a-z`, `0-9`, `_` y `-` (sin rutas, mayúsculas ni `..`)",
            limitar_para_mensaje(feature)
        ))
    }
}

/// Ruta del documento de una feature dentro del proyecto. Exige un nombre ya validado.
pub fn ruta(cwd: &Path, feature: &str) -> PathBuf {
    cwd.join(DIR_RELATIVO).join(format!("{feature}.md"))
}

/// Lee el documento. `Ok(None)` si no existe; rechaza sin leer si supera el tamaño máximo.
pub fn leer(ruta: &Path) -> Result<Option<String>, String> {
    let metadatos = match std::fs::metadata(ruta) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("no pude leer el documento: {e}")),
    };
    // Se comprueba el tamaño antes de leer: un fichero enorme no entra en memoria.
    let tamano = metadatos.len();
    if tamano > MAX_DOC_BYTES as u64 {
        return Err(format!(
            "el documento ocupa {tamano} bytes; el máximo es {MAX_DOC_BYTES}. No lo leo ni lo toco"
        ));
    }
    std::fs::read_to_string(ruta)
        .map(Some)
        .map_err(|e| format!("no pude leer el documento como texto UTF-8: {e}"))
}

/// Escribe el contenido con un fichero temporal en el mismo directorio y un `rename`.
///
/// Así un corte a mitad no deja un documento a medias: o queda el anterior o el nuevo.
pub fn escribir_atomico(ruta: &Path, contenido: &str) -> Result<(), String> {
    let directorio = ruta
        .parent()
        .ok_or_else(|| "la ruta del documento no tiene carpeta".to_string())?;
    std::fs::create_dir_all(directorio)
        .map_err(|e| format!("no pude crear la carpeta del documento: {e}"))?;
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporal = directorio.join(format!(".{nombre}.{}.tmp", std::process::id()));
    let resultado = escribir_temporal(&temporal, contenido)
        .and_then(|()| std::fs::rename(&temporal, ruta).map_err(|e| e.to_string()));
    if let Err(error) = resultado {
        // Limpieza best-effort: si el temporal ya no existe, no pasa nada.
        let _ = std::fs::remove_file(&temporal);
        return Err(format!("no pude guardar el documento: {error}"));
    }
    Ok(())
}

fn escribir_temporal(temporal: &Path, contenido: &str) -> Result<(), String> {
    let mut fichero = std::fs::File::create(temporal).map_err(|e| e.to_string())?;
    fichero
        .write_all(contenido.as_bytes())
        .and_then(|()| fichero.sync_all())
        .map_err(|e| e.to_string())
}

/// Recorta un texto para mostrarlo en un mensaje de error, sin desbordarlo.
fn limitar_para_mensaje(texto: &str) -> String {
    const MAX: usize = 80;
    if texto.chars().count() <= MAX {
        texto.to_string()
    } else {
        let corto: String = texto.chars().take(MAX).collect();
        format!("{corto}…")
    }
}
