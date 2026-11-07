//! Compresión determinista de la salida de las herramientas `bash` y `job_output`.
//!
//! Sin modelo y sin red. Cada llamada se filtra por separado; el único estado compartido es la
//! medición del proceso (`stats`), que solo guarda cifras y nombres de regla. Lo que se omite queda
//! anunciado con una cabecera (R1) y el original íntegro se conserva con `ToolSpill` (R2). Solo
//! herramientas: nunca la respuesta de un subagente ni de una tarea A2A. Véase
//! `docs/specs/2026-11-06-loomcoding-compresion-de-salida.md`.

mod cargo;
#[cfg(test)]
mod cargo_tests;
mod comando_simple;
mod enviar;
mod general;
mod git;
#[cfg(test)]
mod git_tests;
mod header;
mod packages;
#[cfg(test)]
mod packages_tests;
pub mod stats;
#[cfg(test)]
mod stats_tests;
#[cfg(test)]
mod tests;

pub use enviar::{contenido_para_modelo, Ahorro, Enviado};

use crate::error::{CoreError, Result};
use crate::tools::spill::ToolSpill;
use crate::tools::ToolOutput;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Entrada por encima de la cual la salida pasa sin filtrar (R8).
pub const MAX_FILTER_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Tiempo máximo de un filtro; si lo supera, la salida pasa sin filtrar (R8).
#[cfg(not(test))]
pub const MAX_FILTER_TIME: Duration = Duration::from_millis(50);

/// En las pruebas el reloj no manda: una compilación de depuración con la máquina cargada superaría
/// los 50 ms y volvería intermitentes las pruebas de las reglas. El límite por tamaño sí se prueba.
#[cfg(test)]
pub const MAX_FILTER_TIME: Duration = Duration::from_secs(30);

/// Variable de entorno que apaga el filtro en todo el proceso (R10).
pub const ENV_DESACTIVA: &str = "LOOMCODING_NO_FILTER";

/// Herramientas cuya salida pasa por el filtro. Nunca lectura, búsqueda ni subagentes.
const HERRAMIENTAS: [&str; 2] = ["bash", "job_output"];

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
/// nada que compactar. Con fallo (`is_error`) se aplica la misma lista blanca (R4): la regla por
/// comando no descarta ninguna línea de error ni el contexto que la rodea.
pub fn apply(
    call_id: &str,
    tool: &str,
    input: &Value,
    out: &ToolOutput,
    cfg: &FilterConfig,
    spill: Option<&ToolSpill>,
) -> Filtered {
    filtrar(call_id, tool, input, out, cfg, spill, &|_| false)
}

/// Como `apply`, pero con un veto por regla: si `desactivada(regla)` es cierto, la salida pasa intacta.
pub fn filtrar(
    call_id: &str,
    tool: &str,
    input: &Value,
    out: &ToolOutput,
    cfg: &FilterConfig,
    spill: Option<&ToolSpill>,
    desactivada: &dyn Fn(&str) -> bool,
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
    let Some((compactado, regla)) = compactar_con_reglas(texto, tool, input) else {
        return Filtered::intacto(&out.content);
    };
    // Medición (Tarea 5): una regla desactivada sola pasa intacta, sin volcado ni cabecera.
    if desactivada(regla) {
        return Filtered::intacto(&out.content);
    }
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
        header::cabecera(compactado.omitidas, total, regla, original.as_deref()),
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
        rule: Some(regla),
        original,
    }
}

/// Regla por comando: nombre, decisión sobre el comando y compactación de la salida.
type Regla = (
    &'static str,
    fn(&str) -> bool,
    fn(&str) -> Option<general::Compactado>,
);

/// Reglas por comando, en orden de prioridad (R3, R6).
const REGLAS: [Regla; 3] = [
    ("cargo", cargo::aplica, cargo::compactar),
    ("git", git::aplica, git::compactar),
    ("packages", packages::aplica, packages::compactar),
];

/// Aplica las reglas generales y, si el comando tiene regla propia, también esa (R3). Devuelve el
/// texto compactado con las líneas omitidas y el nombre de la regla que lo ha hecho.
fn compactar_con_reglas(
    texto: &str,
    tool: &str,
    input: &Value,
) -> Option<(general::Compactado, &'static str)> {
    let generales = general::compactar(texto);
    let omitidas_generales = generales.as_ref().map_or(0, |g| g.omitidas);
    let sobre = generales.as_ref().map_or(texto, |g| g.texto.as_str());
    let comando = if tool == "bash" {
        input["command"].as_str()
    } else {
        None
    };
    // La primera regla que reconoce el ejecutable y el subcomando decide. Si no encuentra nada que
    // compactar, pasan solo las generales (R6).
    let regla = comando.and_then(|comando| REGLAS.iter().find(|regla| (regla.1)(comando)));
    if let Some(&(nombre, _, compactar)) = regla {
        if let Some(especifico) = compactar(sobre) {
            let compactado = general::Compactado {
                texto: especifico.texto,
                omitidas: omitidas_generales + especifico.omitidas,
            };
            return Some((compactado, nombre));
        }
    }
    generales.map(|g| (g, "general"))
}
