//! Carril del transcript en la aplicación: fijarlo, persistirlo y leerlo de `ui.toml`. Vive aparte
//! de `prefs.rs` para que ese fichero no crezca.

use super::*;
use crate::render::rail::RailStyle;

impl App {
    /// Fija el carril (`/ui carril` o la fila «Carril» del panel), lo persiste si hay `prefs_path`
    /// y avisa en `slash_feedback` si el guardado falla, sin impedir el cambio. No toca el texto
    /// del transcript: solo cambia el carril que lo acompaña y, con `box`, la caja del usuario.
    pub fn set_rail(&mut self, style: RailStyle) {
        self.rail_style = style;
        self.fija_caja(style == RailStyle::Box);
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_rail(path, style) {
                self.slash_feedback = Some(format!("Rail not saved: {err}"));
            }
        }
    }

    /// Lee el carril guardado en `ui.toml`. Si falta, se queda el de por defecto; si el valor no se
    /// reconoce, vuelve a `tree` y avisa en `slash_feedback` con las opciones válidas.
    pub(super) fn load_rail_pref(&mut self, path: &std::path::Path) {
        match crate::prefs::load_rail(path) {
            Ok(Some(style)) => self.rail_style = style,
            Ok(None) => {}
            Err(crudo) => {
                self.rail_style = RailStyle::Tree;
                let opciones: Vec<&str> = RailStyle::ALL.iter().map(|style| style.name()).collect();
                self.slash_feedback = Some(format!(
                    "Unknown rail in ui.toml: {crudo}. Options: {}",
                    opciones.join(", ")
                ));
            }
        }
        self.fija_caja(self.rail_style == RailStyle::Box);
    }

    /// Pone la caja del mensaje del usuario en el tema (`theme.rail_box`). Si cambia, invalida la
    /// caché de alturas: la caja añade una fila a cada mensaje del usuario sin cambiar su texto.
    /// `draw_transcript` lo vuelve a fijar en cada fotograma, porque con menos de 40 columnas el
    /// carril no existe y la caja tampoco.
    pub(crate) fn fija_caja(&mut self, activa: bool) {
        if self.theme.rail_box != activa {
            self.theme.rail_box = activa;
            self.layout.invalidate();
        }
    }
}
