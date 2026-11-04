//! Tests: presets.

use super::*;

#[test]
fn only_the_crush_preset_has_its_own_look() {
    for preset in ThemePreset::ALL {
        let theme = Theme::preset(preset);
        let expected = if preset == ThemePreset::Crush {
            Look::crush()
        } else {
            Look::classic()
        };
        assert_eq!(theme.look, expected, "{}", preset.name());
    }
    assert_eq!(Theme::default_dark().look, Look::classic());
    assert_eq!(Theme::test().look, Look::classic());
    assert_eq!(Look::crush().composer, ComposerStyle::Crush);
    assert_eq!(Look::crush().tool, ToolStyle::Inline);
    assert_eq!(Look::classic().composer, ComposerStyle::Classic);
    assert_eq!(Look::classic().tool, ToolStyle::Pinned);
    assert_eq!(Look::classic().layout, LayoutStyle::Workspace);
    assert_eq!(Look::classic().chrome, ChromeStyle::Panels);
    assert_eq!(Look::crush().layout, LayoutStyle::ContextRail);
    assert_eq!(Look::crush().chrome, ChromeStyle::Flat);
}

#[test]
fn new_tokens_resolve_in_every_preset() {
    for preset in ThemePreset::ALL {
        let theme = Theme::preset(preset);
        let name = preset.name();
        assert!(
            theme.style(StyleToken::ToolPath).fg.is_some(),
            "{name}: ToolPath sin color"
        );
        assert!(
            theme.style(StyleToken::ToolCount).fg.is_some(),
            "{name}: ToolCount sin color"
        );
        assert_eq!(
            theme.style(StyleToken::ToolCommand),
            theme.style(StyleToken::Default),
            "{name}: ToolCommand debe resolver al estilo por defecto"
        );
    }
}

#[test]
fn defaults_are_derived_from_existing_styles() {
    // Martty no define valores propios de pieza: hereda los derivados.
    let theme = Theme::preset(ThemePreset::Martty);
    assert_eq!(
        theme.style(StyleToken::ToolPath),
        theme.style(StyleToken::Accent)
    );
    assert_eq!(
        theme.style(StyleToken::ToolCommand),
        theme.style(StyleToken::Default)
    );
    assert_eq!(
        theme.style(StyleToken::ToolCount),
        theme.style(StyleToken::Dim)
    );

    // Y el tema por defecto, sin preset, sigue la misma regla.
    let theme = Theme::test();
    assert_eq!(
        theme.style(StyleToken::ToolPath),
        theme.style(StyleToken::Accent)
    );
    assert_eq!(
        theme.style(StyleToken::ToolCount),
        theme.style(StyleToken::Dim)
    );
}

#[test]
fn crush_overrides_tool_path_or_count() {
    let crush = Theme::preset(ThemePreset::Crush);
    let differs = crush.style(StyleToken::ToolPath) != crush.style(StyleToken::Accent)
        || crush.style(StyleToken::ToolCount) != crush.style(StyleToken::Dim);
    assert!(differs, "Crush debe definir su propia ruta o recuento");
}

#[test]
fn tool_piece_tokens_can_be_overridden() {
    let th =
        Theme::from_toml("[styles]\ntool_path = \"magenta\"\ntool_count = \"green\"\n").unwrap();
    assert_eq!(th.style(StyleToken::ToolPath).fg, Some(Color::Magenta));
    assert_eq!(th.style(StyleToken::ToolCount).fg, Some(Color::Green));
    assert!(th.extensions.is_empty());
}

#[test]
fn every_preset_in_all_has_a_name_and_a_theme() {
    for preset in ThemePreset::ALL {
        assert!(!preset.name().is_empty(), "{preset:?} sin nombre");
        // Construir el tema no debe entrar en pánico para ningún preset.
        let _ = Theme::preset(preset);
    }
}

#[test]
fn loomcoding_is_the_last_preset() {
    assert_eq!(ThemePreset::ALL.last(), Some(&ThemePreset::Loomcoding));
    assert_eq!(ThemePreset::Loomcoding.name(), "Loomcoding");
}

#[test]
fn loomcoding_preset_uses_the_logo_colors() {
    let theme = Theme::preset(ThemePreset::Loomcoding);
    assert_eq!(theme.styles.accent.fg, Some(Color::Rgb(0xc2, 0x5a, 0x27)));
    assert_eq!(theme.styles.border.fg, Some(Color::Rgb(0x5c, 0x63, 0x70)));
}

#[test]
fn agent_state_style_maps_each_state() {
    let st = Theme::test().styles;
    assert_eq!(AgentState::Submitted.style(&st), st.muted);
    assert_eq!(AgentState::Working.style(&st), st.tool_running);
    assert_eq!(AgentState::InputRequired.style(&st), st.warning);
    assert_eq!(AgentState::AuthRequired.style(&st), st.warning);
    assert_eq!(AgentState::Completed.style(&st), st.tool_success);
    assert_eq!(AgentState::Failed.style(&st), st.tool_error);
    assert_eq!(AgentState::Canceled.style(&st), st.tool_canceled);
    assert_eq!(AgentState::Rejected.style(&st), st.tool_error);
}
