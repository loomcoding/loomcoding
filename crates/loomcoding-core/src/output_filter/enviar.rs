//! Lo que recibe el modelo tras filtrar una salida (R9): el contenido, el ahorro que se publica y las
//! reglas que este uso acaba de desactivar (Tarea 5). El transcript y el modelo ven lo mismo.

use super::stats;
use super::{filtrar, FilterConfig, HERRAMIENTAS};
use crate::tools::spill::ToolSpill;
use crate::tools::{limit_bytes_with_header, ToolOutput, MAX_TOOL_OUTPUT_BYTES};
use serde_json::Value;

/// Bytes que se reservan a la cabecera y al marcador de recorte al acotar (R1).
const RESERVA_CABECERA: usize = 512;

/// Ahorro de una salida filtrada: regla aplicada y bytes antes y después de filtrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ahorro {
    pub regla: &'static str,
    pub original_bytes: usize,
    pub sent_bytes: usize,
}

/// Contenido que va al modelo, con el ahorro si el filtro actuó y las reglas desactivadas por esta
/// llamada (para avisar una sola vez).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enviado {
    pub contenido: String,
    pub ahorro: Option<Ahorro>,
    pub desactivadas: Vec<&'static str>,
}

/// Contenido que recibe el modelo para una salida de herramienta. Si el filtro no toca la salida,
/// `recurso` es el tratamiento de siempre. Si la filtra, la acota al tope de herramientas
/// conservando la cabecera, que ya lleva la ruta del original.
pub fn contenido_para_modelo(
    cfg: &FilterConfig,
    spill: Option<&ToolSpill>,
    call_id: &str,
    herramienta: Option<(&str, &Value)>,
    out: &ToolOutput,
    recurso: impl FnOnce() -> String,
) -> Enviado {
    let Some((tool, input)) = herramienta else {
        return sin_filtrar(recurso(), Vec::new());
    };
    if !cfg.enabled || !HERRAMIENTAS.contains(&tool) {
        return sin_filtrar(recurso(), Vec::new());
    }
    let estadisticas = stats::global();
    let huella = stats::huella_de(input);
    let raw = input["raw"].as_bool() == Some(true);
    let mut desactivadas = estadisticas.observa_llamada(huella, raw);
    let filtrada = filtrar(call_id, tool, input, out, cfg, spill, &|regla| {
        estadisticas.desactivada(regla)
    });
    let Some(regla) = filtrada.rule else {
        return sin_filtrar(recurso(), desactivadas);
    };
    let contenido = acotar(filtrada.content);
    let ahorro = Ahorro {
        regla,
        original_bytes: out.content.len(),
        sent_bytes: contenido.len(),
    };
    desactivadas.extend(estadisticas.registra_uso(
        regla,
        huella,
        ahorro.original_bytes,
        ahorro.sent_bytes,
    ));
    Enviado {
        contenido,
        ahorro: Some(ahorro),
        desactivadas,
    }
}

fn sin_filtrar(contenido: String, desactivadas: Vec<&'static str>) -> Enviado {
    Enviado {
        contenido,
        ahorro: None,
        desactivadas,
    }
}

/// Recorta lo filtrado al tope de herramientas, si lo supera, por cabeza y cola.
fn acotar(contenido: String) -> String {
    if contenido.len() <= MAX_TOOL_OUTPUT_BYTES {
        return contenido;
    }
    limit_bytes_with_header(&contenido, MAX_TOOL_OUTPUT_BYTES - RESERVA_CABECERA).into_owned()
}
