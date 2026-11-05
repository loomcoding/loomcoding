use super::*;

#[test]
fn set_theme_changes_the_look() {
    use loomcoding_cloth::widgets::{Look, ThemePreset};
    let mut app = App::new();
    app.use_product_default_view();
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert_eq!(app.theme.look, Look::crush());
    app.set_theme(ThemePreset::Cloth);
    assert_eq!(app.theme.look, Look::classic());
}

#[test]
fn set_prefs_path_applies_the_saved_theme_without_rewriting() {
    use loomcoding_cloth::widgets::ThemePreset;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    // Espacios finales: si el fichero se reescribiera, desaparecerían.
    let original = "theme = \"crush\"   \n";
    std::fs::write(&path, original).unwrap();
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert!(app.slash_feedback.is_none());
    assert_eq!(app.prefs_path.as_deref(), Some(path.as_path()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn set_theme_persists_when_a_path_is_set() {
    use loomcoding_cloth::widgets::ThemePreset;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    app.set_theme(ThemePreset::MastraCode);
    assert_eq!(
        crate::prefs::load_theme(&path),
        Some(ThemePreset::MastraCode)
    );
}

#[test]
fn a_save_error_does_not_break_the_theme_change() {
    use loomcoding_cloth::widgets::ThemePreset;
    let dir = tempfile::tempdir().unwrap();
    // Un fichero normal como «directorio» padre: la escritura siempre falla.
    let blocker = dir.path().join("bloqueo");
    std::fs::write(&blocker, "x").unwrap();
    let mut app = App::new();
    app.set_prefs_path(blocker.join("ui.toml"));
    app.set_theme(ThemePreset::Crush);
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert!(app
        .slash_feedback
        .as_deref()
        .unwrap()
        .starts_with("Theme not saved"));
}

#[test]
fn toggle_mouse_capture_marks_dirty_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    assert!(app.mouse_capture);
    assert!(!app.mouse_capture_dirty);
    app.set_prefs_path(path.clone());
    app.toggle_mouse_capture();
    assert!(!app.mouse_capture);
    assert!(app.mouse_capture_dirty);
    assert!(app.slash_feedback.is_none());
    assert_eq!(crate::prefs::load_mouse(&path), Some(false));
    app.toggle_mouse_capture();
    assert!(app.mouse_capture);
    assert_eq!(crate::prefs::load_mouse(&path), Some(true));
}

#[test]
fn set_prefs_path_loads_mouse_without_marking_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    // Espacios finales: si el fichero se reescribiera, desaparecerían.
    let original = "mouse = false   \n";
    std::fs::write(&path, original).unwrap();
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    assert!(!app.mouse_capture);
    assert!(!app.mouse_capture_dirty);
    assert!(app.slash_feedback.is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn set_prefs_path_applies_saved_matrix_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let saved = loomcoding_cloth::widgets::matrix_rain::MatrixConfig {
        glyphs: loomcoding_cloth::widgets::matrix_rain::GlyphSet::binary(),
        speed: 2.5,
        density: 60,
        length: 14,
        color: loomcoding_cloth::state::StyleToken::Accent,
    };
    crate::prefs::save_matrix(&path, &saved).unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.matrix_rain.config(), &saved);
}

#[test]
fn set_prefs_path_without_matrix_keeps_theme_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "mouse = true\n").unwrap();
    let mut app = App::new();
    let from_theme = app.theme.matrix.clone();
    app.set_prefs_path(path);
    assert_eq!(app.matrix_rain.config(), &from_theme);
}

#[test]
fn temas_opens_at_root() {
    let mut app = App::new();
    app.open_temas();
    assert!(app.temas_open);
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_eq!(app.temas_selected, 0);
    assert!(!app.temas_dirty);
}

