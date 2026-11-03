use crate::message::{ContentPart, Message, Role};

/// Cuántas tool calls idénticas y consecutivas hacen falta para considerar que el
/// agente está en un bucle.
///
/// Cabe señalar que el umbral es 3 y no 2 a propósito: releer el mismo fichero con
/// los mismos argumentos dos veces seguidas es una operación legítima —pasa cada vez
/// que una salida llega truncada y hay que volver a mirar—, mientras que tres
/// seguidas ya no aportan información nueva. Con el umbral en 2 el turno moría en
/// mitad de una tarea normal, y el subagente se quedaba sin escribir nada.
const REPEAT_THRESHOLD: usize = 3;

/// Por qué se detuvo el turno.
///
/// Existe para que quien recibe el corte pueda decir la verdad: antes todo se
/// reportaba como "sin respuesta final tras N pasos", que sonaba a presupuesto
/// agotado incluso cuando el motivo era otro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Se agotó `max_steps`.
    StepBudget,
    /// El agente repitió la misma tool call demasiadas veces seguidas.
    RepeatedToolCalls,
}

fn signature(m: &Message) -> Option<Vec<String>> {
    let calls: Vec<String> = m
        .parts
        .iter()
        .filter_map(|p| match p {
            ContentPart::ToolCall { name, input, .. } => Some(format!("{name}:{input}")),
            _ => None,
        })
        .collect();
    if calls.is_empty() {
        None
    } else {
        Some(calls)
    }
}

/// Motivo por el que hay que parar, si lo hay.
///
/// Solo mira el turno actual: un mensaje de usuario rearma la detección, de modo que
/// una repetición de una interacción anterior no impide empezar la siguiente.
pub fn stop_reason(history: &[Message], steps: usize, max_steps: usize) -> Option<StopReason> {
    if steps >= max_steps {
        return Some(StopReason::StepBudget);
    }
    let turno = history
        .iter()
        .rposition(|message| message.role == Role::User)
        .unwrap_or(0);
    let sigs: Vec<Vec<String>> = history[turno..]
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .filter_map(signature)
        .collect();
    if sigs.len() >= REPEAT_THRESHOLD {
        let ultima = &sigs[sigs.len() - 1];
        let repetidas = sigs[sigs.len() - REPEAT_THRESHOLD..]
            .iter()
            .all(|s| s == ultima);
        if repetidas {
            return Some(StopReason::RepeatedToolCalls);
        }
    }
    None
}

/// Para si se superó `max_steps` o si se repiten tool calls dentro del turno actual.
pub fn should_stop(history: &[Message], steps: usize, max_steps: usize) -> bool {
    stop_reason(history, steps, max_steps).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn with_call(name: &str, input: serde_json::Value) -> Message {
        Message::assistant(vec![ContentPart::ToolCall {
            id: "x".into(),
            name: name.into(),
            input,
        }])
    }

    #[test]
    fn stops_on_step_budget() {
        assert_eq!(
            stop_reason(&[], 5, 5),
            Some(StopReason::StepBudget),
            "agotar el presupuesto debe reportarse como tal"
        );
    }

    #[test]
    fn dos_lecturas_iguales_no_cortan() {
        // Releer el mismo fichero dos veces es legítimo: es lo que hace un agente
        // cuando la salida le llega truncada. Antes esto mataba el turno.
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"a"})),
        ];
        assert_eq!(stop_reason(&h, 1, 10), None);
    }

    #[test]
    fn stops_on_repeated_tool_calls() {
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"a"})),
        ];
        assert_eq!(
            stop_reason(&h, 1, 10),
            Some(StopReason::RepeatedToolCalls),
            "tres repeticiones seguidas sí son un bucle"
        );
    }

    #[test]
    fn continues_when_calls_differ() {
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"b"})),
        ];
        assert!(!should_stop(&h, 1, 10));
    }

    #[test]
    fn repeticion_no_consecutiva_no_corta() {
        // a, b, a: no es un bucle, es ir y volver.
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"b"})),
            with_call("view", json!({"path":"a"})),
        ];
        assert_eq!(stop_reason(&h, 1, 10), None);
    }

    #[test]
    fn un_mensaje_de_usuario_rearma_la_deteccion() {
        let mut h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"a"})),
        ];
        h.push(Message::user("otra cosa"));
        assert_eq!(
            stop_reason(&h, 1, 10),
            None,
            "un turno nuevo no hereda el bucle del anterior"
        );
    }
}
