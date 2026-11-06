//! Registro global de proveedores locales en el arranque.
//!
//! Siembra un `loomcoding.toml` nuevo desde el proveedor `default` de `providers.toml` y da la
//! clave ficticia `local` a los servidores de bucle que no piden clave (`llama-server`).

use std::path::Path;

use loomcoding_core::config::is_loopback_url;
use loomcoding_core::providers_registry::{initial_project_toml, ProvidersRegistry};

/// Clave ficticia para un servidor de bucle local que no pide clave.
pub const CLAVE_LOCAL: &str = "local";

/// Lee el registro del perfil (`<perfil>/.loomcoding/providers.toml`), sembrándolo si falta.
///
/// Sin perfil, o si el fichero no se puede usar (corrupto, sin permisos), se avisa con `tracing`
/// y se devuelve `None`: el arranque sigue con el comportamiento anterior.
pub fn cargar_registro_desde_home(home: Option<&Path>) -> Option<ProvidersRegistry> {
    let Some(home) = home else {
        tracing::warn!(
            "sin perfil de usuario: no se lee providers.toml; se usa el valor por defecto"
        );
        return None;
    };
    let ruta = home.join(".loomcoding").join("providers.toml");
    match ProvidersRegistry::load_or_seed(&ruta) {
        Ok(registro) => Some(registro),
        Err(error) => {
            tracing::warn!(error = %error, "providers.toml no se puede usar; se ignora el registro");
            None
        }
    }
}

/// Registro del perfil de usuario actual (`user_home()`, que respeta `LOOMCODING_HOME`).
pub fn cargar_registro_global() -> Option<ProvidersRegistry> {
    cargar_registro_desde_home(crate::home::user_home().as_deref())
}

/// Texto inicial de `loomcoding.toml` tomado del proveedor `default` del registro, si lo hay.
pub fn texto_inicial(registro: Option<&ProvidersRegistry>) -> Option<String> {
    let proveedor = registro?.default_provider()?;
    match initial_project_toml(proveedor) {
        Ok(texto) => Some(texto),
        Err(error) => {
            tracing::warn!(error = %error, "no se pudo generar el loomcoding.toml inicial");
            None
        }
    }
}

/// Clave ficticia para una URL de bucle local; `None` si la URL no es local.
pub fn clave_de_bucle(base_url: &str) -> Option<String> {
    is_loopback_url(base_url).then(|| CLAVE_LOCAL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use loomcoding_core::config::{LoomcodingConfig, ProviderKind};
    use loomcoding_core::providers_registry::{ProvidersRegistry, SEED_PROVIDERS_TOML};
    use std::collections::HashMap;

    fn perfil_con_registro() -> tempfile::TempDir {
        let home = tempfile::tempdir().expect("tempdir");
        let ruta = home.path().join(".loomcoding").join("providers.toml");
        ProvidersRegistry::load_or_seed(&ruta).expect("siembra el registro");
        home
    }

    #[test]
    fn proyecto_sin_toml_nace_con_el_default_del_registro() {
        let home = perfil_con_registro();
        let registro = cargar_registro_desde_home(Some(home.path()));
        let texto = texto_inicial(registro.as_ref());
        let proyecto = tempfile::tempdir().expect("tempdir");
        let ruta = proyecto.path().join("loomcoding.toml");

        let cfg = LoomcodingConfig::load_or_create_with(&ruta, texto.as_deref()).unwrap();

        assert_eq!(cfg.provider.kind, ProviderKind::OpenAiCompat);
        assert_eq!(cfg.provider.base_url, "http://127.0.0.1:8081/v1");
        assert_eq!(cfg.model, "Ornith-1.5-9B");
        assert_eq!(cfg.max_steps, 8);
        assert!(!std::fs::read_to_string(&ruta)
            .unwrap()
            .contains("openrouter"));
    }

    #[test]
    fn un_toml_existente_no_cambia_aunque_haya_registro() {
        let home = perfil_con_registro();
        let registro = cargar_registro_desde_home(Some(home.path()));
        let texto = texto_inicial(registro.as_ref());
        let proyecto = tempfile::tempdir().expect("tempdir");
        let ruta = proyecto.path().join("loomcoding.toml");
        let propio = "model = \"mi-modelo\"\nmax_steps = 3\n\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://example.test/v1\"\napi_key_env = \"MI_CLAVE\"\n";
        std::fs::write(&ruta, propio).unwrap();

        let cfg = LoomcodingConfig::load_or_create_with(&ruta, texto.as_deref()).unwrap();

        assert_eq!(cfg.model, "mi-modelo");
        assert_eq!(cfg.provider.base_url, "https://example.test/v1");
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), propio);
    }

    #[test]
    fn registro_corrupto_cae_al_comportamiento_anterior() {
        let home = tempfile::tempdir().expect("tempdir");
        let ruta_registro = home.path().join(".loomcoding").join("providers.toml");
        std::fs::create_dir_all(ruta_registro.parent().unwrap()).unwrap();
        std::fs::write(&ruta_registro, "esto no es toml [[").unwrap();
        let registro = cargar_registro_desde_home(Some(home.path()));
        assert!(registro.is_none());
        let texto = texto_inicial(registro.as_ref());
        let proyecto = tempfile::tempdir().expect("tempdir");
        let ruta = proyecto.path().join("loomcoding.toml");

        let cfg = LoomcodingConfig::load_or_create_with(&ruta, texto.as_deref()).unwrap();

        assert_eq!(cfg.provider.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn sin_perfil_no_hay_registro() {
        assert!(cargar_registro_desde_home(None).is_none());
    }

    #[test]
    fn llama_sin_clave_construye_un_proveedor_real() {
        let home = perfil_con_registro();
        let registro = cargar_registro_desde_home(Some(home.path()));
        let texto = texto_inicial(registro.as_ref());
        let proyecto = tempfile::tempdir().expect("tempdir");
        let ruta = proyecto.path().join("loomcoding.toml");
        let cfg = LoomcodingConfig::load_or_create_with(&ruta, texto.as_deref()).unwrap();

        let (_, configurado) =
            crate::build_provider_state(&cfg, &HashMap::new(), None, None, None).unwrap();

        assert!(configurado);
    }

    #[test]
    fn url_no_local_sin_clave_sigue_sin_configurar() {
        let proyecto = tempfile::tempdir().expect("tempdir");
        let ruta = proyecto.path().join("loomcoding.toml");
        let texto = "model = \"m\"\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://api.example.test/v1\"\n";
        let cfg = LoomcodingConfig::load_or_create_with(&ruta, Some(texto)).unwrap();

        let (_, configurado) =
            crate::build_provider_state(&cfg, &HashMap::new(), None, None, None).unwrap();

        assert!(!configurado);
    }

    #[test]
    fn la_clave_de_bucle_solo_se_da_a_urls_locales() {
        assert_eq!(
            clave_de_bucle("http://127.0.0.1:8081/v1").as_deref(),
            Some("local")
        );
        assert_eq!(
            clave_de_bucle("http://localhost:8080/v1").as_deref(),
            Some("local")
        );
        assert_eq!(clave_de_bucle("https://api.example.test/v1"), None);
    }

    #[test]
    fn el_texto_inicial_sin_registro_es_none() {
        assert_eq!(texto_inicial(None), None);
        let registro = ProvidersRegistry::parse(SEED_PROVIDERS_TOML).unwrap();
        assert!(texto_inicial(Some(&registro)).is_some());
    }
}
