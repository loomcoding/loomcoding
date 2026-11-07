//! `/odd`: informa del interruptor de ODD y lo guarda en `[odd] enabled` del proyecto.
//!
//! Es de configuración. La TUI es un cliente separado del daemon y no toca el `OddState` en
//! caliente, así que el cambio aplica al reiniciar Loom; por eso no hay canal nuevo.

use super::App;
use loomcoding_core::config::OddConfig;
use std::path::PathBuf;

impl App {
    /// `/odd` informa; `/odd on|off` guarda `[odd] enabled` en el `loomcoding.toml` del proyecto.
    pub fn odd_command(&mut self, arg: &str) -> Result<(), String> {
        match arg.trim() {
            "" => {
                let activo = match self.config_del_proyecto() {
                    Some(path) => OddConfig::load(&path)
                        .map(|config| config.enabled)
                        .unwrap_or(self.flows_odd_enabled),
                    None => self.flows_odd_enabled,
                };
                self.slash_feedback = Some(format!(
                    "ODD: {} (config [odd] enabled) · fase propia de Loom, no estado A2A",
                    estado(activo)
                ));
                Ok(())
            }
            valor @ ("on" | "off") => {
                let on = valor == "on";
                let path = self.config_del_proyecto().ok_or_else(|| {
                    "ODD: no hay proyecto donde guardar [odd] enabled".to_string()
                })?;
                OddConfig::save_enabled(&path, on)
                    .map_err(|error| format!("ODD: no se pudo guardar: {error}"))?;
                self.slash_feedback = Some(format!(
                    "ODD: {} en la config · aplica al reiniciar Loom",
                    estado(on)
                ));
                Ok(())
            }
            _ => Err("Usage: /odd [on|off]".to_string()),
        }
    }

    /// `loomcoding.toml` del directorio del proyecto, o `None` si la aplicación no tiene proyecto.
    fn config_del_proyecto(&self) -> Option<PathBuf> {
        (!self.cwd.is_empty()).then(|| PathBuf::from(&self.cwd).join("loomcoding.toml"))
    }
}

fn estado(activo: bool) -> &'static str {
    if activo {
        "activo"
    } else {
        "apagado"
    }
}
