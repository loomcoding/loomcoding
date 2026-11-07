//! Compresión determinista de la salida de las herramientas `bash` y `job_output`.
//!
//! Sin modelo, sin red y sin estado compartido: cada llamada se filtra por separado. Lo que se
//! omite queda anunciado con una cabecera (R1) y el original íntegro se conserva con `ToolSpill`
//! (R2). Solo herramientas: nunca la respuesta de un subagente ni de una tarea A2A. Véase
//! `docs/specs/2026-11-06-loomcoding-compresion-de-salida.md`.

mod general;
mod header;
#[cfg(test)]
mod tests;

use crate::error::{CoreError, Result};
use crate::tools::spill::ToolSpill;
use crate::tools::{limit_bytes_with_header, ToolOutput, MAX_TOOL_OUTPUT_BYTES};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Entrada por encima de la cual la salida pasa sin filtrar (R8).
pub const MAX_FILTER_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Tiempo máximo de un filtro; si lo supera, la salida pasa sin filtrar (R8).
pub const MAX_FILTER_TIME: Duration = Duration::from_millis(50);

/// Variable de entorno que apaga el filtro en todo el proceso (R10).
pub const ENV_DESACTIVA: &str = "LOOMCODING_NO_FILTER";

/// Herramientas cuya salida pasa por el filtro. Nunca lectura, búsqueda ni subagentes.
const HERRAMIENTAS: [&str; 2] = ["bash", "job_output"];

/// Bytes que se reservan a la cabecera y al marcador de recorte al acotar (R1).
const RESERVA_CABECERA: usize = 512;

/// Sección `[output_filter]` de `loomcoding.toml`. Por defecto, apagada.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FilterConfig {
    enabled: bool,
}

#[derive(Deserialize)]
struct Seccion {
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct FicheroConFiltro {
    #[serde(default)]
    output_filter: Option<Seccion>,
}

/// Resultado de filtrar una salida. Si `rule` es `None`, el contenido es el original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    pub content: String,
    pub omitted_lines: usize,
    pub rule: Option<&'static str>,
    pub original: Option<PathBuf>,
}

impl Filtered {
    fn intacto(content: &str) -> Self {
        Self {
            content: content.to_owned(),
            omitted_lines: 0,
            rule: None,
            original: None,
        }
    }
}

impl FilterConfig {
    /// Lee `[output_filter] enabled`. Sin la sección, el filtro queda apagado.
    pub fn from_toml(texto: &str) -> Result<Self> {
        let fichero = toml::from_str::<FicheroConFiltro>(texto)
            .map_err(|e| CoreError::Config(e.to_string()))?;
        let enabled = fichero
            .output_filter
            .and_then(|seccion| seccion.enabled)
            .unwrap_or(false);
        Ok(Self { enabled })
    }

    /// Carga la sección desde el fichero de configuración del proyecto. Con `LOOMCODING_NO_FILTER=1`
    /// el filtro queda apagado aunque la sección lo encienda.
    pub fn load(path: &Path) -> Result<Self> {
        let texto = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Ok(Self::from_toml(&texto)?.sin_entorno(std::env::var(ENV_DESACTIVA).ok().as_deref()))
    }

    /// Apaga el filtro si la variable de entorno lo pide. Recibe el valor ya leído para poder probarlo.
    fn sin_entorno(self, valor: Option<&str>) -> Self {
        if valor == Some("1") {
            Self { enabled: false }
        } else {
            self
        }
    }
}

/// Filtra la salida de una llamada. Devuelve el original sin tocar si el filtro está apagado, si la
/// herramienta no es de las filtradas, si la entrada es desmesurada, si pide `raw: true` o si no hay
/// nada que compactar. Con fallo (`is_error`) solo se aplican las reglas generales (R4), que son las
/// únicas que existen hoy.
pub fn apply(
    call_id: &str,
    tool: &str,
    input: &Value,
    out: &ToolOutput,
    cfg: &FilterConfig,
    spill: Option<&ToolSpill>,
) -> Filtered {
    if !cfg.enabled
        || !HERRAMIENTAS.contains(&tool)
        || input["raw"].as_bool() == Some(true)
        || out.content.len() > MAX_FILTER_INPUT_BYTES
    {
        return Filtered::intacto(&out.content);
    }
    let inicio = Instant::now();
    // `job_output` devuelve JSON: el texto a compactar es su campo `text`, y el resto se conserva.
    let valor: Option<Value> = if tool == "job_output" {
        match serde_json::from_str(&out.content) {
            Ok(valor) => Some(valor),
            Err(_) => return Filtered::intacto(&out.content),
        }
    } else {
        None
    };
    let texto: &str = match &valor {
        Some(valor) => match valor["text"].as_str() {
            Some(texto) => texto,
            None => return Filtered::intacto(&out.content),
        },
        None => &out.content,
    };
    let Some(compactado) = general::compactar(texto) else {
        return Filtered::intacto(&out.content);
    };
    if inicio.elapsed() > MAX_FILTER_TIME {
        return Filtered::intacto(&out.content);
    }
    // R2: el original íntegro se guarda antes de anunciar su ruta en la cabecera.
    // Sin original recuperable no se omite nada: una pérdida irreversible sería peor que el ruido.
    let Some(original) = spill.and_then(|spill| spill.save(call_id, &out.content)) else {
        return Filtered::intacto(&out.content);
    };
    let original = Some(original);
    let total = texto.lines().count();
    let cuerpo = format!(
        "{}\n{}",
        header::cabecera(compactado.omitidas, total, "general", original.as_deref()),
        compactado.texto
    );
    let content = match valor {
        Some(mut valor) => {
            valor["text"] = Value::String(cuerpo);
            match serde_json::to_string(&valor) {
                Ok(json) => json,
                Err(_) => return Filtered::intacto(&out.content),
            }
        }
        None => cuerpo,
    };
    Filtered {
        content,
        omitted_lines: compactado.omitidas,
        rule: Some("general"),
        original,
    }
}

/// Contenido que recibe el modelo para una salida de herramienta. Si el filtro no toca la salida,
/// devuelve `recurso` (el tratamiento de siempre). Si la filtra, la acota al tope de herramientas
/// conservando la cabecera, que ya lleva la ruta del original.
pub fn contenido_para_modelo(
    cfg: &FilterConfig,
    spill: Option<&ToolSpill>,
    call_id: &str,
    herramienta: Option<(&str, &Value)>,
    out: &ToolOutput,
    recurso: impl FnOnce() -> String,
) -> String {
    let Some((tool, input)) = herramienta else {
        return recurso();
    };
    let filtrada = apply(call_id, tool, input, out, cfg, spill);
    if filtrada.rule.is_none() {
        return recurso();
    }
    acotar(filtrada.content)
}

/// Recorta lo filtrado al tope de herramientas, si lo supera, por cabeza y cola.
fn acotar(contenido: String) -> String {
    if contenido.len() <= MAX_TOOL_OUTPUT_BYTES {
        return contenido;
    }
    limit_bytes_with_header(&contenido, MAX_TOOL_OUTPUT_BYTES - RESERVA_CABECERA).into_owned()
}
