//! Lectura de un perfil local: campos conocidos, ámbito, habilidades A2A y proveedor.

use super::*;

/// Lee un perfil. `file_stem` es el nombre del fichero sin extensión; el `name` del
/// frontmatter debe coincidir con él.
///
/// `Err` = el perfil no se puede cargar. Los avisos (campos desconocidos, campos declarados
/// y no aplicados) van en el `Vec`. El texto se normaliza a LF antes de leerlo.
pub fn parse_profile(
    file_stem: &str,
    text: &str,
) -> Result<(AgentProfile, Vec<ProfileProblem>), ProfileProblem> {
    if !is_valid_agent_name(file_stem) {
        return Err(ProfileProblem::InvalidName(file_stem.to_string()));
    }

    // Se quita el BOM, si lo hay, y se normalizan los finales de línea antes de partir.
    let normalizado = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n");
    let (frontmatter, cuerpo) = separar_frontmatter(&normalizado)?;

    let valor: Value = serde_yaml_bw::from_str(frontmatter)
        .map_err(|error| ProfileProblem::BadFrontmatter(error.to_string()))?;
    let Some(mapa) = valor.as_mapping() else {
        return Err(ProfileProblem::BadFrontmatter(
            "el frontmatter debe ser un mapa YAML".to_string(),
        ));
    };

    let remote_endpoint = endpoint_remoto(mapa)?;
    let remote_credentials = if remote_endpoint.is_some() {
        credenciales_remotas(mapa)?
    } else {
        BTreeMap::new()
    };

    // Las claves desconocidas avisan; las declaradas-y-no-aplicadas avisan y se anotan.
    let mut avisos = Vec::new();
    let mut declared_not_applied = Vec::new();
    for (clave, _) in mapa.iter() {
        let Some(clave) = clave.as_str() else {
            return Err(ProfileProblem::BadFrontmatter(
                "clave no textual en el frontmatter".to_string(),
            ));
        };
        if clave == "scope" {
            declared_not_applied.push(clave.to_string());
            avisos.push(ProfileProblem::DeclaredNotApplied(clave.to_string()));
            continue;
        }
        // El perfil local puede declarar `a2a.card`; las interfaces se derivan del runtime.
        if clave == "a2a" {
            continue;
        }
        if CLAVES_CONOCIDAS.contains(&clave) {
            continue;
        }
        // Un perfil remoto aplica `transport` y `a2a`: ya no son «declarados sin aplicar».
        if remote_endpoint.is_some() && (clave == "transport" || clave == "a2a") {
            continue;
        }
        if CLAVES_NO_APLICADAS.contains(&clave) {
            declared_not_applied.push(clave.to_string());
            avisos.push(ProfileProblem::DeclaredNotApplied(clave.to_string()));
        } else {
            avisos.push(ProfileProblem::UnknownField(clave.to_string()));
        }
    }

    let nombre = texto_obligatorio(mapa, "name")?;
    if !is_valid_agent_name(&nombre) {
        return Err(ProfileProblem::InvalidName(nombre));
    }
    if nombre != file_stem {
        return Err(ProfileProblem::NameMismatch {
            file: file_stem.to_string(),
            declared: nombre,
        });
    }
    let description = texto_obligatorio(mapa, "description")?;

    let (persona_role, persona_goal, persona_backstory) = match mapa.get("persona") {
        None | Some(Value::Null(_)) => (None, None, None),
        Some(persona) => {
            let Some(persona) = persona.as_mapping() else {
                return Err(ProfileProblem::BadFrontmatter(
                    "`persona` debe ser un mapa".to_string(),
                ));
            };
            for (clave, _) in persona.iter() {
                match clave.as_str() {
                    Some("role" | "goal" | "backstory") => {}
                    Some(otra) => {
                        avisos.push(ProfileProblem::UnknownField(format!("persona.{otra}")))
                    }
                    None => {
                        return Err(ProfileProblem::BadFrontmatter(
                            "clave de `persona` no textual".to_string(),
                        ))
                    }
                }
            }
            (
                texto_opcional(persona, "role")?,
                texto_opcional(persona, "goal")?,
                texto_opcional(persona, "backstory")?,
            )
        }
    };

    let provider = proveedor_local(mapa)?;
    if remote_endpoint.is_some() && provider.is_some() {
        return Err(ProfileProblem::InvalidProvider);
    }
    // `max_depth` son los niveles del subárbol que el agente encabeza, contándose a sí mismo: el techo
    // `[agents] max_depth` puede recortarlo al arrancar. Solo `0` carece de sentido y se avisa.
    let max_depth = entero_opcional(mapa, "max_depth")?;
    if max_depth == Some(0) {
        declared_not_applied.push("max_depth".to_string());
        avisos.push(ProfileProblem::DeclaredNotApplied("max_depth".to_string()));
    }
    let perfil = AgentProfile {
        name: nombre,
        description,
        version: texto_opcional(mapa, "version")?,
        persona_role,
        persona_goal,
        persona_backstory,
        body: cuerpo.trim().to_string(),
        model: texto_opcional(mapa, "model")?,
        provider,
        card_skills: habilidades_a2a(mapa)?,
        scope: scope_local(mapa)?,
        effort: texto_opcional(mapa, "effort")?,
        max_turns: entero_opcional(mapa, "max_turns")?,
        max_depth,
        declared_not_applied,
        remote_endpoint,
        remote_credentials,
    };
    Ok((perfil, avisos))
}

