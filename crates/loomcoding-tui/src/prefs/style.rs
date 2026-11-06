//! Preferencia de estilo (`style = "classic"|"flat"`): el paquete de composer, herramientas y
//! chrome que el usuario elige en `/ui`. Vive aparte de `prefs.rs` para que ese fichero no crezca.

use std::io;
use std::path::Path;

use loomcoding_cloth::widgets::StyleBundle;

use super::{load_value, save_value};

const STYLE_KEY: &str = "style";

/// Lee el paquete de estilo guardado (`style = "classic"` o `"flat"`). Cualquier otro valor,
/// o la ausencia de la clave, produce `None` y se queda el del tema.
pub fn load_style(path: &Path) -> Option<StyleBundle> {
    load_value(path, STYLE_KEY, |value| {
        StyleBundle::from_name(value.strip_prefix('"')?.strip_suffix('"')?)
    })
}

/// Guarda el paquete de estilo, conservando el resto de líneas del fichero.
pub fn save_style(path: &Path, bundle: StyleBundle) -> io::Result<()> {
    save_value(path, STYLE_KEY, &format!("\"{}\"", bundle.name()))
}
