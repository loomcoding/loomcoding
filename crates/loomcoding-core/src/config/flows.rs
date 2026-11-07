//! Tablas `[flows.<kind>]` de `loomcoding.toml`: el interruptor de cada modo de flujo.
//!
//! El núcleo solo guarda el valor. No sabe qué modos existen ni cuál es su valor por defecto: eso
//! lo decide quien publica el modo, y se pasa a `enabled` como `default`. Una clave ausente da ese
//! valor; una clave desconocida dentro de la tabla se ignora.

use super::escribir_config;
use crate::error::{CoreError, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Interruptores de los modos de flujo, por `kind`. Sin ninguna tabla, todos valen su default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowsConfig {
    enabled: BTreeMap<String, bool>,
}

/// Forma de una tabla `[flows.<kind>]`. Una clave ausente queda en `None`.
#[derive(Deserialize)]
struct Interruptor {
    enabled: Option<bool>,
}

/// Forma del fichero a efectos de `[flows.*]`.
#[derive(Deserialize)]
struct FicheroConFlows {
    #[serde(default)]
    flows: BTreeMap<String, Interruptor>,
}

impl FlowsConfig {
    /// Lee las tablas `[flows.*]` de un texto TOML. Su ausencia da el valor por defecto de cada modo.
    pub fn from_toml(text: &str) -> Result<FlowsConfig> {
        let fichero = toml::from_str::<FicheroConFlows>(text)
            .map_err(|e| CoreError::Config(e.to_string()))?;
        let enabled = fichero
            .flows
            .into_iter()
            .filter_map(|(kind, interruptor)| interruptor.enabled.map(|on| (kind, on)))
            .collect();
        Ok(FlowsConfig { enabled })
    }

    pub fn load(path: &Path) -> Result<FlowsConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }

    /// Si el modo `kind` está encendido. Sin clave, vale `default`.
    pub fn enabled(&self, kind: &str, default: bool) -> bool {
        self.enabled.get(kind).copied().unwrap_or(default)
    }

    /// Guarda `[flows.<kind>] enabled` en el fichero, dejando el resto intacto (comentarios incluidos).
    /// Rechaza un `kind` fuera de `[a-z0-9_-]{1,32}` antes de tocar nada.
    pub fn save_enabled(path: &Path, kind: &str, enabled: bool) -> Result<()> {
        if !kind_valido(kind) {
            return Err(CoreError::Config(format!(
                "kind de flujo no válido: {kind:?} (usa [a-z0-9_-], hasta 32 caracteres)"
            )));
        }
        let mut document = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| CoreError::Config(format!("parse {}: {e}", path.display())))?;
        // Tablas explícitas: sin ellas, `toml_edit` escribiría tablas en línea.
        if !document.contains_key("flows") {
            document.insert("flows", toml_edit::table());
        }
        let flows = document["flows"]
            .as_table_like_mut()
            .ok_or_else(|| CoreError::Config("[flows] no es una tabla".into()))?;
        if !flows.contains_key(kind) {
            flows.insert(kind, toml_edit::table());
        }
        let interruptor = flows
            .get_mut(kind)
            .and_then(|item| item.as_table_like_mut())
            .ok_or_else(|| CoreError::Config(format!("[flows.{kind}] no es una tabla")))?;
        interruptor.insert("enabled", toml_edit::value(enabled));
        escribir_config(path, &document.to_string())
    }
}

/// Un `kind` válido: de 1 a 32 caracteres en `[a-z0-9_-]`.
fn kind_valido(kind: &str) -> bool {
    (1..=32).contains(&kind.len())
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::FlowsConfig;

    #[test]
    fn flujo_sin_seccion_usa_el_valor_por_defecto() {
        let flows = FlowsConfig::from_toml("model = \"x\"\n").unwrap();
        assert!(flows.enabled("uno", true));
        assert!(!flows.enabled("uno", false));
    }

    #[test]
    fn tabla_sin_clave_usa_el_valor_por_defecto() {
        let flows = FlowsConfig::from_toml("[flows.uno]\n").unwrap();
        assert!(flows.enabled("uno", true));
    }

    #[test]
    fn enabled_false_gana_al_valor_por_defecto() {
        let flows = FlowsConfig::from_toml("[flows.uno]\nenabled = false\n").unwrap();
        assert!(!flows.enabled("uno", true));
        // Otro kind no se ve afectado.
        assert!(flows.enabled("otro", true));
    }

    #[test]
    fn enabled_no_booleano_se_rechaza() {
        assert!(FlowsConfig::from_toml("[flows.uno]\nenabled = \"no\"\n").is_err());
    }

    #[test]
    fn clave_desconocida_dentro_de_la_tabla_se_ignora() {
        let flows = FlowsConfig::from_toml("[flows.uno]\nenabled = false\nfoo = 1\n").unwrap();
        assert!(!flows.enabled("uno", true));
    }

    #[test]
    fn flows_que_no_es_tabla_de_kinds_se_rechaza() {
        assert!(FlowsConfig::from_toml("[flows]\nuno = true\n").is_err());
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

        FlowsConfig::save_enabled(&path, "uno", false).unwrap();
        let texto = std::fs::read_to_string(&path).unwrap();
        assert!(texto.contains("# comentario del proyecto"), "{texto}");
        assert!(texto.contains("verbose_provider_notices = true"), "{texto}");
        assert!(texto.contains("[flows.uno]"), "{texto}");
        assert!(!FlowsConfig::load(&path).unwrap().enabled("uno", true));

        FlowsConfig::save_enabled(&path, "uno", true).unwrap();
        assert!(FlowsConfig::load(&path).unwrap().enabled("uno", false));
    }

    #[test]
    fn guardar_un_kind_no_toca_los_demas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        std::fs::write(&path, "[flows.a]\nenabled = false\n").unwrap();

        FlowsConfig::save_enabled(&path, "b", false).unwrap();
        let flows = FlowsConfig::load(&path).unwrap();
        assert!(!flows.enabled("a", true));
        assert!(!flows.enabled("b", true));
        assert!(flows.enabled("c", true));
    }

    #[test]
    fn guardar_rechaza_kinds_que_no_son_validos_y_no_escribe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        let original = "model = \"x\"\n";
        std::fs::write(&path, original).unwrap();
        let demasiado_largo = "a".repeat(33);
        for kind in [
            "",
            "MAYUS",
            "con espacio",
            "punto.malo",
            demasiado_largo.as_str(),
        ] {
            assert!(
                FlowsConfig::save_enabled(&path, kind, false).is_err(),
                "{kind:?}"
            );
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        // Los límites válidos sí se aceptan.
        let justo = "a".repeat(32);
        assert!(FlowsConfig::save_enabled(&path, &justo, false).is_ok());
        assert!(FlowsConfig::save_enabled(&path, "a-b_9", false).is_ok());
    }
}
