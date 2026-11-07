//! Resumen genérico de un flujo de trabajo activo, publicado como `LiveEvent::FlowChanged`.
//!
//! El núcleo no sabe qué significa `kind` ni cómo se llega a cada `phase`: quien observa el flujo
//! construye el resumen y la interfaz pinta `label` tal cual. `id` vale 0 cuando el flujo no tiene
//! registro propio que lo identifique.

use serde::{Deserialize, Serialize};

/// Estado de un flujo en un instante, en la forma que la interfaz necesita para pintarlo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowSummary {
    pub kind: String,
    pub id: i64,
    pub phase: String,
    pub round: u8,
    pub max_rounds: u8,
    pub terminal: bool,
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::LiveEvent;

    #[test]
    fn flow_changed_hace_ida_y_vuelta_por_serde() {
        let evento = LiveEvent::FlowChanged {
            summary: FlowSummary {
                kind: "flujo-de-prueba".into(),
                id: 7,
                phase: "checking".into(),
                round: 2,
                max_rounds: 3,
                terminal: false,
                label: "prueba · checking".into(),
            },
            actor: "loom".into(),
            detail: "bitácora".into(),
        };
        let texto = serde_json::to_string(&evento).expect("el evento se serializa");
        let vuelta: LiveEvent = serde_json::from_str(&texto).expect("el evento se deserializa");
        match vuelta {
            LiveEvent::FlowChanged {
                summary,
                actor,
                detail,
            } => {
                assert_eq!(summary.kind, "flujo-de-prueba");
                assert_eq!(summary.id, 7);
                assert_eq!(summary.phase, "checking");
                assert_eq!(summary.round, 2);
                assert_eq!(summary.max_rounds, 3);
                assert!(!summary.terminal);
                assert_eq!(summary.label, "prueba · checking");
                assert_eq!(actor, "loom");
                assert_eq!(detail, "bitácora");
            }
            otro => panic!("debía volver FlowChanged y volvió {otro:?}"),
        }
    }
}
