use crate::error::{CoreError, Result};
use std::path::Path;

/// Cota superior del plazo de una petición, en segundos (una hora); el decálogo exige que toda cota conste.
pub const MAX_REQUEST_TIMEOUT_SECS: u64 = 3600;

/// Claves opcionales `context_window` y `request_timeout_secs` de la sección `[provider]`, para quien usa
/// servidores locales. Van en un cargador aparte, como `ProviderCacheConfig`, porque `ProviderConfig` se
/// construye en muchos sitios; el resto de claves de `[provider]` se ignora.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderLimitsConfig {
    /// Tokens por petición que el servidor procesa con holgura dentro del plazo. Sin valor, manda el catálogo.
    pub context_window: Option<u64>,
    /// Plazo total de cada petición del adaptador OpenAI-compatible, en segundos. Sin valor, rigen 120 s.
    pub request_timeout_secs: Option<u64>,
    /// Niveles de esfuerzo que admite el modelo local, en el orden en que la TUI los ofrece (`none`, `low`,
    /// `xhigh`…): el catálogo no conoce un servidor propio. Vacío, la TUI se atiene al catálogo.
    pub efforts: Vec<String>,
}

/// Cuántos niveles de esfuerzo se pueden declarar como máximo (decálogo: toda lista tiene cota).
pub const MAX_DECLARED_EFFORTS: usize = 16;

/// Lee `efforts` de `[provider]`: lista de cadenas no vacías, como mucho `MAX_DECLARED_EFFORTS`.
fn declared_efforts(provider: Option<&toml::Value>) -> Result<Vec<String>> {
    let Some(value) = provider.and_then(|section| section.get("efforts")) else {
        return Ok(Vec::new());
    };
    let invalid = || CoreError::Config("`efforts` must be a list of non-empty strings".into());
    let list = value.as_array().ok_or_else(invalid)?;
    if list.len() > MAX_DECLARED_EFFORTS {
        return Err(CoreError::Config(format!(
            "`efforts` must have at most {MAX_DECLARED_EFFORTS} levels, not {}",
            list.len()
        )));
    }
    list.iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|level| !level.is_empty())
                .map(str::to_owned)
                .ok_or_else(invalid)
        })
        .collect()
}

/// Lee una clave entera estrictamente positiva de `[provider]`, con cota superior opcional. Su ausencia
/// da `None`; cualquier otro tipo o valor fuera de rango es un error que nombra la clave.
fn positive_provider_key(
    provider: Option<&toml::Value>,
    key: &str,
    max: Option<u64>,
) -> Result<Option<u64>> {
    let Some(value) = provider.and_then(|section| section.get(key)) else {
        return Ok(None);
    };
    let number = value
        .as_integer()
        .ok_or_else(|| CoreError::Config(format!("`{key}` must be a positive integer")))?;
    if number <= 0 {
        return Err(CoreError::Config(format!(
            "`{key}` must be greater than zero, not {number}"
        )));
    }
    let number = number as u64;
    if let Some(max) = max.filter(|max| number > *max) {
        return Err(CoreError::Config(format!(
            "`{key}` must be at most {max}, not {number}"
        )));
    }
    Ok(Some(number))
}

impl ProviderLimitsConfig {
    /// Lee ambas claves de un texto TOML. Su ausencia da `None`.
    pub fn from_toml(text: &str) -> Result<ProviderLimitsConfig> {
        let document: toml::Value =
            toml::from_str(text).map_err(|e| CoreError::Config(e.to_string()))?;
        let provider = document.get("provider");
        Ok(ProviderLimitsConfig {
            context_window: positive_provider_key(provider, "context_window", None)?,
            request_timeout_secs: positive_provider_key(
                provider,
                "request_timeout_secs",
                Some(MAX_REQUEST_TIMEOUT_SECS),
            )?,
            efforts: declared_efforts(provider)?,
        })
    }

