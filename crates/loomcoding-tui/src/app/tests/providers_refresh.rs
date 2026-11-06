//! Proveedores locales en la TUI: los modelos descubiertos llegan al selector de `/model` y abrir
//! `/model`, `/settings` o el selector de proveedor pide un sondeo, sin esperarlo.

use super::*;
use loomcoding_core::catalog::{Catalog, CatalogModel, CatalogProvider};
use loomcoding_core::config::ProviderKind;
use std::sync::atomic::{AtomicUsize, Ordering};

const URL_LOCAL: &str = "http://127.0.0.1:8081/v1";

fn modelo(id: &str) -> CatalogModel {
    CatalogModel {
        id: id.into(),
        name: id.into(),
        context: Some(65_536),
        max_output: None,
        input_cost: None,
        output_cost: None,
        reasoning: None,
        release_date: None,
    }
}

/// Aplicación con ajustes de un servidor local y su entrada de catálogo con dos modelos descubiertos.
fn app_con_servidor_local() -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: ProviderKind::OpenAiCompat,
        model: "uno".into(),
        base_url: URL_LOCAL.into(),
        api_key_env: String::new(),
        effort: None,
    });
    app.set_catalog(Arc::new(Catalog::from_providers(vec![CatalogProvider {
        id: "llama".into(),
        name: "llama-server · up (2)".into(),
        kind: ProviderKind::OpenAiCompat,
        base_url: URL_LOCAL.into(),
        api_key_env: String::new(),
        models: vec![modelo("uno"), modelo("dos")],
    }])));
    app
}

/// Instala un enlace de refresco que cuenta las llamadas.
fn con_contador(app: &mut App) -> Arc<AtomicUsize> {
    let cuenta = Arc::new(AtomicUsize::new(0));
    let dentro = cuenta.clone();
    app.set_refresh_providers_hook(Arc::new(move || {
        dentro.fetch_add(1, Ordering::SeqCst);
    }));
    cuenta
}

#[test]
fn los_modelos_descubiertos_aparecen_en_el_selector_de_model() {
    let mut app = app_con_servidor_local();

    assert_eq!(app.open_model_picker_now(), Ok(()));

    let ids: Vec<String> = app.picker_items().into_iter().map(|item| item.id).collect();
    assert_eq!(ids, ["uno", "dos"]);
}

#[test]
fn abrir_model_settings_y_el_selector_de_proveedor_pide_sondeo() {
    let mut app = app_con_servidor_local();
    let cuenta = con_contador(&mut app);

    assert_eq!(app.open_model_picker_now(), Ok(()));
    assert_eq!(cuenta.load(Ordering::SeqCst), 1, "/model");
    app.close_quick_model_picker();

    app.open_settings();
    assert_eq!(cuenta.load(Ordering::SeqCst), 2, "/settings");

    app.open_picker(SettingsPicker::Provider);
    assert_eq!(cuenta.load(Ordering::SeqCst), 3, "selector de proveedor");
}

#[test]
fn el_selector_de_modelo_no_pide_sondeo_por_si_solo() {
    let mut app = app_con_servidor_local();
    let cuenta = con_contador(&mut app);

    app.open_picker(SettingsPicker::Model);

    assert_eq!(cuenta.load(Ordering::SeqCst), 0);
}

#[test]
fn sin_ajustes_model_no_pide_sondeo() {
    let mut app = App::new();
    let cuenta = con_contador(&mut app);

    assert!(app.open_model_picker_now().is_err());

    assert_eq!(cuenta.load(Ordering::SeqCst), 0);
}

#[test]
fn sin_enlace_abrir_los_selectores_no_falla() {
    let mut app = app_con_servidor_local();

    assert_eq!(app.open_model_picker_now(), Ok(()));
    app.open_settings();
    app.open_picker(SettingsPicker::Provider);

    assert_eq!(app.settings_picker, Some(SettingsPicker::Provider));
}
