//! Interacción con hijos vivos: tiempos de espera de la cancelación y prompt de los seguimientos.

use super::*;

/// Lo máximo que `cancel` espera a que el hijo se asiente tras la orden de parar.
pub(super) const CANCEL_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Cada cuánto `cancel` relee el estado del hijo mientras espera.
pub(super) const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// Prompt de un seguimiento (D4 de A11): el resultado de la tarea anterior, acotado por la regla
/// 8 del decálogo, antepuesto a la petición. Sin resultado se escribe `(no result)` y, si lo hay,
/// el texto del mensaje de su estado.
pub(super) fn prompt_de_seguimiento(anterior: &SubagentDescriptor, peticion: &str) -> String {
    let resultado = anterior.result_text().unwrap_or_else(|| {
        match anterior
            .status
            .message
            .as_ref()
            .map(|mensaje| mensaje.text())
            .filter(|texto| !texto.trim().is_empty())
        {
            Some(texto) => format!("(no result) {texto}"),
            None => "(no result)".to_string(),
        }
    });
    format!(
        "[Follow-up of subagent {} \"{}\" ({})]\nPrevious result:\n{}\n\n{}",
        anterior.child.0,
        anterior.label,
        anterior.status.state.proto_name(),
        crate::tools::limit_bytes(&resultado, crate::tools::MAX_NOTICE_BYTES),
        peticion
    )
}