#[test]
fn temas_enter_walks_down_the_tree() {
    let mut app = App::new();
    app.open_temas();
    // Raíz → Efectos
    app.temas_enter();
    assert_eq!(app.temas_level, TemasLevel::Effects);
    // Efectos → Matrix
    app.temas_enter();
    assert_eq!(app.temas_level, TemasLevel::Matrix);
    assert_eq!(app.temas_selected, 0);
}

#[test]
fn temas_esc_walks_up_and_closes_at_root() {
    let mut app = App::new();
    app.open_temas();
    app.temas_enter();
    app.temas_enter();
    assert_eq!(app.temas_level, TemasLevel::Matrix);
    app.temas_back();
    assert_eq!(app.temas_level, TemasLevel::Effects);
    app.temas_back();
    assert_eq!(app.temas_level, TemasLevel::Root);
    app.temas_back();
    assert!(!app.temas_open);
}

#[test]
fn temas_selection_does_not_wrap() {
    let mut app = App::new();
    app.open_temas();
    app.temas_move(-1);
    assert_eq!(app.temas_selected, 0);
    app.temas_move(1);
    assert_eq!(app.temas_selected, 1);
    app.temas_move(1);
    assert_eq!(app.temas_selected, 2);
    app.temas_move(1);
    assert_eq!(app.temas_selected, 2);
}

#[test]
fn temas_theme_level_applies_preset() {
    let mut app = App::new();
    app.open_temas();
    app.temas_move(1);
    app.temas_enter();
    assert_eq!(app.temas_level, TemasLevel::Theme);
    let before = app.theme_preset;
    app.temas_move(1);
    app.temas_enter();
    assert_ne!(app.theme_preset, before);
}

#[test]
fn temas_matrix_adjust_marks_dirty_and_previews() {
    let mut app = App::new();
    app.open_temas();
    app.temas_enter();
    app.temas_enter();
    let before = app.temas_matrix.speed;
    app.temas_move(1);
    app.temas_adjust(true);
    assert!(app.temas_matrix.speed > before);
    assert!(app.temas_dirty);
    // La previsualización aplica al widget en vivo.
    assert_eq!(app.matrix_rain.config().speed, app.temas_matrix.speed);
}

#[test]
fn temas_matrix_glyph_cycle_changes_preset() {
    let mut app = App::new();
    app.open_temas();
    app.temas_enter();
    app.temas_enter();
    let before = app.temas_matrix.glyphs.preset_name();
    app.temas_adjust(true);
    assert_ne!(app.temas_matrix.glyphs.preset_name(), before);
}

#[test]
fn temas_matrix_values_are_clamped() {
    let mut app = App::new();
    app.open_temas();
    app.temas_enter();
    app.temas_enter();
    // Densidad: bajar 200 veces no debe pasar de 0.
    app.temas_move(2);
    for _ in 0..200 {
        app.temas_adjust(false);
    }
    assert_eq!(app.temas_matrix.density, 0);
    // Longitud: subir 300 veces no debe pasar de 200.
    app.temas_move(1);
    for _ in 0..300 {
        app.temas_adjust(true);
    }
    assert_eq!(app.temas_matrix.length, 200);
}

#[test]
fn temas_save_persists_and_clears_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    app.open_temas();
    app.temas_enter();
    app.temas_enter();
    app.temas_move(1);
    app.temas_adjust(true);
    assert!(app.temas_dirty);
    app.temas_save();
    assert!(!app.temas_dirty);
    // Se guardó de verdad: recargar lo devuelve.
    let loaded = crate::prefs::load_matrix(
        &path,
        &loomcoding_cloth::widgets::matrix_rain::MatrixConfig::default(),
    );
    assert_eq!(loaded.speed, app.temas_matrix.speed);
}

#[test]
fn temas_close_discards_unsaved_changes() {
    let mut app = App::new();
    app.open_temas();
    app.temas_enter();
    app.temas_enter();
    app.temas_move(1);
    app.temas_adjust(true);
    assert!(app.temas_dirty);
    app.close_temas();
    assert!(!app.temas_open);
    assert!(!app.temas_dirty);
}

