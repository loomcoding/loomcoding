//! Paquete de contexto acotado: lo que un modelo pequeño necesita saber de un símbolo en una sola respuesta.
//!
//! Cabe señalar que todo lleva cota (decálogo de contexto): caracteres totales con techo duro, número
//! de símbolos, líneas de cuerpo y usos por símbolo. Si algo no cabe, se dice.

use super::index::{Symbol, SymbolIndex, SymbolKind};
use std::path::Path;

/// Techo duro de `max_chars`, pida lo que pida quien llama.
pub const MAX_CHARS_HARD: usize = 20_000;

/// Suelo de `max_chars`: por debajo de esto ni la cabecera cabe.
const MIN_CHARS: usize = 400;

/// Usos y pruebas que se listan por símbolo.
const MAX_USES: usize = 5;
const MAX_TESTS: usize = 3;

/// Caracteres máximos de una línea de cuerpo y de la sección del mapa.
const MAX_LINE_CHARS: usize = 160;
const MAX_MAP_CHARS: usize = 1500;

/// Nombres parecidos que se proponen cuando no hay coincidencia exacta.
const MAX_SUGGESTIONS: usize = 8;

/// Opciones del paquete.
#[derive(Debug, Clone)]
pub struct PackOptions {
    pub max_chars: usize,
    pub max_symbols: usize,
    pub body_lines: usize,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            max_chars: 6000,
            max_symbols: 12,
            body_lines: 20,
        }
    }
}

fn kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "fn",
        SymbolKind::Method => "method",
        SymbolKind::Type => "type",
        SymbolKind::Trait => "trait",
        SymbolKind::Module => "mod",
        SymbolKind::Macro => "macro",
    }
}

fn is_test_path(path: &Path) -> bool {
    path.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        name == "tests" || name.ends_with("_tests.rs") || name.starts_with("test_")
    })
}

/// Ruta con barras normales, igual en todas las plataformas.
fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn cut_line(line: &str) -> String {
    let line = line.trim_end();
    if line.chars().count() <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let mut cut: String = line.chars().take(MAX_LINE_CHARS).collect();
    cut.push('…');
    cut
}

/// Símbolos que responden a la consulta: coincidencia exacta o, si no la hay, por contener el texto.
fn matching<'a>(index: &'a SymbolIndex, query: &str) -> (Vec<&'a Symbol>, bool) {
    // `Tipo::metodo` se busca por su último segmento.
    let name = query.rsplit("::").next().unwrap_or(query).trim();
    let exact: Vec<&Symbol> = index.definitions(name).iter().collect();
    if !exact.is_empty() {
        return (exact, true);
    }
    let needle = name.to_lowercase();
    let mut names: Vec<&str> = index
        .names()
        .filter(|n| n.to_lowercase().contains(&needle))
        .collect();
    names.sort_unstable();
    let similar = names
        .into_iter()
        .flat_map(|n| index.definitions(n).iter())
        .collect();
    (similar, false)
}

/// Sección del mapa del repositorio (`## Quiero…`) que nombra el símbolo, si existe.
fn code_map_section(root: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(root.join("docs/CODE-MAP.md")).ok()?;
    let mut sections: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            sections.push(String::new());
        }
        if let Some(last) = sections.last_mut() {
            last.push_str(line);
            last.push('\n');
        }
    }
    let section = sections
        .into_iter()
        .find(|s| s.starts_with("## Quiero") && s.contains(name))?;
    Some(section.chars().take(MAX_MAP_CHARS).collect())
}

