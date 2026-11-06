//! Proveedores locales en el catálogo: cada entrada del registro se traduce a un `CatalogProvider`
//! según lo que respondió su sondeo, de modo que los selectores de `/settings` y `/model` la traten
//! como a cualquier otro proveedor, sin lógica nueva.
//!
//! El estado (`up`, `down`, `sin acceso`) viaja en el nombre visible y no en la estructura del
//! catálogo, así no se toca ningún tipo público del catálogo.

use crate::catalog::{CatalogModel, CatalogProvider};
use crate::config::ProviderKind;
use crate::providers_probe::ProbeStatus;
use crate::providers_registry::LocalProvider;

/// Traduce el campo `kind` del registro al tipo de SDK; `None` si no se reconoce.
pub fn tipo_de(kind: &str) -> Option<ProviderKind> {
    match kind {
        "openai_compat" => Some(ProviderKind::OpenAiCompat),
        "openai_responses" => Some(ProviderKind::OpenAiResponses),
        "anthropic" => Some(ProviderKind::Anthropic),
        "gemini" => Some(ProviderKind::Gemini),
        _ => None,
    }
}

/// Construye el proveedor de catálogo de una entrada del registro a partir de su sondeo.
///
/// Con `up` se listan los modelos descubiertos; con `down` o `sin acceso` (y con `up` sin modelos)
/// queda el modelo de reserva del registro. El nombre muestra el estado, p. ej.
/// «llama-server · up (3)», «· down» o «· sin acceso (clave)». `None` si el tipo no se reconoce.
pub fn proveedor_local(provider: &LocalProvider, status: &ProbeStatus) -> Option<CatalogProvider> {
    let Some(kind) = tipo_de(&provider.kind) else {
        tracing::warn!(id = %provider.id, kind = %provider.kind, "tipo de proveedor local desconocido; se omite");
        return None;
    };
    let reserva = || vec![provider.model.clone()];
    let (estado, ids) = match status {
        ProbeStatus::Up { models } if !models.is_empty() => {
            (format!("up ({})", models.len()), models.clone())
        }
        ProbeStatus::Up { .. } => ("up (0)".to_string(), reserva()),
        ProbeStatus::Down => ("down".to_string(), reserva()),
        ProbeStatus::Auth => ("sin acceso (clave)".to_string(), reserva()),
    };
    Some(CatalogProvider {
        id: provider.id.clone(),
        name: format!("{} · {estado}", provider.name),
        kind,
        base_url: provider.base_url.clone(),
        api_key_env: provider.api_key_env.clone().unwrap_or_default(),
        models: ids
            .into_iter()
            .map(|id| modelo(id, provider.context_window))
            .collect(),
    })
}