#[test]
fn mouse_save_error_does_not_block_the_change() {
    let dir = tempfile::tempdir().unwrap();
    // Un fichero normal como «directorio» padre: la escritura siempre falla.
    let blocker = dir.path().join("bloqueo");
    std::fs::write(&blocker, "x").unwrap();
    let mut app = App::new();
    app.set_prefs_path(blocker.join("ui.toml"));
    app.toggle_mouse_capture();
    assert!(!app.mouse_capture);
    assert!(app.mouse_capture_dirty);
    assert!(app
        .slash_feedback
        .as_deref()
        .unwrap()
        .starts_with("Mouse setting not saved"));
}

#[test]
fn pane_at_prefers_the_composer_over_the_transcript() {
    let mut app = App::new();
    let transcript = Rect::new(0, 0, 40, 20);
    let composer = Rect::new(0, 15, 40, 5);
    // El transcript va primero a propósito: el composer debe ganar igualmente.
    app.pane_rects = vec![(Focus::Transcript, transcript), (Focus::Composer, composer)];
    app.composer_area = Some(composer);
    assert_eq!(app.pane_at(5, 16), Some(Focus::Composer));
    assert_eq!(app.pane_at(5, 3), Some(Focus::Transcript));
}

#[test]
fn pane_at_returns_none_outside_any_pane() {
    let mut app = App::new();
    app.pane_rects = vec![(Focus::Transcript, Rect::new(0, 0, 40, 20))];
    assert_eq!(app.pane_at(50, 3), None);
    // Los bordes derecho e inferior quedan fuera del rectángulo.
    assert_eq!(app.pane_at(40, 0), None);
    assert_eq!(app.pane_at(3, 20), None);
}

fn settings_fixture() -> ProviderSettings {
    ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "model-a".into(),
        base_url: "https://example.test/v1".into(),
        api_key_env: "API_KEY".into(),
        effort: None,
    }
}

fn app_with_settings_open() -> App {
    let mut app = App::new();
    app.set_provider_settings(settings_fixture());
    app.open_settings();
    app
}

#[test]
fn settings_not_dirty_when_opened() {
    let app = app_with_settings_open();
    assert!(app.settings_open);
    assert!(!app.settings_dirty());
}

#[test]
fn editing_a_field_makes_settings_dirty() {
    let mut app = app_with_settings_open();
    app.provider_settings.as_mut().unwrap().model = "model-b".into();
    assert!(app.settings_dirty());
}

#[test]
fn esc_with_unsaved_changes_warns_first() {
    let mut app = app_with_settings_open();
    app.provider_settings.as_mut().unwrap().model = "model-b".into();
    assert!(!app.close_settings());
    assert!(app.settings_open);
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Unsaved changes · Esc again to discard")
    );
    assert_eq!(app.provider_settings.as_ref().unwrap().model, "model-b");
}

#[test]
fn second_esc_discards_and_restores() {
    let mut app = app_with_settings_open();
    app.provider_settings.as_mut().unwrap().model = "model-b".into();
    assert!(!app.close_settings());
    assert!(app.close_settings());
    assert!(!app.settings_open);
    assert!(!app.settings_dirty());
    assert_eq!(app.provider_settings.as_ref().unwrap().model, "model-a");
    assert_eq!(app.model, "model-a");
}

#[test]
fn esc_without_changes_closes_at_once() {
    let mut app = app_with_settings_open();
    assert!(app.close_settings());
    assert!(!app.settings_open);
}

#[test]
fn successful_save_clears_dirty() {
    let mut app = app_with_settings_open();
    app.provider_settings.as_mut().unwrap().model = "model-b".into();
    app.settings_saving = true;
    app.settings_saved(Ok(true));
    assert!(!app.settings_dirty());
    assert!(app.close_settings());
}

