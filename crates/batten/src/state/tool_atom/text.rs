//! Piezas de texto de los átomos: primera línea, cola acotada, cabeceras, rutas y recuentos.

use super::*;

/// Longitud máxima en grafemas de la cola de un resumen (`in {glob}`, rango).
pub(super) const TAIL_GRAPHEMES: usize = 40;

/// Líneas de contexto alrededor de cada cambio del diff de `edit`.
pub(super) const DIFF_CONTEXT: usize = 2;

/// Líneas de diff visibles mientras la llamada no está expandida.
pub(super) const DIFF_LINES: usize = 12;

/// Primera línea no vacía de `text`, sin espacios sobrantes.
pub(super) fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

/// Parte principal del resumen con su cola opcional. La cola se acota primero y
/// la parte principal se trunca para que el resumen completo quepa en
/// `SUMMARY_GRAPHEMES`, de modo que el rango o el filtro no desaparezcan.
pub(super) fn with_tail(main: &str, main_part: Part, tail: Option<String>) -> Vec<(Part, String)> {
    let tail = tail.map(|tail| truncate_graphemes(&tail, TAIL_GRAPHEMES));
    // El espacio separador cuenta como un grafema más.
    let reserved = tail.as_deref().map_or(0, |t| t.graphemes(true).count() + 1);
    let max = SUMMARY_GRAPHEMES - reserved;
    let main = if main_part == Part::Path {
        truncate_path(main, max)
    } else {
        truncate_graphemes(main, max)
    };
    let mut parts = vec![(main_part, main)];
    if let Some(tail) = tail {
        parts.push((Part::Detail, tail));
    }
    parts
}

/// Añade `text` al final de `line` con el token de `part`.
pub(super) fn push_text(line: &mut Line, text: &str, part: Part) {
    let start = line.text.len();
    line.text.push_str(text);
    line.spans.push(SpanSpec {
        start,
        len: text.len(),
        token: style_of(part),
    });
}

/// Cabecera con el nombre de la herramienta, su estado y las partes del resumen.
/// Mismo formato que la cabecera genérica: nombre, dos espacios y resumen.
pub(super) fn head_line(item: &ToolItem, parts: &[(Part, String)]) -> Line {
    let mut head = Line::new(LineKind::ToolHead(item.status()), item.name.as_str());
    if !item.name.is_empty() {
        head.spans.push(SpanSpec {
            start: 0,
            len: item.name.len(),
            token: style_of(Part::Name),
        });
    }
    for (index, (part, text)) in parts.iter().enumerate() {
        head.text.push_str(if index == 0 { "  " } else { " " });
        if *part == Part::Path {
            push_path(&mut head, text);
        } else {
            push_text(&mut head, text, *part);
        }
    }
    head
}

/// Añade una ruta de fichero a `line`: el directorio apagado y el nombre resaltado, de modo que
/// el fichero se vea de un vistazo. Una ruta sin directorio es solo el nombre.
pub(super) fn push_path(line: &mut Line, path: &str) {
    let (dir, name) = split_path(path);
    if !dir.is_empty() {
        push_text(line, dir, Part::Dir);
    }
    push_text(line, name, Part::Path);
}

/// Cabecera con `parts` seguida del cuerpo común de la herramienta.
pub(super) fn with_body(item: &ToolItem, parts: &[(Part, String)]) -> Vec<Line> {
    let mut lines = vec![head_line(item, parts)];
    lines.extend(body_lines(item));
    lines
}

/// Número de líneas no vacías de una salida.
pub(super) fn count_nonempty(output: &str) -> usize {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// Línea final de recuento, «N matches» o «N files», con el recuento en `Count`.
pub(super) fn count_line(count: usize, label: &str) -> Line {
    let text = format!("{count} {label}");
    let len = text.len();
    let mut line = Line::new(LineKind::ToolBody, text);
    line.spans.push(SpanSpec {
        start: 0,
        len,
        token: style_of(Part::Count),
    });
    line
}
