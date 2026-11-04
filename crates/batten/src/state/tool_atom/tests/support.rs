//! Piezas comunes de los tests.

use super::*;

/// Llamadas representativas de cada estado que `tool_lines` distingue.
pub(super) fn sample_items() -> Vec<ToolItem> {
    let mut pending = ToolItem::new("c0", "bash");
    pending.set_summary("ls");

    let mut running = ToolItem::new("c1", "bash");
    running.set_summary("cargo test");
    running.start();

    let mut short = ToolItem::new("c2", "bash");
    short.set_summary("echo hi");
    short.set_output("a\nb");

    let mut folded = ToolItem::new("c3", "bash");
    folded.set_result(false);
    folded.set_output("1\n2\n3\n4\n5");

    let mut expanded = ToolItem::new("c4", "bash");
    expanded.set_result(true);
    expanded.set_output("1\n2\n3\n4");
    expanded.toggle_expanded();

    let mut failed = ToolItem::new("c5", "edit");
    failed.set_summary("src/lib.rs");
    failed.set_result(true);
    failed.set_output("old text not found");

    let mut waiting = ToolItem::new("c6", "bash");
    waiting.set_summary("rm -rf build");
    waiting.start();
    waiting.await_permission();

    let mut canceled = ToolItem::new("c7", "view");
    canceled.set_summary("src/main.rs");
    canceled.start();
    canceled.cancel();

    vec![
        pending, running, short, folded, expanded, failed, waiting, canceled,
    ]
}

/// Hechos con solo el `primary` dado.
pub(super) fn primary(text: &str) -> ToolFacts {
    ToolFacts {
        primary: Some(text.to_string()),
        ..ToolFacts::default()
    }
}

/// Resumen de la cabecera tras el nombre de la herramienta `name`.
pub(super) fn summary_of(lines: &[Line], name: &str) -> String {
    let head = lines[0].plain();
    head.strip_prefix(name)
        .and_then(|rest| rest.strip_prefix("  "))
        .unwrap_or_default()
        .to_string()
}

/// Hechos de una edición de `path` de `before` a `after`.
pub(super) fn edit_facts(path: &str, before: &str, after: &str) -> ToolFacts {
    ToolFacts {
        primary: Some(path.to_string()),
        before: Some(before.to_string()),
        after: Some(after.to_string()),
        ..ToolFacts::default()
    }
}

/// Llamada `edit` terminada con éxito.
pub(super) fn edit_item() -> ToolItem {
    let mut item = ToolItem::new("c1", "edit");
    item.set_result(false);
    item.set_output("edit applied");
    item
}

pub(super) fn diff_count(lines: &[Line]) -> usize {
    lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::ToolDiff(_)))
        .count()
}

/// Hechos de `ask_user`: la primera pregunta y el número de opciones.
pub(super) fn ask_facts(question: &str, options: usize) -> ToolFacts {
    ToolFacts {
        count: Some(options),
        ..primary(question)
    }
}

/// Token del span de `line` que cubre exactamente `text`, si lo hay.
pub(super) fn token_of(line: &Line, text: &str) -> Option<StyleToken> {
    line.spans
        .iter()
        .find(|span| line.text.get(span.start..span.start + span.len) == Some(text))
        .map(|span| span.token)
}
