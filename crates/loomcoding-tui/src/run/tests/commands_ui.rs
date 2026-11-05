use super::*;

#[test]
fn crush_and_classic_commands_switch_the_look() {
    use loomcoding_cloth::widgets::{Look, ThemePreset};
    let mut app = App::new();
    app.composer.set_text("/crush");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert_eq!(app.theme.look, Look::crush());
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Theme: Crush · Charmtone")
    );

    app.composer.set_text("/classic");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert_eq!(app.theme_preset, ThemePreset::Cloth);
    assert_eq!(app.theme.look, Look::classic());
    assert_eq!(app.slash_feedback.as_deref(), Some("Theme: Cloth"));
}

#[test]
fn layout_command_cycles() {
    use loomcoding_cloth::widgets::LayoutStyle;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    assert_eq!(app.theme.look.layout, LayoutStyle::Workspace);
    for (expected, feedback) in [
        (LayoutStyle::ContextRail, "layout: rail"),
        (LayoutStyle::Minimal, "layout: minimal"),
        (LayoutStyle::Cards, "layout: cards"),
        (LayoutStyle::ContextRail, "layout: rail"),
    ] {
        app.composer.set_text("/layout");
        assert_eq!(
            handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
            None
        );
        assert_eq!(app.theme.look.layout, expected);
        assert_eq!(app.slash_feedback.as_deref(), Some(feedback));
        assert_eq!(crate::prefs::load_layout(&path), Some(expected));
    }
}

#[test]
fn agents_command_opens_a_popup_that_esc_closes() {
    let mut app = App::new();
    app.composer.set_text("/agents");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(app.agents_open);
    assert!(app.composer.is_empty());

    // Con el popup abierto, las teclas no llegan al composer.
    handle_key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    assert!(app.composer.is_empty());

    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.agents_open);
}

#[test]
fn a2a_command_opens_a_navigable_protocol_observatory() {
    let mut app = App::new();
    app.composer.set_text("/a2a");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

    assert!(app.a2a_open);
    assert!(app.composer.is_empty());
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.a2a_focus, 1);
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(app.a2a_focus, 2);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.a2a_open);
}

#[test]
fn mouse_command_toggles_capture_and_reports() {
    let mut app = App::new();
    assert!(app.mouse_capture);
    app.composer.set_text("/mouse");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(!app.mouse_capture);
    assert!(app.mouse_capture_dirty);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Mouse capture off — select text with the mouse")
    );
    assert!(app.composer.is_empty());

    app.composer.set_text("/mouse");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(app.mouse_capture);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Mouse capture on (wheel scrolls)")
    );
}

#[test]
fn verbose_command_toggles_provider_notices_and_reports() {
    let mut app = App::new();
    assert!(!app.verbose_provider_notices);
    app.composer.set_text("/verbose");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(app.verbose_provider_notices);
    assert_eq!(app.slash_feedback.as_deref(), Some("Provider notices: on"));
    assert!(app.composer.is_empty());

    app.composer.set_text("/verbose");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(!app.verbose_provider_notices);
    assert_eq!(app.slash_feedback.as_deref(), Some("Provider notices: off"));
}

#[test]
fn verbose_command_takes_an_explicit_value_and_rejects_junk() {
    let mut app = App::new();
    app.composer.set_text("/verbose on");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(app.verbose_provider_notices);

    app.composer.set_text("/verbose off");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(!app.verbose_provider_notices);
    assert_eq!(app.slash_feedback.as_deref(), Some("Provider notices: off"));

    app.composer.set_text("/verbose maybe");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(!app.verbose_provider_notices);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Usage: /verbose [on|off]")
    );
}
