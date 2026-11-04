//! Tests: edit.

use super::support::*;
use super::*;

#[test]
fn edit_summary_shows_path_and_counts() {
    let item = edit_item();
    let facts = edit_facts("src/lib.rs", "a\nb\nc\n", "a\nB\nc\nd\n");
    let lines = tool_lines_with(&item, &facts);
    assert_eq!(lines[0].plain(), "edit  src/lib.rs  +2 -1");
    assert_eq!(lines[0].kind, LineKind::ToolHead(ToolStatus::Success));
}

#[test]
fn edit_body_has_diff_lines_with_kinds() {
    let item = edit_item();
    let facts = edit_facts("a.txt", "a\nb\n", "a\nB\n");
    let lines = tool_lines_with(&item, &facts);
    let body: Vec<(LineKind, &str)> = lines[1..].iter().map(|l| (l.kind, l.plain())).collect();
    assert_eq!(
        body,
        vec![
            (LineKind::ToolDiff(DiffKind::Hunk), "@@ -1,2 +1,2 @@"),
            (LineKind::ToolDiff(DiffKind::Context), "  a"),
            (LineKind::ToolDiff(DiffKind::Delete), "- b"),
            (LineKind::ToolDiff(DiffKind::Insert), "+ B"),
        ]
    );
    assert_eq!(lines[3].spans[0].token, StyleToken::DiffDelete);
    assert_eq!(lines[4].spans[0].token, StyleToken::DiffInsert);
}

#[test]
fn edit_diff_is_capped_and_expandable() {
    let before: String = (0..30).map(|i| format!("viejo {i}\n")).collect();
    let after: String = (0..30).map(|i| format!("nuevo {i}\n")).collect();
    let total = Diff::new(&before, &after, DIFF_CONTEXT).lines().len();
    assert!(
        total > DIFF_LINES + 1,
        "precondición: el diff supera el tope"
    );
    let facts = edit_facts("a.txt", &before, &after);

    let mut item = edit_item();
    let lines = tool_lines_with(&item, &facts);
    assert_eq!(diff_count(&lines), DIFF_LINES);
    let more = lines.last().unwrap();
    assert_eq!(more.kind, LineKind::ToolMore);
    assert_eq!(
        more.plain(),
        format!("… ({} lines hidden) [Ctrl+O to expand]", total - DIFF_LINES)
    );

    item.toggle_expanded();
    let lines = tool_lines_with(&item, &facts);
    assert_eq!(diff_count(&lines), total);
    assert!(lines.iter().all(|l| l.kind != LineKind::ToolMore));
}

#[test]
fn edit_without_before_after_falls_back() {
    let mut item = edit_item();
    item.set_summary("src/lib.rs");
    let only_path = primary("src/lib.rs");
    let without_after = ToolFacts {
        before: Some("x".to_string()),
        ..primary("src/lib.rs")
    };
    for facts in [only_path, without_after] {
        assert_eq!(tool_lines_with(&item, &facts), tool_lines(&item));
    }
}

#[test]
fn empty_diff_falls_back() {
    let item = edit_item();
    let facts = edit_facts("a.txt", "igual\n", "igual\n");
    assert_eq!(tool_lines_with(&item, &facts), tool_lines(&item));
}

#[test]
fn edit_error_keeps_error_output_after_the_diff() {
    let mut item = ToolItem::new("c1", "edit");
    item.set_result(true);
    item.set_output("old text not found");
    let lines = tool_lines_with(&item, &edit_facts("a.txt", "a\n", "b\n"));
    assert_eq!(lines[0].kind, LineKind::ToolHead(ToolStatus::Error));
    let last = lines.last().unwrap();
    assert_eq!(last.kind, LineKind::ToolBody);
    assert_eq!(last.plain(), "old text not found");
    assert_eq!(diff_count(&lines[..lines.len() - 1]), 3);
}

#[test]
fn edit_running_shows_diff_from_input() {
    let mut item = ToolItem::new("c1", "edit");
    item.start();
    let lines = tool_lines_with(&item, &edit_facts("a.txt", "a\n", "b\n"));
    assert_eq!(lines[0].kind, LineKind::ToolHead(ToolStatus::Running));
    assert!(lines
        .iter()
        .any(|l| l.kind == LineKind::ToolDiff(DiffKind::Insert)));
}
