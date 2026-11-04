use crate::message::{ContentPart, Message, Role};

/// Cuántas tool calls idénticas y consecutivas hacen falta para considerar que el
/// agente está en un bucle.
///
/// Cabe señalar que el umbral es alto a propósito: releer el mismo fichero con los
/// mismos argumentos es una operación legítima —pasa cada vez que una salida llega
/// truncada y hay que volver a mirar—, y un agente que explora repite lecturas sin
/// que eso sea un problema. El corte existe para el caso peligroso, que es repetir
/// una **escritura**: eso sí es un bucle que no avanza.
const REPEAT_THRESHOLD: usize = 6;

/// Herramientas cuyo bucle es peligroso: escriben o lanzan trabajo.
///
/// Las de solo lectura (`view`, `grep`, `glob`, `ls`) quedan fuera a propósito: un
/// agente que relee no está atascado, está mirando. Cortarle ahí mataba el turno en
/// mitad de una tarea normal y el subagente se quedaba sin escribir nada.
const WRITE_TOOLS: [&str; 9] = [
    "write",
    "edit",
    "multiedit",
    "apply_patch",
    "bash",
    "job_start",
    "subagent",
    "development_workflow",
    "gauntlet",
];

/// Por qué se detuvo el turno.
///
/// Existe para que quien recibe el corte pueda decir la verdad: antes todo se
/// reportaba como "sin respuesta final tras N pasos", que sonaba a presupuesto
/// agotado incluso cuando el motivo era otro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Se agotó `max_steps`.
    StepBudget,
    /// El agente repitió la misma tool call de escritura demasiadas veces seguidas.
    RepeatedToolCalls,
    /// Se agotó el tope de rondas de solo lectura.
    ReadBudget,
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

/// Si la herramienta de ese nombre escribe (y por tanto su ronda cuenta como trabajo).
pub fn is_write_tool(name: &str) -> bool {
    WRITE_TOOLS.contains(&name)
}

/// Si la firma corresponde a una herramienta que escribe.
///
/// La firma tiene la forma `nombre:json`, así que basta con mirar el prefijo.
fn es_escritura(sig: &[String]) -> bool {
    sig.iter().any(|call| {
        let nombre = call.split(':').next().unwrap_or("");
        WRITE_TOOLS.contains(&nombre)
    })
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
    if sigs.len() < REPEAT_THRESHOLD {
        return None;
    }
    let ultima = &sigs[sigs.len() - 1];
    let repetidas = sigs[sigs.len() - REPEAT_THRESHOLD..]
        .iter()
        .all(|s| s == ultima);
    if repetidas && es_escritura(ultima) {
        return Some(StopReason::RepeatedToolCalls);
    }
    None
}

/// Rondas de lectura seguidas, sin escribir, tras las cuales se avisa al modelo de que explora sin avanzar.
pub const STUCK_EXPLORATION_ROUNDS: usize = 10;

/// Cuántas rondas de solo lectura van seguidas al final del historial.
///
/// Cuenta hacia atrás las rondas del asistente con llamadas a herramientas y ninguna de escritura; se
/// detiene en un mensaje de usuario (también un aviso anterior, que rearma la cuenta) y en cualquier ronda
/// que escriba o que no llame a herramientas.
pub fn exploration_streak(history: &[Message]) -> usize {
    let mut streak = 0;
    for message in history.iter().rev() {
        match message.role {
            Role::Tool => continue,
            Role::Assistant => match signature(message) {
                Some(sig) if !es_escritura(&sig) => streak += 1,
                _ => break,
            },
            _ => break,
        }
    }
    streak
}

/// Para si se superó `max_steps` o si se repite una escritura dentro del turno actual.
pub fn should_stop(history: &[Message], steps: usize, max_steps: usize) -> bool {
    stop_reason(history, steps, max_steps).is_some()
}

/// Si el mensaje es una ronda de solo lectura: asistente, con al menos una llamada a herramienta,
/// y ninguna de esas llamadas es de escritura.
pub fn is_read_round(message: &Message) -> bool {
    message.role == Role::Assistant
        && !message.parts.is_empty()
        && message
            .parts
            .iter()
            .any(|p| matches!(p, ContentPart::ToolCall { .. }))
        && message.parts.iter().all(|p| {
            if let ContentPart::ToolCall { name, .. } = p {
                !WRITE_TOOLS.contains(&name.as_str())
            } else {
                true
            }
        })
}