/// Claves que la carga interpreta hoy.
pub(super) const CLAVES_CONOCIDAS: &[&str] = &[
    "name",
    "description",
    "version",
    "persona",
    "model",
    "provider",
    "effort",
    "max_turns",
    "max_depth",
];

pub(super) const MAX_PROFILE_LIST: usize = 100;

pub(super) const MAX_CARD_SKILLS: usize = 50;

pub(super) const MAX_CARD_TEXT: usize = 2_000;

pub(super) fn scope_local(mapa: &Mapping) -> Result<AgentScope, ProfileProblem> {
    let Some(valor) = mapa.get("scope") else {
        return Ok(AgentScope::default());
    };
    let scope: AgentScope = serde_yaml_bw::from_value(valor.clone()).map_err(|_| {
        ProfileProblem::BadFrontmatter("`scope` debe ser un mapa de listas de texto".into())
    })?;
    for lista in [&scope.mcp, &scope.skills, &scope.tools] {
        if lista.len() > MAX_PROFILE_LIST
            || lista
                .iter()
                .any(|item| item.trim().is_empty() || item.len() > 128)
        {
            return Err(ProfileProblem::BadFrontmatter(
                "`scope` contiene una lista o identificador no válido".into(),
            ));
        }
    }
    if scope.mcp.iter().any(|id| {
        id.is_empty()
            || id.len() > 16
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    }) {
        return Err(ProfileProblem::BadFrontmatter(
            "`scope.mcp` usa IDs de servidor de 1-16 letras minúsculas, números o guiones".into(),
        ));
    }
    Ok(scope)
}

pub(super) fn habilidades_a2a(mapa: &Mapping) -> Result<Vec<AgentSkill>, ProfileProblem> {
    let Some(skills) = mapa
        .get("a2a")
        .and_then(Value::as_mapping)
        .and_then(|a2a| a2a.get("card"))
        .and_then(Value::as_mapping)
        .and_then(|card| card.get("skills"))
    else {
        return Ok(Vec::new());
    };
    let skills: Vec<AgentSkill> = serde_yaml_bw::from_value(skills.clone()).map_err(|_| {
        ProfileProblem::BadFrontmatter("`a2a.card.skills` debe ser una lista de skills A2A".into())
    })?;
    if skills.len() > MAX_CARD_SKILLS
        || skills.iter().any(|skill| {
            skill.id.trim().is_empty()
                || skill.name.trim().is_empty()
                || [
                    skill.id.as_str(),
                    skill.name.as_str(),
                    skill.description.as_str(),
                ]
                .iter()
                .any(|value| value.len() > MAX_CARD_TEXT)
                || skill.tags.iter().any(|tag| tag.len() > 128)
        })
    {
        return Err(ProfileProblem::BadFrontmatter(
            "`a2a.card.skills` excede los límites o contiene una skill sin id/nombre".into(),
        ));
    }
    Ok(skills)
}

pub(super) fn mapping_entry<'a>(
    map: &'a mut Mapping,
    key: &str,
) -> Result<&'a mut Mapping, ProfileProblem> {
    let key = Value::from(key);
    if !map.contains_key(&key) {
        map.insert(key.clone(), Value::Mapping(Mapping::new()));
    }
    map.get_mut(&key)
        .and_then(Value::as_mapping_mut)
        .ok_or_else(|| ProfileProblem::BadFrontmatter("`a2a.card` debe ser un mapa".into()))
}

pub(super) fn proveedor_local(mapa: &Mapping) -> Result<Option<ProviderConfig>, ProfileProblem> {
    let Some(value) = mapa.get("provider") else {
        return Ok(None);
    };
    let provider: ProviderConfig =
        serde_yaml_bw::from_value(value.clone()).map_err(|_| ProfileProblem::InvalidProvider)?;
    crate::config::validate_base_url(&provider.base_url)
        .map_err(|_| ProfileProblem::InvalidProvider)?;
    if !es_nombre_de_variable(&provider.api_key_env) {
        return Err(ProfileProblem::InvalidProvider);
    }
    Ok(Some(provider))
}
