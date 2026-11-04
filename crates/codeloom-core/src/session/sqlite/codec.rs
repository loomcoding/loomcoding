//! Codificación de las columnas que no son texto plano: enteros, mensajes de estado, artefactos
//! y referencias.

use crate::a2a::{self, Artifact};
use crate::error::{CoreError, Result};
use crate::session::SessionId;
use crate::task::TaskState;

/// Los tokens se guardan como INTEGER de SQLite (i64); un valor fuera de rango es un error.
pub(super) fn to_sql_int(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| CoreError::Store(format!("token count out of range: {value}")))
}

pub(super) fn from_sql_int(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| CoreError::Store(format!("negative token count: {value}")))
}

/// Serializa el mensaje de estado para la columna `status_message`: el JSON de un `Message`.
pub(super) fn encode_status_message(message: Option<&a2a::Message>) -> Result<Option<String>> {
    message
        .map(|message| serde_json::to_string(message).map_err(|e| CoreError::Store(e.to_string())))
        .transpose()
}

/// Lee la columna `status_message`. Lo escrito por esta versión es el JSON de un `Message`; una
/// fila anterior a A2A guarda texto plano, que se lee como mensaje del agente. El identificador
/// de esa lectura se deriva de la fila, para que dos lecturas den lo mismo.
pub(super) fn decode_status_message(raw: Option<String>, child: i64) -> Option<a2a::Message> {
    let raw = raw?;
    match serde_json::from_str::<a2a::Message>(&raw) {
        Ok(message) => Some(message),
        Err(_) => {
            let mut message = a2a::Message::agent_text(raw);
            message.message_id = format!("estado-{child}");
            Some(message)
        }
    }
}

/// Las referencias de una tarea se guardan como un arreglo JSON de ids (`[3,5]`); sin ninguna,
/// `NULL`, que es también lo que tiene una fila anterior a la columna.
pub(super) fn encode_references(references: &[SessionId]) -> Result<Option<String>> {
    if references.is_empty() {
        return Ok(None);
    }
    let ids: Vec<i64> = references.iter().map(|id| id.0).collect();
    serde_json::to_string(&ids)
        .map(Some)
        .map_err(|e| CoreError::Store(e.to_string()))
}

/// Lee las referencias. Un valor ilegible se descarta con un aviso y deja la lista vacía: un
/// enlace perdido no debe impedir listar los hijos.
pub(super) fn decode_references(json: Option<String>, child: i64) -> Vec<SessionId> {
    let Some(json) = json else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<i64>>(&json) {
        Ok(ids) => ids.into_iter().map(SessionId).collect(),
        Err(error) => {
            tracing::warn!(child, error = %error, "referencias de tarea ilegibles; se ignoran");
            Vec::new()
        }
    }
}

/// Serializa los artefactos del resultado para la columna `artifacts_json`.
pub(super) fn encode_artifacts(artifacts: &[Artifact]) -> Result<String> {
    serde_json::to_string(artifacts).map_err(|e| CoreError::Store(e.to_string()))
}

/// Lee los artefactos de una fila. Si `artifacts_json` es nulo y `result` no, la fila es
/// anterior a A2A y su resultado de texto es un único artefacto de texto.
pub(super) fn decode_artifacts(
    json: Option<String>,
    legacy_result: Option<String>,
    child: i64,
) -> Result<Vec<Artifact>> {
    match json {
        Some(json) => {
            serde_json::from_str(&json).map_err(|e| CoreError::Store(format!("artifacts: {e}")))
        }
        None => Ok(legacy_result
            .map(|texto| {
                let mut artefacto = Artifact::text_artifact(texto);
                artefacto.artifact_id = format!("resultado-{child}");
                vec![artefacto]
            })
            .unwrap_or_default()),
    }
}

/// Estado de una fila del almacén. Acepta los nombres del proto v1.0 y las cadenas de las
/// versiones anteriores, que se leen como su equivalente de A2A: `running` es `working`;
/// `killed`, `canceled`; `interrupted` y `unfinished`, `failed`.
pub(super) fn parse_task_state(status: &str) -> Result<TaskState> {
    match status {
        "running" => Ok(TaskState::Working),
        "completed" => Ok(TaskState::Completed),
        "failed" | "interrupted" | "unfinished" => Ok(TaskState::Failed),
        "killed" => Ok(TaskState::Canceled),
        _ => TaskState::from_proto_name(status)
            .ok_or_else(|| CoreError::Store(format!("unknown subagent status: {status}"))),
    }
}