/// Modelo mínimo: solo el identificador y, si el registro lo declara, la ventana de contexto.
fn modelo(id: String, context_window: Option<u32>) -> CatalogModel {
    CatalogModel {
        name: id.clone(),
        id,
        context: context_window.map(u64::from),
        max_output: None,
        input_cost: None,
        output_cost: None,
        reasoning: None,
        release_date: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    fn registro(id: &str, base_url: &str, api_key_env: Option<&str>) -> LocalProvider {
        LocalProvider {
            id: id.into(),
            name: format!("{id}-server"),
            kind: "openai_compat".into(),
            base_url: base_url.into(),
            api_key_env: api_key_env.map(Into::into),
            model: format!("{id}-reserva"),
            context_window: Some(65_536),
        }
    }

    fn ids(provider: &CatalogProvider) -> Vec<&str> {
        provider.models.iter().map(|m| m.id.as_str()).collect()
    }

    #[test]
    fn un_servidor_up_lista_sus_modelos_con_su_estado_en_el_nombre() {
        let local = registro("llama", "http://127.0.0.1:8081/v1", None);
        let status = ProbeStatus::Up {
            models: vec!["uno".into(), "dos".into(), "tres".into()],
        };

        let provider = proveedor_local(&local, &status).expect("tipo conocido");

        assert_eq!(provider.id, "llama");
        assert_eq!(provider.name, "llama-server · up (3)");
        assert_eq!(provider.kind, ProviderKind::OpenAiCompat);
        assert_eq!(provider.base_url, "http://127.0.0.1:8081/v1");
        assert_eq!(ids(&provider), ["uno", "dos", "tres"]);
        assert_eq!(provider.models[0].name, "uno");
        assert_eq!(provider.models[0].context, Some(65_536));
    }

    #[test]
    fn un_servidor_caido_conserva_su_modelo_de_reserva_y_dice_down() {
        let local = registro("llama", "http://127.0.0.1:8081/v1", None);

        let provider = proveedor_local(&local, &ProbeStatus::Down).expect("tipo conocido");

        assert_eq!(provider.name, "llama-server · down");
        assert_eq!(ids(&provider), ["llama-reserva"]);
    }

    #[test]
    fn falta_de_clave_dice_sin_acceso_y_conserva_la_reserva() {
        let local = registro("strata", "http://127.0.0.1:8080/v1", Some("STRATA_API_KEY"));

        let provider = proveedor_local(&local, &ProbeStatus::Auth).expect("tipo conocido");

        assert_eq!(provider.name, "strata-server · sin acceso (clave)");
        assert_eq!(ids(&provider), ["strata-reserva"]);
        assert_eq!(provider.api_key_env, "STRATA_API_KEY");
    }

    #[test]
    fn un_servidor_up_sin_modelos_usa_la_reserva() {
        let local = registro("llama", "http://127.0.0.1:8081/v1", None);

        let provider =
            proveedor_local(&local, &ProbeStatus::Up { models: vec![] }).expect("tipo conocido");

        assert_eq!(provider.name, "llama-server · up (0)");
        assert_eq!(ids(&provider), ["llama-reserva"]);
    }

    #[test]
    fn sin_api_key_env_la_clave_queda_vacia() {
        let local = registro("llama", "http://127.0.0.1:8081/v1", None);

        let provider = proveedor_local(&local, &ProbeStatus::Down).expect("tipo conocido");

        assert_eq!(provider.api_key_env, "");
    }

    #[test]
    fn un_tipo_desconocido_se_omite() {
        let mut local = registro("raro", "http://127.0.0.1:9/v1", None);
        local.kind = "xyz".into();

        assert!(proveedor_local(&local, &ProbeStatus::Down).is_none());
    }

    #[test]
    fn los_tipos_del_registro_se_traducen_al_sdk() {
        assert_eq!(tipo_de("openai_compat"), Some(ProviderKind::OpenAiCompat));
        assert_eq!(
            tipo_de("openai_responses"),
            Some(ProviderKind::OpenAiResponses)
        );
        assert_eq!(tipo_de("anthropic"), Some(ProviderKind::Anthropic));
        assert_eq!(tipo_de("gemini"), Some(ProviderKind::Gemini));
        assert_eq!(tipo_de("otro"), None);
    }

    #[test]
    fn con_locales_al_principio_y_sustituyendo_por_id() {
        let remoto = |id: &str, name: &str| CatalogProvider {
            id: id.into(),
            name: name.into(),
            kind: ProviderKind::OpenAiCompat,
            base_url: format!("https://{id}.test/v1"),
            api_key_env: String::new(),
            models: vec![],
        };
        let base = Catalog::from_providers(vec![
            remoto("openrouter", "OpenRouter"),
            remoto("llama", "Llama de models.dev"),
        ]);
        let local = proveedor_local(
            &registro("llama", "http://127.0.0.1:8081/v1", None),
            &ProbeStatus::Down,
        )
        .expect("tipo conocido");

        let catalogo = base.with_local_providers(vec![local]);

        let nombres: Vec<&str> = catalogo.providers.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(nombres, ["llama-server · down", "OpenRouter"]);
        assert_eq!(
            catalogo
                .providers
                .iter()
                .filter(|p| p.id == "llama")
                .count(),
            1,
            "el id repetido se sustituye, no se duplica"
        );
    }

    #[test]
    fn sin_locales_el_catalogo_no_cambia() {
        let base = Catalog::from_providers(vec![CatalogProvider {
            id: "x".into(),
            name: "X".into(),
            kind: ProviderKind::Anthropic,
            base_url: "https://x.test".into(),
            api_key_env: String::new(),
            models: vec![],
        }]);

        let catalogo = base.clone().with_local_providers(vec![]);

        assert_eq!(catalogo, base);
    }

    #[test]
    fn los_modelos_descubiertos_son_consultables_por_url_y_modelo() {
        let local = registro("llama", "http://127.0.0.1:8081/v1", None);
        let status = ProbeStatus::Up {
            models: vec!["uno".into()],
        };
        let catalogo = Catalog::default()
            .with_local_providers(vec![
                proveedor_local(&local, &status).expect("tipo conocido")
            ]);

        assert_eq!(
            catalogo.knows_model("http://127.0.0.1:8081/v1", "uno"),
            Some(true)
        );
        assert_eq!(
            catalogo.context_window("http://127.0.0.1:8081/v1", "uno"),
            Some(65_536)
        );
    }
}