#[test]
fn editing_after_warning_cancels_the_warning() {
    let mut app = app_with_settings_open();
    app.provider_settings.as_mut().unwrap().model = "model-b".into();
    assert!(!app.close_settings());
    app.settings_field = 1;
    app.edit_settings_value();
    app.settings_editor.set_text("model-c");
    app.commit_settings_value();
    assert_eq!(app.settings_feedback, None);
    assert!(!app.close_settings());
    assert!(app.settings_open);
}

#[test]
fn set_catalog_replaces_the_catalog() {
    use loomcoding_core::catalog::Catalog;

    let sample = r#"{"anthropic": {"id": "anthropic", "name": "Anthropic",
        "npm": "@ai-sdk/anthropic", "env": ["ANTHROPIC_API_KEY"],
        "models": {"m": {"id": "m", "tool_call": true}}}}"#;

    let mut app = App::new();
    assert!(app.catalog.is_empty(), "por defecto el catálogo está vacío");

    app.set_catalog(std::sync::Arc::new(Catalog::from_json(sample).unwrap()));
    assert_eq!(app.catalog.providers.len(), 1);
    assert_eq!(app.catalog.providers[0].id, "anthropic");

    app.set_catalog(std::sync::Arc::new(Catalog::default()));
    assert!(
        app.catalog.is_empty(),
        "el catálogo nuevo sustituye al anterior"
    );
}

fn anthropic_settings_open() -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: "model-a".into(),
        base_url: "https://api.anthropic.com".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.set_catalog(picker_sample_catalog());
    app.open_settings();
    app
}

#[test]
fn picker_filters_by_query() {
    let mut app = app_with_settings_open();
    app.set_catalog(picker_sample_catalog());
    app.open_picker(SettingsPicker::Provider);
    assert_eq!(app.picker_items().len(), 4);
    app.picker_query = "ANTH".into();
    let items = app.picker_items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "anthropic");
    app.picker_query = "nada".into();
    assert!(app.picker_items().is_empty());
}

#[test]
fn choosing_a_provider_fills_kind_url_and_env() {
    let mut app = app_with_settings_open();
    app.set_catalog(picker_sample_catalog());
    app.open_picker(SettingsPicker::Provider);
    app.picker_query = "anthropic".into();
    app.picker_choose();
    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(
        settings.kind,
        loomcoding_core::config::ProviderKind::Anthropic
    );
    assert_eq!(settings.base_url, "https://api.anthropic.com");
    assert_eq!(settings.api_key_env, "ANTHROPIC_API_KEY");
    assert_eq!(
        settings.model, "model-a",
        "elegir proveedor no toca el modelo"
    );
    // Tras elegir proveedor se pasa a la lista de sus modelos.
    assert_eq!(app.settings_picker, Some(SettingsPicker::Model));
    assert_eq!(app.settings_field, 1);
}

#[test]
fn unknown_model_typed_in_the_picker_is_accepted() {
    let mut app = app_with_settings_open();
    app.set_catalog(picker_sample_catalog());
    app.settings_field = 1;
    app.open_picker(SettingsPicker::Provider);
    app.picker_query = "anthropic".into();
    app.picker_choose();
    app.picker_query = "mi-modelo-nuevo".into();
    app.picker_choose();
    assert_eq!(
        app.provider_settings.as_ref().unwrap().model,
        "mi-modelo-nuevo"
    );
    assert!(app.settings_picker.is_none());
}

#[test]
fn provider_without_key_variable_keeps_the_current_one() {
    use loomcoding_core::catalog::{Catalog, CatalogProvider};

    let mut app = app_with_settings_open();
    app.set_catalog(Arc::new(Catalog::from_providers(vec![CatalogProvider {
        id: "local".into(),
        name: "Local".into(),
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        base_url: "http://localhost:11434/v1".into(),
        api_key_env: String::new(),
        models: Vec::new(),
    }])));
    app.open_picker(SettingsPicker::Provider);
    app.picker_choose();
    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.base_url, "http://localhost:11434/v1");
    assert_eq!(settings.api_key_env, "API_KEY");
}

