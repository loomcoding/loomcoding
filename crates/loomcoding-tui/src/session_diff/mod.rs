//! Diferencias de los ficheros tocados por la sesión, obtenidas de `git diff`.
//!
//! El módulo se divide en tres partes: los tipos de dominio (este fichero), el
//! analizador puro de la salida de `git diff` (`parse`) y la obtención acotada
//! de esa salida, con plazo y cota de bytes (`fetch`).

mod fetch;
mod parse;

pub use fetch::fetch_session_diff;
pub use parse::parse_git_diff;

/// Una línea de un hunk, sin el prefijo `' '`, `'+'` o `'-'`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

/// Un bloque de cambios. `header` es la línea `@@ -a,b +c,d @@ ...` completa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Naturaleza del cambio sobre un fichero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Modified,
    Added,
    Deleted,
    Renamed { from: String },
    Binary,
}

/// Cambios de un fichero. `path` es la ruta nueva (la del lado `b/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub kind: FileKind,
    pub added: usize,
    pub removed: usize,
    pub hunks: Vec<Hunk>,
}

/// Resultado de pedir el diff de la sesión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOutcome {
    Ok(Vec<FileDiff>),
    NoGit,
    TimedOut,
}
