//! Preferencia del carril del transcript (`rail = "tree"|"dots"|"bar"|"box"|"tint"|"none"`), que
//! la persona usuaria elige en `/ui carril`. Vive aparte de `prefs.rs` para que ese fichero no
//! crezca; reutiliza el mismo lector y escritor de líneas `clave = valor`.

use std::io;
use std::path::Path;

use crate::render::rail::RailStyle;

use super::{load_value, save_value};

const RAIL_KEY: &str = "rail";

/// Lee el carril guardado. Ausente da `Ok(None)` y se queda el de por defecto. Un valor que no se
/// reconoce da `Err` con el texto tal como estaba escrito, para que el llamador avise.
pub fn load_rail(path: &Path) -> Result<Option<RailStyle>, String> {
    match load_value(path, RAIL_KEY, |value| {
        let sin_comillas = value
            .strip_prefix('"')
            .and_then(|resto| resto.strip_suffix('"'))
            .unwrap_or(value);
        Some((sin_comillas.to_string(), RailStyle::from_name(sin_comillas)))
    }) {
        None => Ok(None),
        Some((_, Some(style))) => Ok(Some(style)),
        Some((crudo, None)) => Err(crudo),
    }
}

/// Guarda el carril, conservando el resto de líneas del fichero.
pub fn save_rail(path: &Path, style: RailStyle) -> io::Result<()> {
    save_value(path, RAIL_KEY, &format!("\"{}\"", style.name()))
}
