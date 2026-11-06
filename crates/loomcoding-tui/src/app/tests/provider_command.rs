//! `/provider` a nivel de API y la corrección previa de `/settings`: un servidor de bucle local
//! se guarda sin variable de clave, y una URL remota sigue exigiéndola.

use super::*;
use crate::commands::{Command, COMMANDS};
use loomcoding_core::catalog::{Catalog, CatalogModel, CatalogProvider, Reasoning};
use loomcoding_core::config::ProviderKind;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

const URL_LLAMA: &str = "http://127.0.0.1:8081/v1";

/// Ajustes con la URL indicada y sin variable de clave, como un `llama-server` sin clave.
fn ajustes_sin_clave(base_url: &str) -> ProviderSettings {
    ProviderSettings {
        kind: ProviderKind::OpenAiCompat,
        model: "Ornith-1.5-9B".into(),
        base_url: base_url.into(),
        api_key_env: String::new(),
        effort: None,
    }
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

fn proveedor(
    id: &str,
    name: &str,
    base_url: &str,
    api_key_env: &str,
    modelos: &[&str],
) -> CatalogProvider {
    CatalogProvider {
        id: id.into(),
        name: name.into(),
        kind: ProviderKind::OpenAiCompat,
        base_url: base_url.into(),
        api_key_env: api_key_env.into(),
        models: modelos.iter().map(|id| modelo(id)).collect(),
    }
}

/// Catálogo de muestra: un local con estado en el nombre, uno remoto y uno sin modelos.
fn catalogo() -> Arc<Catalog> {
    Arc::new(Catalog::from_providers(vec![
        proveedor(
            "llama",
            "llama-server · up (3)",
            URL_LLAMA,
            "",
            &["Ornith-1.5-9B", "otro"],
        ),
        proveedor(
            "anthropic",
            "Anthropic",
            "https://api.anthropic.com/v1",
            "ANTHROPIC_API_KEY",
            &["claude-sonnet"],
        ),
        proveedor(
            "vacio",
            "Vacío",
            "https://vacio.example.test/v1",
            "VACIO_KEY",
            &[],
        ),
    ]))
}

/// Aplicación con ajustes de Anthropic y el catálogo de muestra.
fn app_con_catalogo() -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: ProviderKind::Anthropic,
        model: "model-a".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.set_catalog(catalogo());
    app
}

/// Texto completo de un fotograma de `app` de `w`×`h` celdas.
fn pintado(app: &mut App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|frame| crate::render::render(frame, app))
        .unwrap();
    term.backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn un_servidor_local_se_guarda_sin_variable_de_clave() {
    let mut app = App::new();
    app.set_provider_settings(ajustes_sin_clave(URL_LLAMA));

    app.request_settings_save();

    assert!(app.settings_save_requested);
    assert_eq!(app.settings_feedback, None);
}

#[test]
fn una_clave_escrita_sin_variable_avisa_y_no_guarda() {
    let mut app = App::new();
    app.set_provider_settings(ajustes_sin_clave(URL_LLAMA));
    app.settings_api_key = "sk-de-prueba".into();

    app.request_settings_save();

    assert!(!app.settings_save_requested);
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Name the API key variable to store this key")
    );
}

#[test]
fn una_url_remota_sigue_exigiendo_la_variable_de_clave() {
    let mut app = App::new();
    app.set_provider_settings(ajustes_sin_clave("https://api.example.test/v1"));

    app.request_settings_save();

    assert!(!app.settings_save_requested);
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Model, URL and API key variable are required")
    );
}

#[test]
fn provider_con_id_fija_tipo_url_clave_y_modelo_por_el_camino_de_guardado() {
    let mut app = app_con_catalogo();

    assert_eq!(app.set_provider_now("llama"), Ok(()));

    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.kind, ProviderKind::OpenAiCompat);
    assert_eq!(settings.base_url, URL_LLAMA);
    assert_eq!(settings.api_key_env, "");
    assert_eq!(settings.model, "Ornith-1.5-9B");
    assert!(app.settings_save_requested);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("provider: llama · model: Ornith-1.5-9B")
    );
}

#[test]
fn el_modelo_actual_se_conserva_si_el_proveedor_lo_publica() {
    let mut app = app_con_catalogo();
    app.provider_settings.as_mut().unwrap().model = "otro".into();

    assert_eq!(app.set_provider_now("llama"), Ok(()));

    assert_eq!(app.provider_settings.as_ref().unwrap().model, "otro");
}

#[test]
fn elegir_el_proveedor_vigente_no_reenvia_nada() {
    let mut app = app_con_catalogo();
    app.set_provider_now("anthropic").unwrap();
    app.settings_save_requested = false;

    assert_eq!(app.set_provider_now("anthropic"), Ok(()));

    assert!(!app.settings_save_requested);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("provider: anthropic (unchanged)")
    );
}

#[test]
fn un_id_desconocido_avisa_con_la_lista_de_ids_validos() {
    let mut app = app_con_catalogo();

    let error = app.set_provider_now("nope").unwrap_err();

    assert!(error.starts_with("Unknown provider: nope"), "{error}");
    assert!(error.contains("llama"), "{error}");
    assert!(error.contains("anthropic"), "{error}");
    assert_eq!(app.provider_settings.as_ref().unwrap().model, "model-a");
    assert!(!app.settings_save_requested);
}

