//! `/flow`: lista los modos de flujo y guarda su interruptor en `[flows.<kind>] enabled` del proyecto.
//!
//! Es de configuración. La TUI es un cliente separado del daemon y no toca el estado de los modos
//! en caliente, así que el cambio aplica al reiniciar Loom; por eso no hay canal nuevo.

use super::App;
use loomcoding_core::config::FlowsConfig;
use std::path::PathBuf;

impl App {
    /// `/flow` lista los modos; `/flow <kind> on|off` guarda `[flows.<kind>] enabled` en el `loomcoding.toml` del proyecto.
    pub fn flow_command(&mut self, arg: &str) -> Result<(), String> {
        let partes: Vec<&str> = arg.split_whitespace().collect();
        match partes.as_slice() {
            [] => {
                self.slash_feedback = Some(self.listado_de_modos());
                Ok(())
            }
            [kind, valor] if matches!(*valor, "on" | "off") => {
                let kind = *kind;
                let on = *valor == "on";
                if !self.flow_modes.iter().any(|modo| modo.kind == kind) {
                    return Err(self.uso());
                }
                let path = self.config_del_proyecto().ok_or_else(|| {
                    format!("flow: no hay proyecto donde guardar [flows.{kind}] enabled")
                })?;
                FlowsConfig::save_enabled(&path, kind, on)
                    .map_err(|error| format!("flow: no se pudo guardar: {error}"))?;
                self.slash_feedback = Some(format!(
                    "flow {kind}: {} en la config · aplica al reiniciar Loom",
                    estado(on)
                ));
                Ok(())
            }
            _ => Err(self.uso()),
        }
    }

    /// Estado de cada modo: lo que dice la config del proyecto si existe, o lo que se resolvió al arrancar.
    fn listado_de_modos(&self) -> String {
        if self.flow_modes.is_empty() {
            return "flow: no hay modos de flujo en esta sesión".to_string();
        }
        let config = self
            .config_del_proyecto()
            .and_then(|path| FlowsConfig::load(&path).ok());
        let estados: Vec<String> = self
            .flow_modes
            .iter()
            .map(|modo| {
                let activo = config.as_ref().map_or(modo.enabled, |config| {
                    config.enabled(&modo.kind, modo.enabled)
                });
                format!("{}: {}", modo.kind, estado(activo))
            })
            .collect();
        format!(
            "flow {} (config [flows.<kind>] enabled) · fase propia de Loom, no estado A2A",
            estados.join(", ")
        )
    }

    /// Uso de `/flow`, con los `kind` conocidos.
    fn uso(&self) -> String {
        if self.flow_modes.is_empty() {
            return "Usage: /flow [<kind> on|off]".to_string();
        }
        let kinds: Vec<&str> = self
            .flow_modes
            .iter()
            .map(|modo| modo.kind.as_str())
            .collect();
        format!("Usage: /flow [<kind> on|off] (kinds: {})", kinds.join(", "))
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
