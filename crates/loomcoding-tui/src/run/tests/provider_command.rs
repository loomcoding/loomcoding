//! `/provider` desde el compositor y su selector rápido: `Enter` aplica, `Esc` cancela sin cambios,
//! un id desconocido deja el texto escrito y avisa, y el pegado no llega al compositor.

use super::*;
use loomcoding_core::catalog::{Catalog, CatalogModel, CatalogProvider};
use loomcoding_core::config::ProviderKind;

const URL_LLAMA: &str = "http://127.0.0.1:8081/v1";

/// Escribe el comando en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

fn modelo(id: &str) -> CatalogModel {
    CatalogModel {
        id: id.into(),
        name: id.into(),
        context: None,
        max_output: None,
        input_cost: None,
        output_cost: None,
        reasoning: None,
        release_date: None,
    }
}

/// Aplicación con ajustes de Anthropic y un catálogo con un local (llama) y uno remoto.
fn app_con_catalogo() -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: ProviderKind::Anthropic,
        model: "claude-sonnet".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.set_catalog(Arc::new(Catalog::from_providers(vec![
        CatalogProvider {
            id: "llama".into(),
            name: "llama-server · up (1)".into(),
            kind: ProviderKind::OpenAiCompat,
            base_url: URL_LLAMA.into(),
            api_key_env: String::new(),
            models: vec![modelo("Ornith-1.5-9B")],
        },
        CatalogProvider {
            id: "anthropic".into(),
            name: "Anthropic".into(),
            kind: ProviderKind::Anthropic,
            base_url: "https://api.anthropic.com/v1".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            models: vec![modelo("claude-sonnet")],
        },
    ])));
    app
}

fn proveedor_base_url(app: &App) -> String {
    app.provider_settings.as_ref().unwrap().base_url.clone()
}

#[test]
fn provider_con_argumento_aplica_desde_el_compositor() {
    let mut app = app_con_catalogo();

    assert_eq!(escribir(&mut app, "/provider llama"), None);

    assert_eq!(proveedor_base_url(&app), URL_LLAMA);
    assert!(app.settings_save_requested);
    assert!(app.composer.is_empty());
}

#[test]
fn un_id_desconocido_deja_el_texto_escrito_y_avisa() {
    let mut app = app_con_catalogo();

    assert_eq!(escribir(&mut app, "/provider nope"), None);

    assert!(app
        .slash_feedback
        .as_deref()
        .is_some_and(|text| text.starts_with("Unknown provider: nope")));
    assert_eq!(app.composer.text(), "/provider nope");
    assert_eq!(proveedor_base_url(&app), "https://api.anthropic.com/v1");
    assert!(!app.settings_save_requested);
}

#[test]
fn provider_sin_argumento_abre_el_selector_y_enter_aplica_la_fila() {
    let mut app = app_con_catalogo();

    assert_eq!(escribir(&mut app, "/provider"), None);
    assert!(app.quick_provider_picker);
    assert!(!app.settings_open);
    assert!(app.composer.is_empty());

    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

    assert!(!app.quick_provider_picker);
    assert_eq!(proveedor_base_url(&app), "https://api.anthropic.com/v1");
    assert_eq!(
        app.provider_settings.as_ref().unwrap().model,
        "claude-sonnet"
    );
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("provider: anthropic (unchanged)")
    );

    escribir(&mut app, "/provider");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

    assert_eq!(proveedor_base_url(&app), URL_LLAMA);
    assert_eq!(
        app.provider_settings.as_ref().unwrap().model,
        "Ornith-1.5-9B"
    );
    assert!(app.settings_save_requested);
    assert!(app.settings_picker.is_none());
    assert!(!app.settings_open);
}

#[test]
fn escape_cierra_el_selector_sin_cambios() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/provider");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);

    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);

    assert!(!app.quick_provider_picker);
    assert_eq!(proveedor_base_url(&app), "https://api.anthropic.com/v1");
    assert!(!app.settings_save_requested);
}

#[test]
fn escribir_en_el_selector_filtra_la_lista() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/provider");

    for c in "llama".chars() {
        handle_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
    }
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

    assert_eq!(proveedor_base_url(&app), URL_LLAMA);
}

#[test]
fn un_pegado_con_el_selector_abierto_no_llega_al_compositor() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/provider");
    assert!(
        app.quick_provider_picker,
        "el selector de proveedor debía abrirse"
    );

    handle_event(&mut app, Event::Paste("texto pegado".into()));

    assert!(
        app.composer.is_empty(),
        "el pegado no debe colarse en el compositor: {:?}",
        app.composer.text()
    );
}
