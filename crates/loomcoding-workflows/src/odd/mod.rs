//! Desarrollo orgánico (ODD): documento de feature y su formato Markdown.
//!
//! Todo el módulo es puro: ni E/S de ficheros, ni eventos, ni herramientas. Lo que aquí vive
//! es la estructura del documento, sus cotas y su ida y vuelta con Markdown. Ver la spec
//! `docs/specs/2026-11-07-loomcoding-odd-desarrollo-organico.md`.

pub mod doc;
pub mod render;

pub use doc::{FeatureDoc, Task, TaskStatus};

/// Errores del documento de feature. El texto está en español, como el resto del núcleo.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OddError {
    #[error("el documento ocupa {size} bytes; el máximo es {max}")]
    TooLarge { size: usize, max: usize },
    #[error("el documento tiene {count} tareas; el máximo es {max}")]
    TooManyTasks { count: usize, max: usize },
    #[error("el identificador de tarea `{id}` aparece más de una vez")]
    DuplicateTask { id: String },
    #[error("el campo `{field}` de la tarea `{id}` ocupa {size} bytes; el máximo es {max}")]
    TaskFieldTooLong {
        id: String,
        field: &'static str,
        size: usize,
        max: usize,
    },
    #[error("falta la cabecera `# <feature>` en la primera línea no vacía")]
    MissingHeader,
    #[error("línea {line}: {reason}")]
    Malformed { line: usize, reason: &'static str },
}
