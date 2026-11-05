use crate::error::{CoreError, Result};
use serde::Deserialize;
use std::path::Path;

/// Configuración de límites (`[limits]`).
///
/// Vive aparte de `LoomcodingConfig` para que su ausencia no afecte a la configuración principal.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    /// Tope de pasos de lectura en una sola respuesta del proveedor.
    /// Sin valor, `None`; ver `read_step_cap`.
    pub max_read_steps: Option<usize>,
    /// Tope de herramientas paralelas en una sola respuesta del proveedor.
    /// `0` equivale al valor por defecto (8).
    #[serde(default)]
    pub max_parallel_tools: usize,
    /// Tope de peticiones concurrentes al proveedor.
    /// `0` equivale al valor por defecto (4).
    #[serde(default)]
    pub max_concurrent_requests: usize,
    /// Tope de tokens de salida de cada petición, razonamiento incluido.
    /// `0` equivale al valor por defecto (4096); nunca pasa de `MAX_OUTPUT_TOKENS_HARD`.
    ///
    /// Cabe señalar que con un modelo de razonamiento el pensamiento gasta este presupuesto: si es
    /// corto, la respuesta se corta antes de que el modelo llegue a actuar.
    #[serde(default)]
    pub max_output_tokens: u32,
}

/// Valor por defecto de `max_output_tokens`.
pub const DEFAULT_OUTPUT_TOKENS: u32 = 4096;

/// Máximo duro de `max_output_tokens` (decálogo: toda cota tiene techo).
pub const MAX_OUTPUT_TOKENS_HARD: u32 = 32_768;

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_read_steps: None,
            max_parallel_tools: 8,
            max_concurrent_requests: 4,
            max_output_tokens: DEFAULT_OUTPUT_TOKENS,
        }
    }
}

impl LimitsConfig {
    /// Lee la sección `[limits]` de un texto TOML. Su ausencia da los valores por defecto.
    pub fn from_toml(text: &str) -> Result<LimitsConfig> {
        let mut config: LimitsConfig = toml::from_str::<FicheroConLimits>(text)
            .map(|fichero| fichero.limits)
            .map_err(|e| CoreError::Config(e.to_string()))?;
        config.apply_defaults();
        Ok(config)
    }

    /// Lee la sección `[limits]` de un archivo TOML. Su ausencia da los valores por defecto.
    pub fn load(path: &Path) -> Result<LimitsConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }

    /// Aplica la regla de que `0` equivale al valor por defecto.
    fn apply_defaults(&mut self) {
        if self.max_parallel_tools == 0 {
            self.max_parallel_tools = 8;
        }
        if self.max_concurrent_requests == 0 {
            self.max_concurrent_requests = 4;
        }
        if self.max_output_tokens == 0 {
            self.max_output_tokens = DEFAULT_OUTPUT_TOKENS;
        }
        self.max_output_tokens = self.max_output_tokens.min(MAX_OUTPUT_TOKENS_HARD);
    }

    /// Devuelve el tope de pasos de lectura para un `max_steps` dado.
    ///
    /// Si `max_read_steps` está configurado, lo devuelve. Si no, devuelve `4 * max_steps`,
    /// pero nunca menos de 1.
    pub fn read_step_cap(&self, max_steps: usize) -> usize {
        if let Some(cap) = self.max_read_steps {
            cap
        } else {
            let cap = 4 * max_steps;
            cap.max(1)
        }
    }
}

/// Forma del fichero a efectos de `[limits]`.
#[derive(Deserialize)]
struct FicheroConLimits {
    #[serde(default)]
    limits: LimitsConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_default_when_section_missing() {
        let text = "model = \"x\"\n";
        let config = LimitsConfig::from_toml(text).unwrap();
        assert_eq!(config.max_parallel_tools, 8);
        assert_eq!(config.max_concurrent_requests, 4);
        assert_eq!(config.max_read_steps, None);
    }

    #[test]
    fn limits_read_values_from_section() {
        let text = r#"
[limits]
max_parallel_tools = 2
max_concurrent_requests = 3
max_read_steps = 50
"#;
        let config = LimitsConfig::from_toml(text).unwrap();
        assert_eq!(config.max_parallel_tools, 2);
        assert_eq!(config.max_concurrent_requests, 3);
        assert_eq!(config.max_read_steps, Some(50));
    }

    #[test]
    fn zero_limits_fall_back_to_defaults() {
        let text = r#"
[limits]
max_concurrent_requests = 0
"#;
        let config = LimitsConfig::from_toml(text).unwrap();
        assert_eq!(config.max_concurrent_requests, 4);

        let text2 = r#"
[limits]
max_parallel_tools = 0
"#;
        let config2 = LimitsConfig::from_toml(text2).unwrap();
        assert_eq!(config2.max_parallel_tools, 8);
    }

    #[test]
    fn read_step_cap_defaults_to_four_times_max_steps() {
        let config = LimitsConfig::default();
        assert_eq!(config.read_step_cap(30), 120);
        assert_eq!(config.read_step_cap(0), 1);

        let config_with_cap = LimitsConfig {
            max_read_steps: Some(7),
            ..LimitsConfig::default()
        };
        assert_eq!(config_with_cap.read_step_cap(30), 7);
    }

    #[test]
    fn the_output_cap_defaults_to_4096_and_zero_means_default() {
        assert_eq!(LimitsConfig::default().max_output_tokens, 4096);
        let config = LimitsConfig::from_toml(
            "[limits]
max_output_tokens = 0
",
        )
        .unwrap();
        assert_eq!(config.max_output_tokens, 4096);
        let config = LimitsConfig::from_toml(
            "[limits]
max_output_tokens = 12000
",
        )
        .unwrap();
        assert_eq!(config.max_output_tokens, 12000);
    }

    #[test]
    fn the_output_cap_is_clamped_to_the_hard_maximum() {
        let config = LimitsConfig::from_toml(
            "[limits]
max_output_tokens = 9999999
",
        )
        .unwrap();
        assert_eq!(config.max_output_tokens, MAX_OUTPUT_TOKENS_HARD);
    }
}
