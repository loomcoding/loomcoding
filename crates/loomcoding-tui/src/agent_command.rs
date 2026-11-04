//! Análisis de la orden que lanza un agente por mandato directo de la persona usuaria.
//!
//! Dos sintaxis equivalentes: `/agent <nombre> [--skills a,b] <tarea>` y `@<nombre> [--skills a,b] <tarea>`.
//! Es una función pura que se llama al enviar, nunca por pulsación.

use serde::{Deserialize, Serialize};

/// Orden ya analizada: qué agente, con qué skills (si se eligieron) y qué tarea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInvocation {
    pub agent: String,
    /// Subconjunto elegido de las skills del perfil; `None` carga todas las que declara.
    pub skills: Option<Vec<String>>,
    pub prompt: String,
}

/// Uso de la orden, para los mensajes de error.
pub const USAGE: &str = "usage: /agent <name> [--skills a,b] <task>   (or @<name> …)";

/// Analiza `text`. `None` si no es una orden de agente (se envía como mensaje normal). `Some(Err)` si lo es
/// pero está mal formada, con un mensaje legible. `usable` son los nombres de agente que pueden lanzarse.
///
/// `@` solo cuenta al principio del texto y si el nombre es un agente usable: un `@fichero` cualquiera es un
/// mensaje normal. `/agent` siempre es una orden, y un nombre desconocido es un error.
pub fn parse_agent_command(
    text: &str,
    usable: &[String],
) -> Option<Result<AgentInvocation, String>> {
    let texto = text.trim_start();
    if let Some(resto) = texto.strip_prefix("/agent") {
        if resto.is_empty() || resto.starts_with(char::is_whitespace) {
            return Some(analizar(resto.trim_start(), usable, true));
        }
        return None;
    }
    let resto = texto.strip_prefix('@')?;
    let nombre = resto.split_whitespace().next()?;
    if !usable.iter().any(|agente| agente == nombre) {
        return None;
    }
    Some(analizar(resto, usable, false))
}

/// Completa con Tab el nombre del agente de `/agent <prefijo>` o `@<prefijo>`. Devuelve el texto nuevo, o
/// `None` si no hay nada que completar (otro texto, nombre ya completo y único, o ninguna coincidencia). Con
/// varios candidatos extiende solo hasta el prefijo común; con uno solo añade el nombre y un espacio.
/// Trabaja sobre la lista en memoria, sin leer disco.
pub fn complete_agent_name(text: &str, usable: &[String]) -> Option<String> {
    let (cabecera, prefijo) = if let Some(resto) = text.strip_prefix("/agent ") {
        ("/agent ", resto)
    } else {
        ("@", text.strip_prefix('@')?)
    };
    if prefijo.contains(char::is_whitespace) {
        return None;
    }
    let mut candidatos: Vec<&String> = usable
        .iter()
        .filter(|agente| agente.starts_with(prefijo))
        .collect();
    candidatos.sort();
    let primero = candidatos.first()?;
    let comun = candidatos.iter().skip(1).fold(primero.as_str(), |acc, c| {
        let largo = acc
            .chars()
            .zip(c.chars())
            .take_while(|(a, b)| a == b)
            .count();
        &acc[..acc.char_indices().nth(largo).map_or(acc.len(), |(i, _)| i)]
    });
    let nuevo = if candidatos.len() == 1 {
        format!("{cabecera}{primero} ")
    } else {
        format!("{cabecera}{comun}")
    };
    (nuevo != text).then_some(nuevo)
}