#[test]
fn sin_catalogo_el_comando_avisa() {
    let mut app = App::new();
    app.set_provider_settings(ajustes_sin_clave(URL_LLAMA));

    assert_eq!(
        app.set_provider_now("llama"),
        Err("No provider catalog yet: use /settings".into())
    );
    assert_eq!(
        app.open_provider_picker_now(),
        Err("No provider catalog yet: use /settings".into())
    );
    assert!(!app.quick_provider_picker);
}

#[test]
fn un_proveedor_sin_modelos_avisa_y_no_cambia_nada() {
    let mut app = app_con_catalogo();

    assert_eq!(
        app.set_provider_now("vacio"),
        Err("vacio has no models in the catalog: use /settings".into())
    );

    assert_eq!(
        app.provider_settings.as_ref().unwrap().kind,
        ProviderKind::Anthropic
    );
    assert!(!app.settings_save_requested);
}

#[test]
fn un_turno_en_curso_impide_cambiar_de_proveedor() {
    let mut app = app_con_catalogo();
    app.turn_in_flight = true;

    let error = app.set_provider_now("llama").unwrap_err();

    assert!(error.contains("active turn"), "{error}");
    assert_eq!(
        app.provider_settings.as_ref().unwrap().base_url,
        "https://api.anthropic.com/v1"
    );
    assert!(!app.settings_save_requested);
}

#[test]
fn un_id_con_espacios_o_vacio_no_es_valido() {
    let mut app = app_con_catalogo();

    assert_eq!(
        app.set_provider_now("a b"),
        Err("Usage: /provider [id]".into())
    );
    assert_eq!(
        app.set_provider_now("   "),
        Err("Usage: /provider [id]".into())
    );
}

#[test]
fn el_esfuerzo_que_el_proveedor_nuevo_no_admite_se_descarta_y_se_dice() {
    let mut app = app_con_catalogo();
    let mut con_razonamiento = modelo("razona");
    con_razonamiento.reasoning = Some(Reasoning {
        efforts: vec!["high".into()],
        budget: None,
        toggle: false,
    });
    app.set_catalog(Arc::new(Catalog::from_providers(vec![
        CatalogProvider {
            models: vec![con_razonamiento],
            ..proveedor(
                "razonador",
                "Razonador",
                "https://razona.example.test/v1",
                "",
                &[],
            )
        },
        proveedor(
            "plano",
            "Plano",
            "https://plano.example.test/v1",
            "",
            &["simple"],
        ),
    ])));
    app.set_provider_settings(ProviderSettings {
        base_url: "https://razona.example.test/v1".into(),
        effort: Some("high".into()),
        ..ajustes_sin_clave("https://razona.example.test/v1")
    });

    assert_eq!(app.set_provider_now("plano"), Ok(()));

    assert_eq!(app.provider_settings.as_ref().unwrap().effort, None);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("provider: plano · model: simple · effort reset")
    );
}

#[test]
fn sin_argumento_abre_el_selector_y_pide_refresco() {
    let mut app = app_con_catalogo();
    let cuenta = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dentro = cuenta.clone();
    app.set_refresh_providers_hook(Arc::new(move || {
        dentro.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    assert_eq!(app.open_provider_picker_now(), Ok(()));

    assert!(app.quick_provider_picker);
    assert!(!app.settings_open);
    assert_eq!(cuenta.load(std::sync::atomic::Ordering::SeqCst), 1);
    let ids: Vec<String> = app
        .provider_items()
        .into_iter()
        .map(|item| item.id)
        .collect();
    assert_eq!(ids, ["llama", "anthropic", "vacio"]);
}

#[test]
fn el_selector_muestra_el_nombre_con_su_estado() {
    let mut app = app_con_catalogo();
    app.open_provider_picker_now().unwrap();

    let content = pintado(&mut app, 100, 30);

    assert!(content.contains("llama-server · up (3)"), "{content}");
}

#[test]
fn el_selector_filtra_por_id_o_nombre() {
    let mut app = app_con_catalogo();
    app.open_provider_picker_now().unwrap();
    app.picker_query = "ANTHRO".into();

    let ids: Vec<String> = app
        .provider_items()
        .into_iter()
        .map(|item| item.id)
        .collect();

    assert_eq!(ids, ["anthropic"]);
}

#[test]
fn el_selector_sobrevive_areas_minusculas() {
    let mut app = app_con_catalogo();
    app.open_provider_picker_now().unwrap();
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 5)] {
        let _ = pintado(&mut app, w, h);
    }
}

#[test]
fn provider_esta_en_el_popup_con_su_descripcion_y_no_es_alias_de_ui() {
    let entry = COMMANDS
        .iter()
        .find(|(command, _, _)| *command == Command::Provider)
        .expect("/provider está en la lista de comandos");
    assert_eq!(entry.1, "/provider");
    assert!(!crate::commands_alias::is_hidden_alias(Command::Provider));

    let mut app = App::new();
    app.composer.set_text("/pro");
    assert!(app
        .slash_matches()
        .iter()
        .any(|(command, name, _)| *command == Command::Provider && *name == "/provider"));
}

#[test]
fn provider_admite_argumento_en_slash_query() {
    let mut app = App::new();
    app.composer.set_text("/provider llama");
    assert_eq!(app.slash_query(), Some("/provider"));
}

#[test]
fn la_ayuda_de_80_por_24_nombra_provider() {
    let mut app = App::new();
    app.help_open = true;

    let content = pintado(&mut app, 80, 24);

    assert!(content.contains("/provider"), "{content}");
}
