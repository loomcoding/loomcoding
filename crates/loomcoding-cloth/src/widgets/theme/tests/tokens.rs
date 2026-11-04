//! Tests: tokens.

use super::*;

#[test]
fn test_theme_resolves_tokens() {
    let th = Theme::test();
    assert_eq!(th.style(StyleToken::Error).fg, Some(Color::Red));
    assert_eq!(th.style(StyleToken::Success).fg, Some(Color::Green));
}

#[test]
fn glyphs_have_defaults() {
    let g = Glyphs::default();
    assert_eq!(g.tool_success, '✓');
    assert_eq!(g.tool_error, '×');
    assert_eq!(g.todo_completed, '✓');
}

#[test]
fn partial_toml_overrides_only_named_tokens() {
    let th = Theme::from_toml(
        r#"
        [styles]
        error = "indexed:196"
        "#,
    )
    .unwrap();
    assert_eq!(th.style(StyleToken::Error).fg, Some(Color::Indexed(196)));
    // Un token no nombrado conserva el default.
    assert_eq!(
        th.style(StyleToken::Success).fg,
        Theme::default_dark().style(StyleToken::Success).fg
    );
}

#[test]
fn hex_and_named_colors_parse() {
    let th = Theme::from_toml(
        r#"
        [styles]
        accent = '#00d4ff'
        user = 'cyan'
        "#,
    )
    .unwrap();
    assert_eq!(
        th.style(StyleToken::Accent).fg,
        Some(Color::Rgb(0, 212, 255))
    );
    assert_eq!(th.style(StyleToken::User).fg, Some(Color::Cyan));
}

#[test]
fn unknown_token_goes_to_extensions() {
    let th = Theme::from_toml(
        r#"
        [styles]
        my_custom = "magenta"
        "#,
    )
    .unwrap();
    assert_eq!(
        th.extensions.get("my_custom").unwrap().fg,
        Some(Color::Magenta)
    );
}

#[test]
fn invalid_color_is_an_error() {
    assert!(Theme::from_toml("[styles]\nerror = \"not-a-color\"\n").is_err());
}

#[test]
fn accent_gradient_tokens_resolve() {
    let th = Theme::test();
    assert_eq!(
        th.style(StyleToken::AccentFrom).fg,
        Some(Color::Rgb(0, 212, 255))
    );
    assert_eq!(
        th.style(StyleToken::AccentTo).fg,
        Some(Color::Rgb(124, 92, 255))
    );
    assert_eq!(
        th.style(StyleToken::LogoField).fg,
        Some(Color::Indexed(238))
    );
}

#[test]
fn accent_tokens_can_be_overridden() {
    let th = Theme::from_toml("[styles]\naccent_from = \"magenta\"\n").unwrap();
    assert_eq!(th.style(StyleToken::AccentFrom).fg, Some(Color::Magenta));
}

#[test]
fn scrollbar_token_resolves_and_overrides() {
    let th = Theme::test();
    assert_eq!(
        th.style(StyleToken::Scrollbar).fg,
        Some(Color::Indexed(240))
    );
    let th = Theme::from_toml("[styles]\nscrollbar = \"magenta\"\n").unwrap();
    assert_eq!(th.style(StyleToken::Scrollbar).fg, Some(Color::Magenta));
}

#[test]
fn border_token_resolves_and_overrides() {
    let th = Theme::test();
    assert_eq!(th.style(StyleToken::Border).fg, Some(Color::Indexed(240)));
    let th = Theme::from_toml("[styles]\nborder = \"magenta\"\n").unwrap();
    assert_eq!(th.style(StyleToken::Border).fg, Some(Color::Magenta));
}

#[test]
fn diff_tokens_resolve_and_override() {
    let th = Theme::test();
    assert_eq!(th.style(StyleToken::DiffInsert).fg, Some(Color::Green));
    assert_eq!(th.style(StyleToken::DiffHunk).fg, Some(Color::Cyan));
    let th = Theme::from_toml("[styles]\ndiff_delete = \"magenta\"\n").unwrap();
    assert_eq!(th.style(StyleToken::DiffDelete).fg, Some(Color::Magenta));
}

#[test]
fn selection_token_is_a_background() {
    let th = Theme::test();
    assert_eq!(
        th.style(StyleToken::Selection).bg,
        Some(Color::Indexed(238))
    );
    let th = Theme::from_toml("[styles]\nselection = \"blue\"\n").unwrap();
    assert_eq!(th.style(StyleToken::Selection).bg, Some(Color::Blue));
}

#[test]
fn markdown_tokens_resolve() {
    use ratatui::style::Modifier;
    let th = Theme::test();
    assert_eq!(th.style(StyleToken::Code).fg, Some(Color::Indexed(180)));
    assert!(th
        .style(StyleToken::Strong)
        .add_modifier
        .contains(Modifier::BOLD));
    assert!(th
        .style(StyleToken::Emphasis)
        .add_modifier
        .contains(Modifier::ITALIC));
}

#[test]
fn style_token_round_trips_through_key() {
    for &token in crate::state::transcript::ALL_TOKENS {
        assert_eq!(StyleToken::from_key(token.key()), Some(token));
    }
}

#[test]
fn style_token_accepts_dashed_form() {
    assert_eq!(
        StyleToken::from_key("tool-running"),
        Some(StyleToken::ToolRunning)
    );
    assert_eq!(StyleToken::from_key("  ACCENT "), Some(StyleToken::Accent));
    assert_eq!(StyleToken::from_key("no-existe"), None);
}