    pub fn load(path: &Path) -> Result<ProviderLimitsConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LoomcodingConfig, ProviderConfig, ProviderKind};

    #[test]
    fn provider_limits_are_absent_by_default() {
        for text in ["", "model = \"m\"\n[provider]\nkind = \"openai_compat\"\n"] {
            let limits = ProviderLimitsConfig::from_toml(text).unwrap();
            assert_eq!(limits, ProviderLimitsConfig::default());
            assert_eq!(limits.context_window, None);
            assert_eq!(limits.request_timeout_secs, None);
        }
    }

    #[test]
    fn provider_limits_read_valid_values() {
        let text = "[provider]\ncontext_window = 35584\nrequest_timeout_secs = 600\n";
        let limits = ProviderLimitsConfig::from_toml(text).unwrap();
        assert_eq!(limits.context_window, Some(35_584));
        assert_eq!(limits.request_timeout_secs, Some(600));
        let top = ProviderLimitsConfig::from_toml("[provider]\nrequest_timeout_secs = 3600\n");
        assert_eq!(
            top.unwrap().request_timeout_secs,
            Some(MAX_REQUEST_TIMEOUT_SECS)
        );
    }

    #[test]
    fn an_invalid_provider_limit_names_its_key() {
        let cases: [(&str, &[&str]); 2] = [
            ("context_window", &["0", "-1", "3.5", "\"35k\"", "true"]),
            (
                "request_timeout_secs",
                &["0", "-1", "3.5", "\"10m\"", "3601", "99999999999"],
            ),
        ];
        for (key, values) in cases {
            for value in values {
                let text = format!("[provider]\n{key} = {value}\n");
                let error = ProviderLimitsConfig::from_toml(&text)
                    .expect_err(&format!("{key} = {value} debía rechazarse"));
                assert!(error.to_string().contains(key), "{key} = {value}: {error}");
            }
        }
    }

    #[test]
    fn declared_efforts_are_read_in_order_and_default_to_none() {
        assert!(ProviderLimitsConfig::from_toml("")
            .unwrap()
            .efforts
            .is_empty());
        let text = "[provider]\nefforts = [\"none\", \"low\", \"xhigh\"]\n";
        let limits = ProviderLimitsConfig::from_toml(text).unwrap();
        assert_eq!(limits.efforts, ["none", "low", "xhigh"]);
    }

    #[test]
    fn invalid_declared_efforts_name_their_key() {
        let too_many = format!(
            "[provider]\nefforts = [{}]\n",
            (0..=MAX_DECLARED_EFFORTS)
                .map(|n| format!("\"e{n}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        for text in [
            "[provider]\nefforts = \"high\"\n".to_string(),
            "[provider]\nefforts = [1, 2]\n".to_string(),
            "[provider]\nefforts = [\"\"]\n".to_string(),
            "[provider]\nefforts = [\"  \"]\n".to_string(),
            too_many,
        ] {
            let error = ProviderLimitsConfig::from_toml(&text).unwrap_err();
            assert!(error.to_string().contains("efforts"), "{text}: {error}");
        }
    }

    #[test]
    fn a_broken_file_does_not_blame_a_limit_key() {
        // El mensaje sale del error real: un TOML roto no puede nombrar una clave que no falló.
        let error = ProviderLimitsConfig::from_toml("[provider\nkind = ").unwrap_err();
        let message = error.to_string();
        assert!(!message.contains("request_timeout_secs"), "{message}");
        assert!(!message.contains("context_window"), "{message}");
    }

    #[test]
    fn a_bad_limit_does_not_name_the_other_one() {
        let error =
            ProviderLimitsConfig::from_toml("[provider]\nrequest_timeout_secs = 0\n").unwrap_err();
        assert!(!error.to_string().contains("context_window"), "{error}");
    }

    #[test]
    fn saving_settings_keeps_both_limit_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        std::fs::write(
            &path,
            "model = \"old\"\nmax_steps = 7\n[provider]\nkind = \"openai_compat\"\nbase_url = \"http://127.0.0.1:8091/v1\"\napi_key_env = \"K\"\ncontext_window = 35584\nrequest_timeout_secs = 300\n",
        )
        .unwrap();
        let config = LoomcodingConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://127.0.0.1:8091/v1".into(),
                api_key_env: "K".into(),
            },
            model: "otro-modelo".into(),
            max_steps: 7,
            effort: None,
        };
        config.save(&path).unwrap();
        let limits = ProviderLimitsConfig::load(&path).unwrap();
        assert_eq!(limits.context_window, Some(35_584));
        assert_eq!(limits.request_timeout_secs, Some(300));
        assert_eq!(LoomcodingConfig::load(&path).unwrap().model, "otro-modelo");
    }
}
