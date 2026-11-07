//! Flujos de trabajo de Loomcoding (ODD, y los que vengan después).
//!
//! Regla de dependencia: este crate depende de `loomcoding-core`; nunca al revés. El núcleo no
//! nombra ningún flujo.

use loomcoding_core::tools::ToolRegistry;
use std::sync::Arc;

pub mod odd;

pub use odd::gate::OddGate;
pub use odd::observer::OddObserver;
pub use odd::prompt::OddPrompt;
pub use odd::state::OddState;

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
