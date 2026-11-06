//! Registro global de proveedores locales (`providers.toml`).
//!
//! Vive en el perfil de Loomcoding (`<perfil>/.loomcoding/providers.toml`) y lista los servidores
//! de inferencia que el usuario tiene a mano (`llama-server`, Strata, PolyStrata). Las claves nunca
//! se escriben aquí: solo el nombre de la variable de entorno que las contiene (`api_key_env`).

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Contenido inicial de `providers.toml`, con las tres entradas por defecto de la spec.
pub const SEED_PROVIDERS_TOML: &str = r#"# Proveedores locales conocidos. Las claves nunca van aquí: solo `api_key_env`.
default = "llama"

[[provider]]
id = "llama"
name = "llama-server"
kind = "openai_compat"
base_url = "http://127.0.0.1:8081/v1"
model = "Ornith-1.5-9B"
context_window = 65536

[[provider]]
id = "strata"
name = "Strata"
kind = "openai_compat"
base_url = "http://127.0.0.1:8080/v1"
api_key_env = "STRATA_API_KEY"
model = "qwen3.8-flash-next-coder-iq1_m"
context_window = 65536

[[provider]]
id = "polystrata"
name = "PolyStrata (Flint)"
kind = "openai_compat"
base_url = "http://127.0.0.1:8082/v1"
model = "flint-40b-a2.5b-ud-q4_k_xl"
context_window = 65536
"#;

/// Un servidor local conocido por el registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProvider {
    /// Nombre corto, el que se teclea (`/provider llama`).
    pub id: String,
    /// Etiqueta visible.
    pub name: String,
    /// Tipo de API; hoy solo `openai_compat`.
    pub kind: String,
    /// URL base, con el prefijo `/v1`.
    pub base_url: String,
    /// Variable de entorno que contiene la clave, si el servidor la exige.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Modelo de reserva si el servidor no responde al sondeo.
    pub model: String,
    /// Ventana de contexto declarada, opcional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
}

/// Contenido completo de `providers.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvidersRegistry {
    /// Identificador del proveedor por defecto.
    #[serde(default)]
    pub default: Option<String>,
    /// Entradas, en el orden del fichero.
    #[serde(default, rename = "provider")]
    pub providers: Vec<LocalProvider>,
}

/// Errores del registro de proveedores locales.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// No se pudo leer o escribir el fichero.
    #[error("no se pudo acceder a {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// El contenido no es un `providers.toml` válido.
    #[error("providers.toml no es válido: {0}")]
    Parse(#[from] toml::de::Error),
}

impl ProvidersRegistry {
    /// Interpreta el texto de un `providers.toml`.
    ///
    /// Si dos entradas comparten `id`, cabe señalar que se conserva la primera y se avisa de la
    /// segunda, de modo que un fichero descuidado no impida arrancar.
    pub fn parse(text: &str) -> Result<Self, RegistryError> {
        let mut registry: Self = toml::from_str(text)?;
        let mut vistos = std::collections::HashSet::new();
        registry.providers.retain(|provider| {
            let primera_vez = vistos.insert(provider.id.clone());
            if !primera_vez {
                tracing::warn!(id = %provider.id, "id de proveedor duplicado en providers.toml; se ignora la segunda entrada");
            }
            primera_vez
        });
        Ok(registry)
    }

    /// Devuelve el proveedor `default`; si no existe, la primera entrada.
    pub fn default_provider(&self) -> Option<&LocalProvider> {
        self.default
            .as_deref()
            .and_then(|id| self.providers.iter().find(|provider| provider.id == id))
            .or_else(|| self.providers.first())
    }

    /// Lee `path`; si el fichero no existe, lo crea con las entradas por defecto.
    ///
    /// Un fichero existente nunca se sobrescribe, aunque esté corrupto: en ese caso se devuelve
    /// el error de lectura y el contenido queda intacto.
    pub fn load_or_seed(path: &Path) -> Result<Self, RegistryError> {
        let io_error = |source| RegistryError::Io {
            path: path.to_path_buf(),
            source,
        };
        if !path.exists() {
            if let Some(carpeta) = path.parent() {
                std::fs::create_dir_all(carpeta).map_err(io_error)?;
            }
            std::fs::write(path, SEED_PROVIDERS_TOML).map_err(io_error)?;
        }
        let text = std::fs::read_to_string(path).map_err(io_error)?;
        Self::parse(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, base_url: &str) -> String {
        format!(
            "[[provider]]\nid = \"{id}\"\nname = \"{id}\"\nkind = \"openai_compat\"\n\
             base_url = \"{base_url}\"\nmodel = \"m\"\n"
        )
    }

    #[test]
    fn parsea_el_ejemplo_de_la_spec() {
        let registry = ProvidersRegistry::parse(SEED_PROVIDERS_TOML).expect("seed válido");

        let ids: Vec<&str> = registry.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["llama", "strata", "polystrata"]);
        assert_eq!(registry.default.as_deref(), Some("llama"));

        let strata = &registry.providers[1];
        assert_eq!(strata.api_key_env.as_deref(), Some("STRATA_API_KEY"));
        assert_eq!(strata.base_url, "http://127.0.0.1:8080/v1");
        assert_eq!(strata.context_window, Some(65536));

        let llama = &registry.providers[0];
        assert_eq!(llama.api_key_env, None);
        assert_eq!(llama.model, "Ornith-1.5-9B");
    }

    #[test]
    fn load_or_seed_crea_el_fichero_si_falta() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".loomcoding").join("providers.toml");

        let registry = ProvidersRegistry::load_or_seed(&path).expect("siembra");

        assert!(path.exists(), "debe crear el fichero y su carpeta");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SEED_PROVIDERS_TOML);
        assert_eq!(
            registry,
            ProvidersRegistry::parse(SEED_PROVIDERS_TOML).unwrap()
        );
    }

    #[test]
    fn load_or_seed_no_pisa_un_fichero_existente() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("providers.toml");
        let custom = format!(
            "default = \"solo\"\n{}",
            entry("solo", "http://127.0.0.1:9000/v1")
        );
        std::fs::write(&path, &custom).unwrap();

        let registry = ProvidersRegistry::load_or_seed(&path).expect("lee");

        assert_eq!(registry.providers.len(), 1);
        assert_eq!(registry.providers[0].id, "solo");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), custom);
    }

    #[test]
    fn default_inexistente_cae_en_la_primera_entrada() {
        let text = format!(
            "default = \"nope\"\n{}{}",
            entry("uno", "http://a/v1"),
            entry("dos", "http://b/v1")
        );
        let registry = ProvidersRegistry::parse(&text).unwrap();

        assert_eq!(
            registry.default_provider().map(|p| p.id.as_str()),
            Some("uno")
        );
    }

    #[test]
    fn id_duplicado_conserva_la_primera_entrada() {
        let text = format!(
            "{}{}",
            entry("dup", "http://primera/v1"),
            entry("dup", "http://segunda/v1")
        );
        let registry = ProvidersRegistry::parse(&text).unwrap();

        assert_eq!(registry.providers.len(), 1);
        assert_eq!(registry.providers[0].base_url, "http://primera/v1");
    }

    #[test]
    fn toml_corrupto_devuelve_error_y_no_toca_el_fichero() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("providers.toml");
        std::fs::write(&path, "esto no es toml [[").unwrap();

        let result = ProvidersRegistry::load_or_seed(&path);

        assert!(matches!(result, Err(RegistryError::Parse(_))));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "esto no es toml [["
        );
    }
}