#[test]
fn choosing_a_model_fills_the_model() {
    let mut app = anthropic_settings_open();
    app.open_picker(SettingsPicker::Model);
    let ids: Vec<String> = app.picker_items().into_iter().map(|i| i.id).collect();
    assert_eq!(ids, ["claude-sonnet", "claude-opus"]);
    app.picker_selected = 1;
    app.picker_choose();
    assert_eq!(app.provider_settings.as_ref().unwrap().model, "claude-opus");
}

#[test]
fn provider_picker_includes_native_apis_without_a_catalog() {
    let mut app = app_with_settings_open();
    app.open_picker(SettingsPicker::Provider);
    assert_eq!(app.picker_items().len(), 2);
    assert_eq!(app.picker_items()[0].id, "openai-responses");
    assert_eq!(app.picker_items()[1].id, "gemini-interactions");
    assert_eq!(app.settings_picker, Some(SettingsPicker::Provider));
    app.picker_query = "gemini".into();
    app.picker_choose();
    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.kind, ProviderKind::Gemini);
    assert_eq!(settings.api_key_env, "GEMINI_API_KEY");
    assert_eq!(
        settings.base_url,
        "https://generativelanguage.googleapis.com/v1beta"
    );
}

#[test]
fn model_picker_without_matching_provider_sets_feedback() {
    // La URL de `settings_fixture` (example.test) no está en el catálogo de muestra.
    let mut app = app_with_settings_open();
    app.set_catalog(picker_sample_catalog());
    app.open_picker(SettingsPicker::Model);
    assert!(app.settings_picker.is_none());
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("No catalog provider matches the current URL")
    );
    assert!(!app.settings_dirty());
}

#[test]
fn picker_changes_count_as_unsaved() {
    let mut app = anthropic_settings_open();
    app.open_picker(SettingsPicker::Model);
    app.picker_selected = 1;
    app.picker_choose();
    assert!(app.settings_dirty());
    assert_eq!(app.settings_change_count(), 1);
    assert!(
        !app.close_settings(),
        "elegir del selector cuenta como cambio sin guardar"
    );
    assert!(app.settings_open);
}

#[test]
fn manual_edit_still_works_after_picking() {
    let mut app = anthropic_settings_open();
    app.open_picker(SettingsPicker::Model);
    app.picker_choose();
    app.settings_field = 1;
    app.edit_settings_value();
    app.settings_editor.set_text("model-manual");
    app.commit_settings_value();
    assert_eq!(
        app.provider_settings.as_ref().unwrap().model,
        "model-manual"
    );
}

/// Diálogo abierto sobre `model` del catálogo de razonamiento, sin esfuerzo.
fn reasoning_app(model: &str) -> App {
    let mut app = App::new();
    app.set_catalog(reasoning_sample_catalog());
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: model.into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    app
}

fn effort_of(app: &App) -> Option<String> {
    app.provider_settings.as_ref().unwrap().effort.clone()
}

#[test]
fn effort_options_follow_the_catalog_model() {
    let mut app = reasoning_app("claude-think");
    assert_eq!(app.effort_options(), ["low", "medium", "high"]);
    app.provider_settings.as_mut().unwrap().model = "claude-plain".into();
    assert!(app.effort_options().is_empty());
}

#[test]
fn cycle_effort_walks_default_and_values() {
    let mut app = reasoning_app("claude-think");
    app.cycle_effort(true);
    assert_eq!(effort_of(&app).as_deref(), Some("low"));
    app.cycle_effort(true);
    app.cycle_effort(true);
    assert_eq!(effort_of(&app).as_deref(), Some("high"));
    app.cycle_effort(true);
    assert_eq!(effort_of(&app), None, "tras el último vuelve a «default»");
    app.cycle_effort(false);
    assert_eq!(effort_of(&app).as_deref(), Some("high"));
}

