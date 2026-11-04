//! Átomo de edición: cabecera con recuentos y cuerpo en forma de diff acotado.

use super::*;

/// Cabecera de `edit`: la ruta y, tras dos espacios, las líneas añadidas y
/// borradas. La ruta se trunca dejando sitio a ese recuento.
pub(super) fn edit_head(item: &ToolItem, path: &str, diff: &Diff) -> Line {
    let (added, removed) = (diff.added(), diff.removed());
    let stats = format!("+{added} -{removed}");
    // Se reservan los dos espacios separadores y el recuento.
    let reserved = stats.graphemes(true).count() + 2;
    let path = truncate_path(path, SUMMARY_GRAPHEMES - reserved);
    let mut head = head_line(item, &[(Part::Path, path)]);
    head.text.push_str("  ");
    push_text(&mut head, &format!("+{added}"), Part::Added);
    head.text.push(' ');
    push_text(&mut head, &format!("-{removed}"), Part::Removed);
    head
}

/// Token de una línea de diff según su tipo.
pub(super) fn diff_token(kind: DiffKind) -> StyleToken {
    match kind {
        DiffKind::Insert => StyleToken::DiffInsert,
        DiffKind::Delete => StyleToken::DiffDelete,
        DiffKind::Hunk => StyleToken::DiffHunk,
        DiffKind::Context => StyleToken::Muted,
    }
}

/// Línea de diff con su marca delante del texto: `+` o `-` en los cambios y un
/// espacio en el contexto. La cabecera de hunk va sin marca.
pub(super) fn diff_line(line: &DiffLine) -> Line {
    let text = match line.kind {
        DiffKind::Hunk => line.text.clone(),
        DiffKind::Context => format!("  {}", line.text),
        DiffKind::Insert => format!("+ {}", line.text),
        DiffKind::Delete => format!("- {}", line.text),
    };
    let len = text.len();
    let mut out = Line::new(LineKind::ToolDiff(line.kind), text);
    out.spans.push(SpanSpec {
        start: 0,
        len,
        token: diff_token(line.kind),
    });
    out
}

/// Líneas del diff, acotadas a `DIFF_LINES` salvo que la llamada esté expandida.
pub(super) fn diff_body(item: &ToolItem, diff: &Diff) -> Vec<Line> {
    let all = diff.lines();
    let shown = if item.expanded {
        all.len()
    } else {
        all.len().min(DIFF_LINES)
    };
    let mut lines: Vec<Line> = all[..shown].iter().map(diff_line).collect();
    if shown < all.len() {
        lines.push(more_line(all.len() - shown));
    }
    lines
}

/// Pieza de `edit`: la ruta con las líneas añadidas y borradas, y el diff como
/// cuerpo. Con error, la salida del error va tras el diff. Sin texto previo y
/// nuevo, o sin cambios, degrada a la genérica.
pub(super) struct EditAtom;

impl ToolAtom for EditAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let (Some(path), Some(before), Some(after)) = (
            facts.primary.as_deref().and_then(first_line),
            facts.before.as_deref(),
            facts.after.as_deref(),
        ) else {
            return tool_lines(item);
        };
        let diff = Diff::new(before, after, DIFF_CONTEXT);
        if diff.is_empty() {
            return tool_lines(item);
        }
        let mut lines = vec![edit_head(item, path, &diff)];
        lines.extend(diff_body(item, &diff));
        if item.status() == ToolStatus::Error {
            lines.extend(body_lines(item));
        }
        lines
    }
}
