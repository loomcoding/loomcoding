//! Flujos de trabajo de Loomcoding (ODD, y los que vengan después).
//!
//! Regla de dependencia: este crate depende de `loomcoding-core`; nunca al revés. El núcleo no
//! nombra ningún flujo.

use loomcoding_core::flow::FlowMode;
use loomcoding_core::tools::ToolRegistry;
use std::sync::Arc;

pub mod odd;

pub use odd::gate::OddGate;
pub use odd::medicion::OddStats;
pub use odd::observer::OddObserver;
pub use odd::prompt::OddPrompt;
pub use odd::state::OddState;

/// `kind` del modo de ODD en `FlowChanged` y en `[flows.<kind>]`.
pub const ODD_KIND: &str = "odd";
/// Etiqueta base de ODD en la cabecera.
pub const ODD_LABEL: &str = "ODD";

/// Modos de flujo que publican estos flujos, con su valor por defecto (`enabled`). La configuración
/// del proyecto lo sustituye; el núcleo y la interfaz no conocen la lista.
pub fn flow_modes() -> Vec<FlowMode> {
    vec![FlowMode {
        kind: ODD_KIND.into(),
        label: ODD_LABEL.into(),
        enabled: true,
    }]
}

/// Registra las herramientas de los flujos en `registry` y devuelve el estado compartido de ODD.
///
/// Se devuelve el [`OddState`] (un manejador barato de clonar) para que quien compone el registro
/// pueda entregárselo luego a otras piezas, como el cierre del turno de la tarea 3. Quien no lo
/// necesite puede descartarlo.
pub fn register(registry: &mut ToolRegistry) -> OddState {
    let estado = OddState::new();
    registry.register(Arc::new(odd::tool::OddTrackTool::new(estado.clone())));
    estado
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_modo_de_odd_viene_encendido_por_defecto() {
        let modos = flow_modes();
        assert_eq!(modos.len(), 1);
        assert_eq!(modos[0].kind, ODD_KIND);
        assert_eq!(modos[0].label, ODD_LABEL);
        assert!(modos[0].enabled);
    }
}