#[test]
fn cycle_effort_does_nothing_without_options() {
    let mut app = reasoning_app("claude-plain");
    app.cycle_effort(true);
    assert_eq!(effort_of(&app), None);
    assert!(!app.settings_effort_visible());
}

#[test]
fn effort_field_hidden_for_non_reasoning_model() {
    let mut app = reasoning_app("claude-plain");
    assert_eq!(app.settings_save_field(), SETTINGS_EFFORT_FIELD);
    app.provider_settings.as_mut().unwrap().model = "claude-think".into();
    assert!(app.settings_effort_visible());
    assert_eq!(app.settings_save_field(), SETTINGS_EFFORT_FIELD + 1);
}

#[test]
fn changing_model_drops_unsupported_effort() {
    let mut app = reasoning_app("claude-think");
    app.provider_settings.as_mut().unwrap().effort = Some("high".into());
    app.open_picker(SettingsPicker::Model);
    app.picker_query = "mid".into();
    app.picker_choose();
    assert_eq!(effort_of(&app), None);
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Effort reset: not supported by this model")
    );

    // Un nivel que el modelo nuevo sí admite se conserva.
    app.provider_settings.as_mut().unwrap().effort = Some("medium".into());
    app.open_picker(SettingsPicker::Model);
    app.picker_query = "think".into();
    app.picker_choose();
    assert_eq!(effort_of(&app).as_deref(), Some("medium"));
    assert_eq!(app.settings_feedback, None);

    // La edición manual del modelo también descarta el nivel no admitido.
    app.provider_settings.as_mut().unwrap().effort = Some("high".into());
    app.settings_field = 1;
    app.edit_settings_value();
    app.settings_editor.set_text("claude-mid");
    app.commit_settings_value();
    assert_eq!(effort_of(&app), None);
}

#[test]
fn effort_change_counts_as_unsaved() {
    let mut app = reasoning_app("claude-think");
    app.cycle_effort(true);
    assert!(app.settings_dirty());
    assert!(app.settings_field_changed(SETTINGS_EFFORT_FIELD));
    assert_eq!(app.settings_change_count(), 1);
    assert!(
        !app.close_settings(),
        "el esfuerzo cuenta como cambio sin guardar"
    );
}

#[test]
fn initial_settings_take_effort_from_config() {
    let config = LoomcodingConfig {
        provider: loomcoding_core::config::ProviderConfig {
            kind: ProviderKind::Anthropic,
            base_url: "https://api.anthropic.com/v1".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
        },
        model: "claude-think".into(),
        max_steps: 8,
        effort: Some("low".into()),
    };
    let settings = ProviderSettings::from(&config);
    assert_eq!(settings.effort.as_deref(), Some("low"));
}

#[test]
fn declared_efforts_fill_in_when_the_catalog_knows_no_levels() {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "ornith-local".into(),
        base_url: "http://127.0.0.1:8081/v1".into(),
        api_key_env: "LLAMA_API_KEY".into(),
        effort: None,
    });
    assert!(app.effort_options().is_empty());
    assert!(!app.settings_effort_visible());

    app.set_declared_efforts(vec!["none".into(), "low".into(), "xhigh".into()]);
    assert_eq!(app.effort_options(), ["none", "low", "xhigh"]);
    assert!(app.settings_effort_visible());
    app.cycle_effort(true);
    assert_eq!(effort_of(&app).as_deref(), Some("none"));
}

#[test]
fn the_catalog_levels_win_over_the_declared_ones() {
    let mut app = reasoning_app("claude-think");
    app.set_declared_efforts(vec!["none".into()]);
    assert_eq!(app.effort_options(), ["low", "medium", "high"]);
}