/// Motivo por el que hay que parar, si lo hay, considerando también el presupuesto de lecturas.
pub fn stop_reason_budgets(
    history: &[Message],
    steps: usize,
    max_steps: usize,
    read_steps: usize,
    max_read_steps: usize,
) -> Option<StopReason> {
    if read_steps >= max_read_steps {
        return Some(StopReason::ReadBudget);
    }
    stop_reason(history, steps, max_steps)
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
        // Releer el mismo fichero es legítimo: es lo que hace un agente cuando la
        // salida le llega truncada. Antes esto mataba el turno.
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"a"})),
        ];
        assert_eq!(stop_reason(&h, 1, 10), None);
    }

    #[test]
    fn muchas_lecturas_iguales_tampoco_cortan() {
        // Un agente que explora puede releer diez veces sin estar atascado: leer no
        // cambia el mundo, así que no hay nada que proteger.
        let h: Vec<Message> = (0..10)
            .map(|_| with_call("view", json!({"path":"a"})))
            .collect();
        assert_eq!(
            stop_reason(&h, 1, 100),
            None,
            "un bucle de lecturas es inofensivo y no debe cortar el turno"
        );
    }

    #[test]
    fn stops_on_repeated_writes() {
        // Repetir la misma escritura sí es un bucle que no avanza.
        let h: Vec<Message> = (0..REPEAT_THRESHOLD)
            .map(|_| with_call("write", json!({"path":"a","content":"x"})))
            .collect();
        assert_eq!(
            stop_reason(&h, 1, 100),
            Some(StopReason::RepeatedToolCalls),
            "repetir la misma escritura sí debe cortar"
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
            with_call("write", json!({"path":"a"})),
            with_call("write", json!({"path":"b"})),
            with_call("write", json!({"path":"a"})),
        ];
        assert_eq!(stop_reason(&h, 1, 10), None);
    }

    #[test]
    fn un_mensaje_de_usuario_rearma_la_deteccion() {
        let mut h: Vec<Message> = (0..REPEAT_THRESHOLD)
            .map(|_| with_call("write", json!({"path":"a"})))
            .collect();
        h.push(Message::user("otra cosa"));
        assert_eq!(
            stop_reason(&h, 1, 100),
            None,
            "un turno nuevo no hereda el bucle del anterior"
        );
    }

    #[test]
    fn exploration_streak_counts_trailing_read_rounds() {
        let h = vec![
            Message::user("hola"),
            with_call("view", json!({"path":"a"})),
            with_call("grep", json!({"pattern":"x"})),
            with_call("view", json!({"path":"b"})),
        ];
        assert_eq!(exploration_streak(&h), 3);
    }

    #[test]
    fn exploration_streak_stops_at_a_write_round() {
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("edit", json!({"path":"a"})),
            with_call("view", json!({"path":"b"})),
        ];
        assert_eq!(exploration_streak(&h), 1);
    }

    #[test]
    fn exploration_streak_is_rearmed_by_a_user_message() {
        let h = vec![
            with_call("view", json!({"path":"a"})),
            with_call("view", json!({"path":"b"})),
            Message::user("[aviso]"),
            with_call("view", json!({"path":"c"})),
        ];
        assert_eq!(exploration_streak(&h), 1);
    }

    #[test]
    fn exploration_streak_ignores_tool_results_between_rounds() {
        let mut h = vec![with_call("view", json!({"path":"a"}))];
        h.push(Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "x".into(),
                content: "ok".into(),
                is_error: false,
            }],
        ));
        h.push(with_call("view", json!({"path":"b"})));
        assert_eq!(exploration_streak(&h), 2);
    }

    fn with_calls(names: &[&str]) -> Message {
        Message::assistant(
            names
                .iter()
                .map(|name| ContentPart::ToolCall {
                    id: "x".into(),
                    name: (*name).into(),
                    input: json!({}),
                })
                .collect(),
        )
    }

    #[test]
    fn a_round_of_only_reads_is_a_read_round() {
        assert!(is_read_round(&with_calls(&["view", "grep"])));
    }

    #[test]
    fn a_round_with_a_write_is_not_a_read_round() {
        assert!(!is_read_round(&with_calls(&["view", "edit"])));
    }

    #[test]
    fn a_round_with_bash_is_not_a_read_round() {
        let h = [with_call("bash", json!({"command":"ls"}))];
        assert!(!is_read_round(&h[0]));
    }

    #[test]
    fn a_message_without_tool_calls_is_not_a_read_round() {
        let h = [Message::assistant(vec![])];
        assert!(!is_read_round(&h[0]));
    }

    #[test]
    fn read_budget_stops_at_the_cap() {
        assert_eq!(
            stop_reason_budgets(&[], 0, 30, 5, 5),
            Some(StopReason::ReadBudget)
        );
    }

    #[test]
    fn work_budget_still_wins_first() {
        assert_eq!(
            stop_reason_budgets(&[], 30, 30, 0, 100),
            Some(StopReason::StepBudget)
        );
    }
}
