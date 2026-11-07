//! Limpieza de resultados de herramienta antiguos en la petición al proveedor.
//!
//! Solo afecta a la copia que se envía al modelo: el historial guardado y el almacén no se tocan.
//!
//! La limpieza respeta la frontera del turno: los resultados del turno en curso se envían
//! íntegros, porque son justo los que el modelo necesita para razonar. Solo se podan los de
//! turnos ya cerrados; el recorte por presupuesto lo hace `compact`.

use crate::message::{ContentPart, Message, MessageOrigin, Role};

/// Número de resultados de herramienta de turnos cerrados que se conservan íntegros.
pub const KEEP_TOOL_RESULTS: usize = 6;

/// Por debajo de este tamaño (en bytes) un resultado antiguo no compensa sustituirlo.
const CLEAR_MIN_BYTES: usize = 2_048;

/// Índice del mensaje que abre el turno en curso.
///
/// Se prefiere el último mensaje humano; si no hay ninguno (turnos internos, subagentes), se usa
/// el último mensaje de usuario. Devuelve `None` si no hay frontera reconocible: en ese caso se
/// aplica la poda por número de resultados, como antes.
fn turn_start_index(history: &[Message]) -> Option<usize> {
    history
        .iter()
        .rposition(|m| m.role == Role::User && m.origin == MessageOrigin::Human)
        .or_else(|| history.iter().rposition(|m| m.role == Role::User))
}

