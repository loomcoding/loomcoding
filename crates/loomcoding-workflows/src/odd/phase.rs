//! Fase de ODD inferida de las herramientas. Función pura: mira el nombre de la herramienta y sus
//! argumentos, sin llamar al modelo.
//!
//! Lecturas, exploración; escrituras, implementación; `bash` con un comprobador conocido, comprobación;
//! la pregunta a la persona, decisión; `odd_track`, planificación. Un `bash` ambiguo no cambia la fase.

use serde_json::Value;

/// Fase de ODD que la interfaz muestra. `Idle` es el estado antes de la primera herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OddPhase {
    Exploring,
    Implementing,
    Checking,
    Deciding,
    Planning,
    Idle,
}

impl OddPhase {
    /// Nombre de la fase en la etiqueta de la interfaz.
    pub fn as_str(&self) -> &'static str {
        match self {
            OddPhase::Exploring => "exploring",
            OddPhase::Implementing => "implementing",
            OddPhase::Checking => "checking",
            OddPhase::Deciding => "deciding",
            OddPhase::Planning => "planning",
            OddPhase::Idle => "idle",
        }
    }
}

/// Órdenes de `bash` que comprueban algo: tests, compilación, lint o formato.
const COMPROBADORES: [&str; 9] = [
    "cargo test",
    "cargo build",
    "cargo check",
    "cargo clippy",
    "cargo fmt",
    "npm test",
    "pytest",
    "go test",
    "make test",
];

/// Fase que corresponde a una llamada de herramienta, o `None` si no cambia la fase.
pub fn phase_of(tool: &str, args: &Value) -> Option<OddPhase> {
    match tool {
        "view" | "grep" | "glob" | "ls" => Some(OddPhase::Exploring),
        "edit" | "write" | "multiedit" => Some(OddPhase::Implementing),
        "bash" => args["command"]
            .as_str()
            .filter(|orden| es_comprobacion(orden))
            .map(|_| OddPhase::Checking),
        "ask_user" => Some(OddPhase::Deciding),
        "odd_track" => Some(OddPhase::Planning),
        _ => None,
    }
}

fn es_comprobacion(orden: &str) -> bool {
    COMPROBADORES
        .iter()
        .any(|comprobador| orden.contains(comprobador))
}
