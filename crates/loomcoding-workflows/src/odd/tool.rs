//! Herramienta `odd_track`: crear, marcar, reabrir, bloquear, registrar y leer documentos de feature.
//!
//! Orden de cada llamada: se valida la entrada; si es `done`, se comprueba la evidencia contra el
//! historial de la sesión; solo entonces se toca el disco, bajo el cerrojo de E/S, con escritura
//! atómica. Cualquier fallo antes de escribir deja el documento intacto.

use super::doc::FeatureDoc;
use super::evidence;
use super::ops;
use super::state::OddState;
use super::store;
use super::todos;
use async_trait::async_trait;
use loomcoding_core::error::Result;
use loomcoding_core::tools::{limit_output, ExecutionMode, Tool, ToolCtx, ToolOutput};
use serde_json::Value;
use std::path::Path;
use tokio_util::sync::CancellationToken;

const OPERACIONES: &str = "create, done, reopen, block, log, read";

pub struct OddTrackTool {
    estado: OddState,
}

impl OddTrackTool {
    pub fn new(estado: OddState) -> Self {
        Self { estado }
    }
}

#[async_trait]
impl Tool for OddTrackTool {
    fn name(&self) -> &str {
        "odd_track"
    }
    fn description(&self) -> &str {
        "Documento de feature (desarrollo orgánico). Operaciones (campo `op`): create \
         {feature, objective, tasks[]}; done {feature, id, evidence:{text, call_id}}; reopen \
         {feature, id, reason}; block {feature, id, reason}; log {feature, text}; read {feature}. \
         Marcar una tarea exige evidencia: el call_id de una llamada de bash, job_start o \
         job_output de esta sesión. El runtime comprueba que la llamada existe; no comprueba que \
         el texto sea cierto."
    }
    fn schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "op": {"type": "string", "enum": ["create", "done", "reopen", "block", "log", "read"]},
                "feature": {"type": "string", "description": "Solo a-z, 0-9, _ y -; hasta 64 caracteres"},
                "objective": {"type": "string"},
                "tasks": {"type": "array", "items": {"type": "string"}},
                "id": {"type": "string", "description": "Identificador de tarea, p. ej. T2"},
                "evidence": {
                    "type": "object",
                    "properties": {
                        "text": {"type": "string"},
                        "call_id": {"type": "string"}
                    }
                },
                "reason": {"type": "string"},
                "text": {"type": "string"}
            },
            "required": ["op", "feature"]
        })
    }
    fn execution_mode(&self, _input: &Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    /// Solo el agente principal gestiona el documento: los subagentes no ven `odd_track`.
    fn parent_only(&self) -> bool {
        true
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: Value,
        _cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let op = match Op::desde(&input) {
            Ok(op) => op,
            Err(error) => return Ok(ToolOutput::err(error)),
        };
        let feature = op.feature().to_string();
        if let Err(error) = store::validar_feature(&feature) {
            return Ok(ToolOutput::err(error));
        }
        // La evidencia se comprueba antes de tocar el disco: un rechazo no deja rastro.
        let op = match op.con_evidencia_comprobada(&ctx).await {
            Ok(op) => op,
            Err(error) => return Ok(ToolOutput::err(error)),
        };

        let ruta = store::ruta(&ctx.cwd, &feature);
        let estado = self.estado.clone();
        let trabajo = tokio::task::spawn_blocking(move || aplicar(&estado, &ruta, op));
        let resultado = match trabajo.await {
            Ok(resultado) => resultado,
            Err(error) => Err(format!("la operación de ODD no terminó: {error}")),
        };
        match resultado {
            Ok(aplicada) => {
                self.estado.recordar(ctx.session, &feature);
                if let Some(doc) = &aplicada.documento {
                    todos::publicar(&self.estado, &ctx, doc);
                }
                Ok(ToolOutput::ok(aplicada.texto))
            }
            Err(error) => Ok(ToolOutput::err(error)),
        }
    }
}

/// Operación ya validada en su forma, sin tocar aún el disco.
enum Op {
    Crear {
        feature: String,
        objetivo: String,
        tareas: Vec<String>,
    },
    Leer {
        feature: String,
    },
    Hecha {
        feature: String,
        id: String,
        texto: String,
        call_id: String,
    },
    Reabrir {
        feature: String,
        id: String,
        motivo: String,
    },
    Bloquear {
        feature: String,
        id: String,
        motivo: String,
    },
    Registrar {
        feature: String,
        texto: String,
    },
}

impl Op {
    /// Lee la operación de la entrada. Solo comprueba la forma; los textos se validan después.
    fn desde(input: &Value) -> std::result::Result<Op, String> {
        let op = campo_texto(input, "op")?;
        let feature = campo_texto(input, "feature")?;
        Ok(match op.as_str() {
            "create" => Op::Crear {
                feature,
                objetivo: campo_texto(input, "objective")?,
                tareas: campo_tareas(input)?,
            },
            "read" => Op::Leer { feature },
            "done" => {
                let evidencia = input.get("evidence").ok_or_else(|| {
                    "falta el campo `evidence`: marcar una tarea exige evidencia \
                         (`{text, call_id}`)"
                        .to_string()
                })?;
                if !evidencia.is_object() {
                    return Err("`evidence` debe ser un objeto con `text` y `call_id`".to_string());
                }
                Op::Hecha {
                    feature,
                    id: campo_texto(input, "id")?,
                    texto: campo_texto(evidencia, "text")?,
                    call_id: campo_texto(evidencia, "call_id")?,
                }
            }
            "reopen" => Op::Reabrir {
                feature,
                id: campo_texto(input, "id")?,
                motivo: campo_texto(input, "reason")?,
            },
            "block" => Op::Bloquear {
                feature,
                id: campo_texto(input, "id")?,
                motivo: campo_texto(input, "reason")?,
            },
            "log" => Op::Registrar {
                feature,
                texto: campo_texto(input, "text")?,
            },
            otra => {
                return Err(format!(
                    "operación `{}` desconocida; use una de: {OPERACIONES}",
                    loomcoding_core::tools::limit_bytes(otra, 32)
                ))
            }
        })
    }