fn symbol_block(index: &SymbolIndex, root: &Path, sym: &Symbol, body_lines: usize) -> String {
    let mut out = format!(
        "## {} ({}) {}:{}\n{}\n",
        sym.name,
        kind_label(sym.kind),
        shown(&sym.path),
        sym.line,
        sym.signature
    );
    if let Ok(text) = std::fs::read_to_string(root.join(&sym.path)) {
        for line in text
            .lines()
            .skip(sym.line.saturating_sub(1) as usize)
            .take(body_lines)
        {
            out.push_str("    ");
            out.push_str(&cut_line(line));
            out.push('\n');
        }
    }
    let refs = index.references(&sym.name);
    let (tests, uses): (Vec<_>, Vec<_>) = refs.iter().partition(|r| is_test_path(&r.path));
    if !uses.is_empty() {
        let shown_uses: Vec<String> = uses
            .iter()
            .take(MAX_USES)
            .map(|r| format!("{}:{}", shown(&r.path), r.line))
            .collect();
        out.push_str(&format!("usos: {}", shown_uses.join(", ")));
        if uses.len() > MAX_USES {
            out.push_str(&format!(" (+{} más)", uses.len() - MAX_USES));
        }
        out.push('\n');
    }
    if !tests.is_empty() {
        let shown_tests: Vec<String> = tests
            .iter()
            .take(MAX_TESTS)
            .map(|r| format!("{}:{}", shown(&r.path), r.line))
            .collect();
        out.push_str(&format!("pruebas: {}", shown_tests.join(", ")));
        if tests.len() > MAX_TESTS {
            out.push_str(&format!(" (+{} más)", tests.len() - MAX_TESTS));
        }
        out.push('\n');
    }
    out
}

/// Compone el paquete de contexto de `query` (nombre de símbolo, `Tipo::metodo` o texto parcial).
pub fn compose(index: &SymbolIndex, root: &Path, query: &str, opts: &PackOptions) -> String {
    let query = query.trim();
    if query.is_empty() {
        return "empty query: pass a symbol name such as `Tipo::metodo`".into();
    }
    let max_chars = opts.max_chars.clamp(MIN_CHARS, MAX_CHARS_HARD);
    let (mut found, exact) = matching(index, query);
    if found.is_empty() {
        return format!("no symbol matches `{query}`");
    }
    // Primero lo más usado; empates por nombre y ruta para que el orden sea estable.
    found.sort_by(|a, b| {
        index
            .references(&b.name)
            .len()
            .cmp(&index.references(&a.name).len())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });
    let mut out = String::new();
    if !exact {
        let names: Vec<&str> = found
            .iter()
            .map(|s| s.name.as_str())
            .take(MAX_SUGGESTIONS)
            .collect();
        out.push_str(&format!(
            "no symbol is named `{query}`; closest names: {}\n\n",
            names.join(", ")
        ));
    }
    // Se reserva sitio para el aviso de recorte y la sección del mapa.
    let map = found
        .first()
        .and_then(|s| code_map_section(root, &s.name))
        .unwrap_or_default();
    let footer_room = 90;
    let budget = max_chars.saturating_sub(footer_room + map.chars().count().min(max_chars / 3));
    let total = found.len().min(opts.max_symbols.max(1));
    let mut shown_count = 0usize;
    for sym in found.iter().take(total) {
        let block = symbol_block(index, root, sym, opts.body_lines);
        if out.chars().count() + block.chars().count() > budget {
            // El primer bloque siempre entra, recortado si hace falta.
            if shown_count == 0 {
                let room = budget.saturating_sub(out.chars().count());
                let mut cut: String = block.chars().take(room).collect();
                if let Some(pos) = cut.rfind('\n') {
                    cut.truncate(pos + 1);
                }
                out.push_str(&cut);
                shown_count += 1;
            }
            break;
        }
        out.push_str(&block);
        out.push('\n');
        shown_count += 1;
    }
    let omitted = found.len() - shown_count;
    if !map.is_empty() {
        let room = max_chars
            .saturating_sub(footer_room)
            .saturating_sub(out.chars().count());
        let mut section: String = map.chars().take(room).collect();
        if section.chars().count() < map.chars().count() {
            if let Some(pos) = section.rfind('\n') {
                section.truncate(pos + 1);
            }
        }
        out.push_str(&section);
    }
    if omitted > 0 {
        out.push_str(&format!(
            "[recortado: {omitted} símbolos más; afina la consulta]\n"
        ));
    }
    out
}
