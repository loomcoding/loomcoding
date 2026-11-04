//! Tests: generic.

use super::support::*;
use super::*;

#[test]
fn generic_atom_matches_the_current_tool_lines() {
    let facts = ToolFacts::default();
    for item in sample_items() {
        assert_eq!(
            tool_lines_with(&item, &facts),
            super::super::tool_lines(&item),
            "distinto para la llamada {} ({})",
            item.call_id,
            item.name
        );
    }
}

#[test]
fn generic_atom_ignores_facts() {
    // Con hechos presentes la pieza genérica sigue pintando lo mismo: el
    // aspecto de las herramientas sin pieza propia no cambia.
    // Se llama a la genérica directamente: los ítems de ejemplo se llaman
    // `bash`, que ya tiene pieza propia, y lo que se prueba es la genérica.
    let facts = ToolFacts {
        primary: Some("ls -la".to_string()),
        count: Some(3),
        ..ToolFacts::default()
    };
    for item in sample_items() {
        assert_eq!(
            GENERIC.lines(&item, &facts),
            super::super::tool_lines(&item)
        );
    }
}

#[test]
fn unknown_tool_falls_back_to_generic() {
    let mut item = ToolItem::new("c1", "mcp__github__create_issue");
    item.set_summary("{\"title\":\"x\"}");
    item.set_result(false);
    item.set_output("created");
    let lines = atom_for(&item.name).lines(&item, &ToolFacts::default());
    assert_eq!(lines, super::super::tool_lines(&item));
    assert_eq!(lines[0].kind, LineKind::ToolHead(ToolStatus::Success));
}

#[test]
fn facts_default_is_empty() {
    let facts = ToolFacts::default();
    assert_eq!(facts.primary, None);
    assert_eq!(facts.secondary, None);
    assert_eq!(facts.range, None);
    assert_eq!(facts.before, None);
    assert_eq!(facts.after, None);
    assert_eq!(facts.count, None);
    assert!(facts.items.is_empty());
}

#[test]
fn missing_primary_falls_back_to_generic() {
    let mut item = ToolItem::new("c1", "bash");
    item.set_summary("{\"command\":1}");
    item.set_output("a\nb\nc");
    let blank = ToolFacts {
        primary: Some("  \n ".to_string()),
        ..ToolFacts::default()
    };
    for facts in [ToolFacts::default(), blank] {
        assert_eq!(
            tool_lines_with(&item, &facts),
            super::super::tool_lines(&item)
        );
    }
}

#[test]
fn unknown_shape_falls_back() {
    let blank = ToolFacts {
        primary: Some(" \n ".to_string()),
        ..ToolFacts::default()
    };
    let count_only = ToolFacts {
        count: Some(2),
        ..ToolFacts::default()
    };
    for name in [
        "ask_user",
        "subagent",
        "subagent_control",
        "job_start",
        "job_output",
        "job_kill",
    ] {
        let mut item = ToolItem::new("c1", name);
        item.set_summary("{\"raro\":true}");
        item.set_result(false);
        item.set_output("salida");
        for facts in [ToolFacts::default(), blank.clone(), count_only.clone()] {
            assert_eq!(
                tool_lines_with(&item, &facts),
                super::super::tool_lines(&item),
                "{name}"
            );
        }
    }
}