    fn feature(&self) -> &str {
        match self {
            Op::Crear { feature, .. }
            | Op::Leer { feature }
            | Op::Hecha { feature, .. }
            | Op::Reabrir { feature, .. }
            | Op::Bloquear { feature, .. }
            | Op::Registrar { feature, .. } => feature,
        }
    }

    /// Para `done`, valida la evidencia y comprueba su llamada en el historial de la sesión.
    /// Sustituye el texto y el identificador por la forma guardada. Las demás operaciones pasan.
    async fn con_evidencia_comprobada(self, ctx: &ToolCtx) -> std::result::Result<Op, String> {
        match self {
            Op::Hecha {
                feature,
                id,
                texto,
                call_id,
            } => {
                let evidencia = ops::formatear_evidencia(&texto, &call_id)?;
                evidence::comprobar_llamada(ctx.store.as_ref(), ctx.session, call_id.trim())
                    .await?;
                Ok(Op::Hecha {
                    feature,
                    id,
                    texto: evidencia,
                    call_id,
                })
            }
            otra => Ok(otra),
        }
    }
}

/// Campo de texto obligatorio de un objeto JSON.
fn campo_texto(objeto: &Value, campo: &str) -> std::result::Result<String, String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => Err(format!("falta el campo `{campo}`")),
        Some(Value::String(texto)) => Ok(texto.clone()),
        Some(_) => Err(format!("el campo `{campo}` debe ser texto")),
    }
}

fn campo_tareas(objeto: &Value) -> std::result::Result<Vec<String>, String> {
    let lista = objeto
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| "`tasks` debe ser una lista de textos".to_string())?;
    lista
        .iter()
        .enumerate()
        .map(|(i, valor)| {
            valor
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("`tasks[{i}]` debe ser texto"))
        })
        .collect()
}

/// Resultado de una operación: el texto para el modelo y el documento releído tras guardarlo. Solo
/// las operaciones que modifican el documento lo traen, y solo si se puede releer.
struct Aplicada {
    texto: String,
    documento: Option<FeatureDoc>,
}

/// Lee, aplica y escribe bajo el cerrojo de E/S. Devuelve el texto para el modelo y el documento.
fn aplicar(estado: &OddState, ruta: &Path, op: Op) -> std::result::Result<Aplicada, String> {
    let _cerrojo = estado.bloqueo_io();
    let feature = op.feature().to_string();
    let existente = store::leer(ruta)?;

    if let Op::Crear {
        feature,
        objetivo,
        tareas,
    } = &op
    {
        if existente.is_some() {
            return Err(format!(
                "el documento de `{feature}` ya existe; use `read`, `done`, `log`, `block` o \
                 `reopen` para trabajar con él"
            ));
        }
        let doc = ops::crear(feature, objetivo, tareas)?;
        escribir(ruta, &doc)?;
        return Ok(Aplicada {
            texto: format!(
                "Documento `{feature}` creado con {} tareas.",
                doc.tasks.len()
            ),
            documento: releer(ruta),
        });
    }

    let markdown = existente
        .ok_or_else(|| format!("no existe el documento de `{feature}`; créelo con `create`"))?;
    let mut doc = FeatureDoc::parse(&markdown)
        .map_err(|error| format!("el documento de `{feature}` está roto ({error}). No lo toco"))?;

    let texto = match op {
        Op::Leer { .. } => {
            return Ok(Aplicada {
                texto: limit_output(&markdown).into_owned(),
                documento: None,
            })
        }
        Op::Hecha { id, texto, .. } => ops::marcar_hecha(&mut doc, &id, &texto)?,
        Op::Reabrir { id, motivo, .. } => ops::reabrir(&mut doc, &id, &motivo)?,
        Op::Bloquear { id, motivo, .. } => ops::bloquear(&mut doc, &id, &motivo)?,
        Op::Registrar { texto, .. } => ops::registrar(&mut doc, &texto)?,
        Op::Crear { .. } => unreachable!("create se trata arriba"),
    };
    escribir(ruta, &doc)?;
    Ok(Aplicada {
        texto,
        documento: releer(ruta),
    })
}

/// Escribe el documento solo si se puede releer tal cual, y de forma atómica.
fn escribir(ruta: &Path, doc: &FeatureDoc) -> std::result::Result<(), String> {
    ops::comprobar_ida_y_vuelta(doc)?;
    store::escribir_atomico(ruta, &doc.to_markdown())
}

/// Relee el documento ya guardado. `None` si no existe o no se puede parsear: entonces no se publica
/// nada y la operación sigue siendo válida.
pub(crate) fn releer(ruta: &Path) -> Option<FeatureDoc> {
    let markdown = store::leer(ruta).ok()??;
    FeatureDoc::parse(&markdown).ok()
}
