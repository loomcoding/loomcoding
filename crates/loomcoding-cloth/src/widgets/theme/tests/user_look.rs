//! Tests: `style` y `layout` opcionales en un tema de usuario (`theme.toml`).
//!
//! Son valores por defecto del tema: `ui.toml` los sobrescribe después, en la TUI.

use super::*;

#[test]
fn user_theme_applies_style_and_layout() {
    let th = Theme::from_toml("style = \"flat\"\nlayout = \"minimal\"\n").unwrap();
    assert_eq!(th.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(th.look.layout, LayoutStyle::Minimal);
}

#[test]
fn user_theme_without_style_or_layout_keeps_the_default_look() {
    let th = Theme::from_toml("[styles]\naccent = \"cyan\"\n").unwrap();
    assert_eq!(th.look, Theme::default_dark().look);
}

#[test]
fn user_theme_style_alone_keeps_the_default_layout() {
    let th = Theme::from_toml("style = \"flat\"\n").unwrap();
    assert_eq!(th.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(th.look.layout, Theme::default_dark().look.layout);
}

#[test]
fn user_theme_layout_alone_keeps_the_default_bundle() {
    let th = Theme::from_toml("layout = \"cards\"\n").unwrap();
    assert_eq!(th.look.layout, LayoutStyle::Cards);
    assert_eq!(th.look.bundle(), Theme::default_dark().look.bundle());
}

#[test]
fn user_theme_unknown_style_is_a_readable_error() {
    let err = Theme::from_toml("style = \"otro\"\n").unwrap_err();
    assert!(matches!(err, ThemeError::Parse(_)));
    assert!(err.to_string().contains("otro"), "{err}");
}

#[test]
fn user_theme_unknown_layout_is_a_readable_error() {
    let err = Theme::from_toml("layout = \"otro\"\n").unwrap_err();
    assert!(matches!(err, ThemeError::Parse(_)));
    assert!(err.to_string().contains("otro"), "{err}");
}

#[test]
fn user_theme_wrong_type_is_an_error_not_a_panic() {
    assert!(Theme::from_toml("style = 3\n").is_err());
    assert!(Theme::from_toml("layout = true\n").is_err());
}
