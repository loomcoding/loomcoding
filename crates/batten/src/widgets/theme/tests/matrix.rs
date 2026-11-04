//! Tests: matrix.

use super::*;

#[test]
fn matrix_section_defaults_when_absent() {
    let th = Theme::from_toml("[styles]\naccent = \"cyan\"\n").unwrap();
    assert_eq!(th.matrix, MatrixConfig::default());
    assert_eq!(th.matrix.color, StyleToken::Success);
}

#[test]
fn matrix_section_overrides_fields() {
    let th = Theme::from_toml(
        r#"
        [effects.matrix]
        glyphs = "emoji"
        speed = 1.5
        density = 40
        length = 12
        color = "accent"
        "#,
    )
    .unwrap();
    assert_eq!(th.matrix.glyphs, GlyphSet::emoji());
    assert_eq!(th.matrix.speed, 1.5);
    assert_eq!(th.matrix.density, 40);
    assert_eq!(th.matrix.length, 12);
    assert_eq!(th.matrix.color, StyleToken::Accent);
}

#[test]
fn matrix_section_accepts_partial_fields() {
    let th = Theme::from_toml("[effects.matrix]\nspeed = 2.0\n").unwrap();
    assert_eq!(th.matrix.speed, 2.0);
    // Lo no especificado cae al default.
    assert_eq!(th.matrix.glyphs, GlyphSet::katakana());
    assert_eq!(th.matrix.density, 3);
}

#[test]
fn matrix_section_clamps_out_of_range_values() {
    let th = Theme::from_toml(
        r#"
        [effects.matrix]
        speed = 999.0
        density = 250
        length = 0
        "#,
    )
    .unwrap();
    assert_eq!(th.matrix.speed, 4.0);
    assert_eq!(th.matrix.density, 100);
    assert_eq!(th.matrix.length, 1);
}

#[test]
fn matrix_section_rejects_unknown_color_token() {
    let err = Theme::from_toml("[effects.matrix]\ncolor = \"no-existe\"\n").unwrap_err();
    assert!(matches!(err, ThemeError::Parse(_)));
}

#[test]
fn matrix_section_accepts_custom_glyphs() {
    let th = Theme::from_toml("[effects.matrix]\nglyphs = \"abc\"\n").unwrap();
    assert_eq!(th.matrix.glyphs.preset_name(), "propio");
    assert_eq!(th.matrix.glyphs.chars(), &['a', 'b', 'c']);
}

#[test]
fn matrix_section_rejects_mixed_width_glyphs() {
    let err = Theme::from_toml("[effects.matrix]\nglyphs = \"a💧\"\n").unwrap_err();
    assert!(matches!(err, ThemeError::Parse(_)));
}