fn analizar(
    resto: &str,
    usable: &[String],
    exigir_conocido: bool,
) -> Result<AgentInvocation, String> {
    let mut partes = resto.splitn(2, char::is_whitespace);
    let nombre = partes.next().unwrap_or("").trim();
    if nombre.is_empty() {
        return Err(format!("missing agent name; {USAGE}"));
    }
    if exigir_conocido && !usable.iter().any(|agente| agente == nombre) {
        return Err(format!(
            "unknown agent «{nombre}»; usable: {}",
            if usable.is_empty() {
                "none".to_string()
            } else {
                usable.join(", ")
            }
        ));
    }
    let mut cola = partes.next().unwrap_or("").trim_start();
    let mut skills = None;
    if let Some(despues) = cola.strip_prefix("--skills") {
        if !despues.starts_with(char::is_whitespace) {
            return Err(format!("--skills needs a comma-separated list; {USAGE}"));
        }
        let despues = despues.trim_start();
        let (lista, tarea) = match despues.split_once(char::is_whitespace) {
            Some((lista, tarea)) => (lista, tarea),
            None => (despues, ""),
        };
        let ids: Vec<String> = lista.split(',').map(|id| id.trim().to_string()).collect();
        if lista.is_empty() || ids.iter().any(String::is_empty) {
            return Err(format!("--skills needs a comma-separated list; {USAGE}"));
        }
        skills = Some(ids);
        cola = tarea.trim_start();
    }
    let tarea = cola.trim();
    if tarea.is_empty() {
        return Err(format!("missing task; {USAGE}"));
    }
    Ok(AgentInvocation {
        agent: nombre.to_string(),
        skills,
        prompt: tarea.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agentes() -> Vec<String> {
        vec!["rust".to_string(), "superpowers".to_string()]
    }

    fn invocacion(agente: &str, skills: Option<&[&str]>, tarea: &str) -> AgentInvocation {
        AgentInvocation {
            agent: agente.into(),
            skills: skills.map(|ids| ids.iter().map(|id| id.to_string()).collect()),
            prompt: tarea.into(),
        }
    }

    #[test]
    fn both_syntaxes_produce_the_same_invocation() {
        let esperado = invocacion("rust", None, "revisa el manejo de errores");
        assert_eq!(
            parse_agent_command("/agent rust revisa el manejo de errores", &agentes()),
            Some(Ok(esperado.clone()))
        );
        assert_eq!(
            parse_agent_command("@rust revisa el manejo de errores", &agentes()),
            Some(Ok(esperado))
        );
    }

    #[test]
    fn skills_are_an_ordered_comma_list_before_the_task() {
        let esperado = invocacion(
            "superpowers",
            Some(&["brainstorming", "writing-plans"]),
            "diseña X con calma",
        );
        for texto in [
            "/agent superpowers --skills brainstorming,writing-plans diseña X con calma",
            "@superpowers --skills brainstorming,writing-plans diseña X con calma",
        ] {
            assert_eq!(
                parse_agent_command(texto, &agentes()),
                Some(Ok(esperado.clone())),
                "{texto}"
            );
        }
    }

    #[test]
    fn a_space_after_a_comma_in_the_skills_list_is_an_error_not_a_misparse() {
        let Some(Err(mensaje)) = parse_agent_command(
            "@superpowers --skills brainstorming, writing-plans diseña X",
            &agentes(),
        ) else {
            panic!("debía ser un error");
        };
        assert!(mensaje.contains("--skills needs"), "{mensaje}");
    }

    #[test]
    fn an_at_sign_that_is_not_an_agent_is_a_normal_message() {
        assert_eq!(
            parse_agent_command("@src/lib.rs mira esto", &agentes()),
            None
        );
        assert_eq!(parse_agent_command("@nadie hola", &agentes()), None);
        assert_eq!(parse_agent_command("hola @rust", &agentes()), None);
        assert_eq!(parse_agent_command("", &agentes()), None);
    }

    #[test]
    fn a_word_starting_with_agent_is_not_the_command() {
        assert_eq!(parse_agent_command("/agents", &agentes()), None);
        assert_eq!(parse_agent_command("/agentx rust hola", &agentes()), None);
    }

    #[test]
    fn slash_agent_errors_are_readable() {
        for (texto, trozo) in [
            ("/agent", "missing agent name"),
            ("/agent rust", "missing task"),
            ("/agent nadie hola", "unknown agent «nadie»"),
            ("/agent rust --skills", "--skills needs"),
            ("/agent rust --skills a,,b hola", "--skills needs"),
            ("/agent rust --skills a", "missing task"),
        ] {
            let Some(Err(mensaje)) = parse_agent_command(texto, &agentes()) else {
                panic!("{texto} debía ser un error");
            };
            assert!(mensaje.contains(trozo), "{texto}: {mensaje}");
        }
    }

    #[test]
    fn an_unknown_agent_error_lists_the_usable_ones() {
        let Some(Err(mensaje)) = parse_agent_command("/agent nadie hola", &agentes()) else {
            panic!("debía ser un error");
        };
        assert!(mensaje.contains("rust, superpowers"), "{mensaje}");
        let Some(Err(sin)) = parse_agent_command("/agent nadie hola", &[]) else {
            panic!("debía ser un error");
        };
        assert!(sin.contains("none"), "{sin}");
    }

    #[test]
    fn tab_completes_a_unique_prefix_and_adds_a_space() {
        assert_eq!(
            complete_agent_name("/agent su", &agentes()),
            Some("/agent superpowers ".to_string())
        );
        assert_eq!(
            complete_agent_name("@ru", &agentes()),
            Some("@rust ".to_string())
        );
    }

    #[test]
    fn tab_extends_to_the_common_prefix_when_several_match() {
        let varios = vec![
            "rust".to_string(),
            "rustfmt".to_string(),
            "ruby".to_string(),
        ];
        assert_eq!(complete_agent_name("@r", &varios), Some("@ru".to_string()));
        assert_eq!(complete_agent_name("@rust", &varios), None);
    }

    #[test]
    fn tab_does_nothing_for_other_text_or_after_the_name() {
        assert_eq!(complete_agent_name("hola", &agentes()), None);
        assert_eq!(complete_agent_name("/agent rust revisa", &agentes()), None);
        assert_eq!(complete_agent_name("@nadie", &agentes()), None);
        assert_eq!(complete_agent_name("@src/lib.rs", &agentes()), None);
    }

    #[test]
    fn extra_whitespace_and_a_leading_space_are_tolerated() {
        assert_eq!(
            parse_agent_command("  /agent   rust    haz   esto ", &agentes()),
            Some(Ok(invocacion("rust", None, "haz   esto")))
        );
    }
}
