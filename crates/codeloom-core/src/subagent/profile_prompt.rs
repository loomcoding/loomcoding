//! Composición del prompt adicional de un hijo: persona del perfil y skills, con sus topes.

use super::*;

/// Tope del texto de persona del perfil dentro del prompt adicional del hijo. Las skills tienen el suyo.
pub(crate) const MAX_PROFILE_PROMPT_BYTES: usize = 16_000;

/// Holgura para el marcador que `limit_bytes` añade al recortar (`[... N bytes omitted ...]`).
pub(crate) const TRUNCATION_MARKER_SLACK: usize = 128;

/// Une la persona del perfil y las skills cargadas en el prompt adicional del hijo. Cada parte lleva su
/// tope propio: unas skills grandes no deben recortar la persona.
pub(crate) fn compose_child_system(persona: Option<String>, skills_prompt: &str) -> Option<String> {
    let mut sistema = persona
        .map(|texto| crate::tools::limit_bytes(&texto, MAX_PROFILE_PROMPT_BYTES).into_owned())
        .unwrap_or_default();
    if !skills_prompt.is_empty() {
        if !sistema.is_empty() {
            sistema.push_str("\n\n");
        }
        sistema.push_str(skills_prompt);
    }
    (!sistema.is_empty()).then_some(sistema)
}

/// Valida la selección de skills de una petición directa contra las que declara el perfil. Devuelve las
/// que se cargarán (`None`: todas las declaradas). Rechaza, antes de crear tarea, una selección vacía,
/// repetida, de más de `MAX_SKILLS_PER_AGENT`, con una skill que el perfil no declara, o hecha a un agente
/// que no declara ninguna o es remoto.
pub(super) fn elegir_skills(
    perfil: Option<&AgentEntry>,
    skills: Option<Vec<String>>,
) -> Result<Option<Vec<String>>> {
    let Some(pedidas) = skills else {
        return Ok(None);
    };
    let declaradas: &[String] = match perfil {
        Some(AgentEntry {
            execution: Execution::Local(spec),
            ..
        }) => &spec.scope.skills,
        _ => &[],
    };
    if declaradas.is_empty() {
        return Err(CoreError::Tool(
            "this agent declares no skills to choose from".into(),
        ));
    }
    if pedidas.is_empty() || pedidas.len() > crate::skills::MAX_SKILLS_PER_AGENT {
        return Err(CoreError::Tool(format!(
            "choose between 1 and {} skills",
            crate::skills::MAX_SKILLS_PER_AGENT
        )));
    }
    let mut vistas = std::collections::HashSet::new();
    for id in &pedidas {
        if !vistas.insert(id.as_str()) {
            return Err(CoreError::Tool(format!("skills: «{id}» is repeated")));
        }
        if !declaradas.contains(id) {
            return Err(CoreError::Tool(format!(
                "skills: «{id}» is not declared by the agent"
            )));
        }
    }
    Ok(Some(pedidas))
}