/// Devuelve una copia del historial en la que los resultados de herramienta de turnos cerrados
/// anteriores a los `keep` más recientes, y de más de `CLEAR_MIN_BYTES` bytes, se sustituyen por
/// un marcador.
///
/// Los resultados del turno en curso (posteriores al último mensaje de usuario) nunca se tocan.
/// Se conservan el orden, el número de partes, los `call_id` y el indicador de error.
pub fn clear_old_tool_results(history: &[Message], keep: usize) -> Vec<Message> {
    // Si no hay frontera de turno reconocible, no se protege nada por turno: se poda por número.
    let turn_start = turn_start_index(history).unwrap_or(history.len());

    let continuation_calls: std::collections::HashSet<&str> = history
        .iter()
        .filter(|message| message.provider_state.is_some())
        .flat_map(|message| message.tool_calls().into_iter().map(|(id, _, _)| id))
        .collect();
    // Solo cuentan los resultados de turnos cerrados: `keep` acota esa poda, no el turno vivo.
    let total = history[..turn_start]
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter(|p| matches!(p, ContentPart::ToolResult { .. }))
        .count();
    // Los resultados con índice inferior a `cutoff`, contando desde el primero, son los antiguos.
    let cutoff = total.saturating_sub(keep);

    let mut ordinal = 0usize;
    let mut out = Vec::with_capacity(history.len());
    for (message_index, message) in history.iter().enumerate() {
        // Dentro del turno en curso no se poda nada: el modelo lo está usando ahora mismo.
        let in_current_turn = message_index >= turn_start;
        let mut parts = Vec::with_capacity(message.parts.len());
        for part in &message.parts {
            match part {
                ContentPart::ToolResult {
                    call_id,
                    content,
                    is_error,
                } => {
                    let index = ordinal;
                    ordinal += 1;
                    if !in_current_turn
                        && index < cutoff
                        && content.len() > CLEAR_MIN_BYTES
                        && !continuation_calls.contains(call_id.as_str())
                    {
                        parts.push(ContentPart::ToolResult {
                            call_id: call_id.clone(),
                            content: format!(
                                "[tool result cleared: {} bytes; run the tool again if you need it]",
                                content.len()
                            ),
                            is_error: *is_error,
                        });
                    } else {
                        parts.push(part.clone());
                    }
                }
                _ => parts.push(part.clone()),
            }
        }
        out.push(Message {
            id: message.id,
            role: message.role,
            origin: message.origin.clone(),
            parts,
            usage: message.usage.clone(),
            provider_state: message.provider_state.clone(),
            created_at_ms: message.created_at_ms,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Role;

    fn result(call_id: &str, content: String, is_error: bool) -> ContentPart {
        ContentPart::ToolResult {
            call_id: call_id.into(),
            content,
            is_error,
        }
    }

    fn result_msg(call_id: &str, bytes: usize) -> Message {
        Message::new(Role::Tool, vec![result(call_id, "x".repeat(bytes), false)])
    }

    /// Tamaño de resultado que supera `CLEAR_MIN_BYTES` y por tanto es podable.
    const BIG: usize = 3_000;

    fn contents(messages: &[Message]) -> Vec<String> {
        messages
            .iter()
            .flat_map(|m| m.parts.iter())
            .filter_map(|p| match p {
                ContentPart::ToolResult { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect()
    }

    fn marker(bytes: usize) -> String {
        format!(
            "[tool result cleared: {} bytes; run the tool again if you need it]",
            bytes
        )
    }

    #[test]
    fn clears_only_results_older_than_the_last_six() {
        let history: Vec<Message> = (0..10).map(|i| result_msg(&format!("c{i}"), BIG)).collect();

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);
        let got = contents(&out);

        assert_eq!(got.len(), 10);
        for content in &got[..4] {
            assert_eq!(content, &marker(BIG));
        }
        for content in &got[4..] {
            assert_eq!(content.len(), BIG);
            assert!(content.chars().all(|c| c == 'x'));
        }
    }

    #[test]
    fn results_of_the_current_turn_are_never_cleared() {
        // Un turno largo: el humano pide algo y el agente encadena muchas herramientas.
        let mut history = vec![Message::user("refactoriza el módulo")];
        for i in 0..12 {
            history.push(result_msg(&format!("c{i}"), BIG));
        }

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);
        let got = contents(&out);

        // Ningún resultado del turno en curso se sustituye, por muchos que haya.
        assert_eq!(got.len(), 12);
        for content in &got {
            assert_eq!(content.len(), BIG, "el turno en curso debe llegar íntegro");
        }
    }

    #[test]
    fn internal_notices_do_not_move_the_turn_boundary() {
        // Un aviso de job llega a mitad de turno como `Message::user` con origen `Internal`.
        // No debe abrir un turno nuevo: si lo hiciera, se podaría trabajo aún en curso.
        let mut history = vec![Message::human("refactoriza el módulo")];
        for i in 0..4 {
            history.push(result_msg(&format!("c{i}"), BIG));
        }
        history.push(Message::user("[Job 7 finished] ok"));
        for i in 4..8 {
            history.push(result_msg(&format!("c{i}"), BIG));
        }

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);
        let got = contents(&out);

        // El turno sigue abierto: los ocho resultados llegan íntegros.
        assert_eq!(got.len(), 8);
        for content in &got {
            assert_eq!(content.len(), BIG, "un aviso interno no cierra el turno");
        }
    }

    #[test]
    fn closed_turns_are_cleared_but_the_current_one_is_not() {
        let mut history = vec![Message::user("primer turno")];
        for i in 0..8 {
            history.push(result_msg(&format!("old{i}"), BIG));
        }
        history.push(Message::user("segundo turno"));
        for i in 0..3 {
            history.push(result_msg(&format!("new{i}"), BIG));
        }

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);
        let got = contents(&out);

        // Los dos más antiguos del turno cerrado se podan (8 - 6 = 2).
        assert_eq!(got[0], marker(BIG));
        assert_eq!(got[1], marker(BIG));
        // El resto del turno cerrado y todo el turno en curso se conservan.
        for content in &got[2..] {
            assert_eq!(content.len(), BIG);
        }
    }

    #[test]
    fn small_old_results_are_kept() {
        let history = vec![
            result_msg("c0", CLEAR_MIN_BYTES),
            result_msg("c1", CLEAR_MIN_BYTES + 1),
            result_msg("c2", BIG),
            result_msg("c3", BIG),
            result_msg("c4", BIG),
            result_msg("c5", BIG),
            result_msg("c6", BIG),
            result_msg("c7", BIG),
        ];

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);
        let got = contents(&out);

        // Justo en el umbral no se toca: solo se poda lo que lo supera.
        assert_eq!(got[0].len(), CLEAR_MIN_BYTES);
        // Un byte por encima del umbral sí se sustituye.
        assert_eq!(got[1], marker(CLEAR_MIN_BYTES + 1));
    }

    #[test]
    fn call_ids_and_order_are_preserved() {
        let history = vec![
            Message::user("hola"),
            Message::new(
                Role::Tool,
                vec![
                    result("c0", "x".repeat(BIG), true),
                    result("c1", "y".repeat(BIG), false),
                ],
            ),
            Message::assistant(vec![
                ContentPart::Text {
                    text: "t".repeat(1000),
                },
                ContentPart::ToolCall {
                    id: "c2".into(),
                    name: "noop".into(),
                    input: serde_json::json!({}),
                },
            ]),
            Message::new(Role::Tool, vec![result("c2", "z".repeat(BIG), false)]),
            result_msg("c3", BIG),
            result_msg("c4", BIG),
            result_msg("c5", BIG),
            result_msg("c6", BIG),
            result_msg("c7", BIG),
            // Cierra el turno: los resultados anteriores ya son historial, no trabajo en curso.
            Message::user("siguiente"),
        ];

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);

        assert_eq!(out.len(), history.len());
        for (before, after) in history.iter().zip(out.iter()) {
            assert_eq!(before.role, after.role);
            assert_eq!(before.parts.len(), after.parts.len());
            for (p_before, p_after) in before.parts.iter().zip(after.parts.iter()) {
                match (p_before, p_after) {
                    (
                        ContentPart::ToolResult {
                            call_id: a,
                            is_error: ea,
                            ..
                        },
                        ContentPart::ToolResult {
                            call_id: b,
                            is_error: eb,
                            ..
                        },
                    ) => {
                        assert_eq!(a, b);
                        assert_eq!(ea, eb);
                    }
                    (ContentPart::ToolResult { .. }, _) | (_, ContentPart::ToolResult { .. }) => {
                        panic!("un resultado no puede cambiar de tipo")
                    }
                    _ => assert_eq!(p_before, p_after),
                }
            }
        }
        // El resultado c0 (antiguo, error) se ha limpiado y conserva su indicador de error.
        match &out[1].parts[0] {
            ContentPart::ToolResult {
                call_id,
                content,
                is_error,
            } => {
                assert_eq!(call_id, "c0");
                assert_eq!(content, &marker(BIG));
                assert!(*is_error);
            }
            otra => panic!("parte inesperada: {otra:?}"),
        }
    }

    #[test]
    fn original_history_is_not_modified() {
        let history: Vec<Message> = (0..10).map(|i| result_msg(&format!("c{i}"), BIG)).collect();
        let snapshot = history.clone();

        let out = clear_old_tool_results(&history, KEEP_TOOL_RESULTS);

        assert_eq!(history, snapshot);
        assert_ne!(
            out, history,
            "la copia debe diferir: hay resultados que limpiar"
        );
    }

    #[test]
    fn pending_provider_continuation_keeps_its_tool_results_intact() {
        use crate::message::ProviderState;
        let mut assistant = Message::assistant(vec![ContentPart::ToolCall {
            id: "pending-call".into(),
            name: "view".into(),
            input: serde_json::json!({}),
        }]);
        assistant.provider_state = Some(ProviderState {
            provider: "openai_responses".into(),
            data: serde_json::json!({}),
        });
        let history = vec![
            assistant,
            result_msg("pending-call", BIG),
            result_msg("unrelated", BIG),
        ];
        let cleared = clear_old_tool_results(&history, 0);
        let result = |id: &str| {
            cleared
                .iter()
                .flat_map(|message| message.parts.iter())
                .find_map(|part| match part {
                    ContentPart::ToolResult {
                        call_id, content, ..
                    } if call_id == id => Some(content),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(result("pending-call").len(), BIG);
        assert!(result("unrelated").starts_with("[tool result cleared:"));
    }
}
