//! Tests: spans.

use super::support::*;
use super::*;

#[test]
fn bash_command_span_uses_tool_command_token() {
    let item = ToolItem::new("c1", "bash");
    let lines = tool_lines_with(&item, &primary("cargo test -p loomcoding-cloth"));
    assert_eq!(
        token_of(&lines[0], "cargo test -p loomcoding-cloth"),
        Some(StyleToken::ToolCommand)
    );
}

#[test]
fn view_path_span_uses_tool_path_token() {
    let item = ToolItem::new("c1", "view");
    let facts = ToolFacts {
        range: Some((10, 29)),
        ..primary("src/lib.rs")
    };
    let lines = tool_lines_with(&item, &facts);
    // El nombre del fichero se resalta y el directorio queda apagado.
    assert_eq!(token_of(&lines[0], "lib.rs"), Some(StyleToken::ToolPath));
    assert_eq!(token_of(&lines[0], "src/"), Some(StyleToken::Muted));
    // El rango es un dato secundario, no la ruta.
    assert_eq!(token_of(&lines[0], "L10-29"), Some(StyleToken::Muted));
}

#[test]
fn grep_count_span_uses_tool_count_token() {
    let mut item = ToolItem::new("c1", "grep");
    item.set_result(false);
    item.set_output("a\nb\nc");
    let lines = tool_lines_with(&item, &primary("TODO"));
    let last = lines.last().unwrap();
    assert_eq!(token_of(last, "3 matches"), Some(StyleToken::ToolCount));
    assert_eq!(token_of(&lines[0], "TODO"), Some(StyleToken::ToolPath));
}

#[test]
fn edit_path_span_uses_tool_path_token() {
    let item = edit_item();
    let lines = tool_lines_with(&item, &edit_facts("src/lib.rs", "a\n", "b\n"));
    assert_eq!(token_of(&lines[0], "lib.rs"), Some(StyleToken::ToolPath));
    assert_eq!(token_of(&lines[0], "src/"), Some(StyleToken::Muted));
}

#[test]
fn a_file_path_is_split_into_a_dim_directory_and_a_highlighted_name() {
    let item = ToolItem::new("c1", "view");
    let lines = tool_lines_with(&item, &primary("crates/loomcoding-tui/src/render.rs"));

    assert_eq!(
        lines[0].plain(),
        "view  crates/loomcoding-tui/src/render.rs"
    );
    assert_eq!(
        token_of(&lines[0], "crates/loomcoding-tui/src/"),
        Some(StyleToken::Muted)
    );
    assert_eq!(token_of(&lines[0], "render.rs"), Some(StyleToken::ToolPath));
}

#[test]
fn a_bare_file_name_is_all_highlighted_and_windows_separators_split_too() {
    let item = ToolItem::new("c1", "view");
    let lines = tool_lines_with(&item, &primary("README.md"));
    assert_eq!(token_of(&lines[0], "README.md"), Some(StyleToken::ToolPath));

    let lines = tool_lines_with(&item, &primary(r"src\main.rs"));
    assert_eq!(token_of(&lines[0], r"src\"), Some(StyleToken::Muted));
    assert_eq!(token_of(&lines[0], "main.rs"), Some(StyleToken::ToolPath));
}

#[test]
fn a_long_path_is_cut_on_the_left_so_the_file_name_survives() {
    let item = ToolItem::new("c1", "view");
    let largo = format!("{}/render.rs", "directorio-muy-largo/".repeat(8));
    let lines = tool_lines_with(&item, &primary(&largo));

    let cabecera = lines[0].plain();
    assert!(cabecera.ends_with("/render.rs"), "{cabecera}");
    assert!(cabecera.contains("  …/"), "{cabecera}");
    assert!(cabecera.graphemes(true).count() <= SUMMARY_GRAPHEMES + "view  ".len());
    assert_eq!(token_of(&lines[0], "render.rs"), Some(StyleToken::ToolPath));
}
