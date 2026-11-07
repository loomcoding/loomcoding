//! Sección `[odd]` de `loomcoding.toml`: el interruptor de ODD.
//!
//! El núcleo solo guarda el valor; lo que hace ODD vive en `loomcoding-workflows`, y la interfaz
//! solo lo muestra. Sin la sección, ODD viene encendido.

use super::escribir_config;
use crate::error::{CoreError, Result};
use serde::Deserialize;
use std::path::Path;

/// Sección `[odd]`. Sin ella, o sin la clave `enabled`, vale `true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct OddConfig {
    /// Apagado, ODD no publica fases ni bloquea el cierre del turno.
    pub enabled: bool,
}

impl Default for OddConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Forma del fichero a efectos de `[odd]`.
#[derive(Deserialize)]
struct FicheroConOdd {
    #[serde(default)]
    odd: OddConfig,
}

impl OddConfig {
    /// Lee la sección `[odd]` de un texto TOML. Su ausencia da los valores por defecto.
    pub fn from_toml(text: &str) -> Result<OddConfig> {
        toml::from_str::<FicheroConOdd>(text)
            .map(|fichero| fichero.odd)
            .map_err(|e| CoreError::Config(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<OddConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }

    /// Guarda `enabled` en `[odd]` del fichero, dejando el resto intacto (comentarios incluidos).
    pub fn save_enabled(path: &Path, enabled: bool) -> Result<()> {
        let mut document = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| CoreError::Config(format!("parse {}: {e}", path.display())))?;
        // Una tabla explícita: sin ella, `toml_edit` la escribe como tabla en línea.
        if !document.contains_key("odd") {
            document["odd"] = toml_edit::table();
        }
        document["odd"]["enabled"] = toml_edit::value(enabled);
        escribir_config(path, &document.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::OddConfig;

    #[test]
    fn odd_viene_activo_sin_seccion() {
        let odd = OddConfig::from_toml("model = \"x\"\n").unwrap();
        assert!(odd.enabled);
    }

    #[test]
    fn odd_viene_activo_con_seccion_sin_clave() {
        let odd = OddConfig::from_toml("[odd]\n").unwrap();
        assert!(odd.enabled);
    }

    #[test]
    fn odd_respeta_enabled_false() {
        let odd = OddConfig::from_toml("[odd]\nenabled = false\n").unwrap();
        assert!(!odd.enabled);
    }

    #[test]
    fn odd_rechaza_un_valor_que_no_es_booleano() {
        assert!(OddConfig::from_toml("[odd]\nenabled = \"no\"\n").is_err());
    }

    #[test]
    fn guardar_cambia_solo_la_clave_y_conserva_el_resto() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        std::fs::write(
            &path,
            "# comentario del proyecto\nmodel = \"x\"\n\n[ui]\nverbose_provider_notices = true\n",
        )
        .unwrap();

        OddConfig::save_enabled(&path, false).unwrap();
        let texto = std::fs::read_to_string(&path).unwrap();
        assert!(texto.contains("# comentario del proyecto"), "{texto}");
        assert!(texto.contains("verbose_provider_notices = true"), "{texto}");
        assert!(!OddConfig::load(&path).unwrap().enabled);

        OddConfig::save_enabled(&path, true).unwrap();
        assert!(OddConfig::load(&path).unwrap().enabled);
    }
}
