//! Piezas atómicas por herramienta: cómo se representa una llamada (`ToolItem`)
//! en el transcript a partir de sus hechos tipados (`ToolFacts`).
//!
//! Este módulo no conoce el JSON de entrada: los hechos los extrae el adaptador
//! que vive fuera de `batten` (`codeloom-tui`), de modo que la pieza solo pinta
//! hechos y estado.

use super::path_text::{split_path, truncate_path};
use super::tool_lines::{body_lines, more_line, truncate_graphemes, SUMMARY_GRAPHEMES};
use super::{
    tool_lines, Diff, DiffKind, DiffLine, Line, LineKind, SpanSpec, StyleToken, ToolItem,
    ToolStatus,
};
use unicode_segmentation::UnicodeSegmentation;

mod atoms;
mod diff;
mod text;

use atoms::*;
use diff::*;
use text::*;

#[cfg(test)]
mod tests;

/// Hechos tipados de una llamada a herramienta, ya extraídos de su entrada.
///
/// Cada campo es opcional: una pieza muestra lo que el adaptador haya podido
/// extraer y degrada con elegancia cuando falte algo. La interpretación exacta
/// de cada campo depende de la herramienta que lo rellena.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolFacts {
    /// Ruta, comando o patrón principal de la llamada.
    pub primary: Option<String>,
    /// Segundo dato corto (p. ej. el filtro `glob` de `grep`).
    pub secondary: Option<String>,
    /// Líneas de inicio y fin (`view`), ambas incluidas.
    pub range: Option<(usize, usize)>,
    /// Texto previo (`edit`).
    pub before: Option<String>,
    /// Texto nuevo (`edit`).
    pub after: Option<String>,
    /// Recuento (coincidencias, tareas…).
    pub count: Option<usize>,
    /// Resto de pares clave-valor que merece la pena mostrar.
    pub items: Vec<(String, String)>,
}

/// Pieza de render de una herramienta: convierte la llamada y sus hechos en
/// líneas del transcript. Las piezas no parsean JSON ni conocen el protocolo.
pub trait ToolAtom: Sync {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line>;
}

/// Pieza de reserva para las herramientas sin pieza propia (y las MCP). Reproduce
/// `tool_lines` exactamente, ignorando los hechos.
struct GenericAtom;

impl ToolAtom for GenericAtom {
    fn lines(&self, item: &ToolItem, _facts: &ToolFacts) -> Vec<Line> {
        tool_lines(item)
    }
}

static GENERIC: GenericAtom = GenericAtom;

/// Partes de un resumen de cabecera. Cada una lleva su token de estilo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Name,
    Command,
    /// Texto libre corto: pregunta, etiqueta de hijo o identificador de job.
    Text,
    Path,
    /// Directorio de una ruta de fichero: lo que antecede al nombre, con su separador final.
    Dir,
    Pattern,
    /// Dato secundario y corto: rango de líneas, filtro `glob`.
    Detail,
    Count,
    /// Líneas añadidas de un `edit` (`+N`).
    Added,
    /// Líneas borradas de un `edit` (`-N`).
    Removed,
}

/// Único punto donde se elige el estilo de cada parte del resumen. Ruta y patrón
/// usan `ToolPath`, el comando `ToolCommand` y los recuentos `ToolCount`; los
/// valores por defecto de esos tokens coinciden con los estilos previos
/// (`accent`, `default` y `dim`), salvo en los presets que los redefinen.
fn style_of(part: Part) -> StyleToken {
    match part {
        Part::Name => StyleToken::ToolName,
        Part::Command => StyleToken::ToolCommand,
        Part::Text => StyleToken::Default,
        Part::Path | Part::Pattern => StyleToken::ToolPath,
        Part::Dir | Part::Detail => StyleToken::Muted,
        Part::Count => StyleToken::ToolCount,
        Part::Added => StyleToken::DiffInsert,
        Part::Removed => StyleToken::DiffDelete,
    }
}

/// Pieza registrada para `name`. Las herramientas sin pieza propia (y las MCP)
/// usan la genérica.
pub fn atom_for(name: &str) -> &'static dyn ToolAtom {
    match name {
        "bash" | "job_start" => &SHELL,
        "view" => &FILE,
        "edit" => &EDIT,
        "grep" => &GREP,
        "glob" => &GLOB,
        "ask_user" => &ASK,
        "subagent" | "job_output" | "job_kill" => &REFERENCE,
        "subagent_control" => &CONTROL,
        "subagent_list" | "job_list" => &LIST,
        _ => &GENERIC,
    }
}

/// Líneas de la llamada según su pieza y sus hechos.
pub fn tool_lines_with(item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
    atom_for(&item.name).lines(item, facts)
}
