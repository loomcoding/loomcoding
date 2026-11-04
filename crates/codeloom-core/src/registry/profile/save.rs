//! Guardado de la edición de un perfil.

use super::*;

/// Valida y actualiza los campos del editor, conservando las demás claves YAML y el cuerpo.
/// La huella evita pisar cambios externos hechos mientras el editor estaba abierto.
pub fn save_profile_edit(
    path: &Path,
    expected_hash: &str,
    edit: &AgentProfileEdit,
) -> Result<String, ProfileSaveError> {
    let original = fs::read_to_string(path)?;
    if crate::registry::trust::content_hash(&original) != expected_hash {
        return Err(ProfileSaveError::Changed);
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ProfileProblem::InvalidName(String::new()))?;
    parse_profile(stem, &original)?;
    if edit.description.trim().is_empty() || edit.description.len() > MAX_CARD_TEXT {
        return Err(ProfileProblem::BadFrontmatter(
            "la descripción A2A debe tener entre 1 y 2000 bytes".into(),
        )
        .into());
    }
    if edit
        .version
        .as_ref()
        .is_some_and(|version| version.trim().is_empty() || version.len() > 128)
    {
        return Err(ProfileProblem::BadFrontmatter("la versión no es válida".into()).into());
    }

    let normalized = original.replace("\r\n", "\n");
    let (frontmatter, body) = separar_frontmatter(&normalized)?;
    let mut root: Value = serde_yaml_bw::from_str(frontmatter)
        .map_err(|error| ProfileProblem::BadFrontmatter(error.to_string()))?;
    let map = root
        .as_mapping_mut()
        .ok_or_else(|| ProfileProblem::BadFrontmatter("el frontmatter debe ser un mapa".into()))?;
    map.insert(
        Value::from("description"),
        Value::from(edit.description.trim()),
    );
    match edit
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(version) => {
            map.insert(Value::from("version"), Value::from(version));
        }
        None => {
            map.remove(Value::from("version"));
        }
    }

    if edit.scope == AgentScope::default() {
        map.remove(Value::from("scope"));
    } else {
        map.insert(
            Value::from("scope"),
            serde_yaml_bw::to_value(&edit.scope)
                .map_err(|error| ProfileProblem::BadFrontmatter(error.to_string()))?,
        );
    }
    let a2a = mapping_entry(map, "a2a")?;
    let card = mapping_entry(a2a, "card")?;
    card.insert(
        Value::from("skills"),
        serde_yaml_bw::to_value(&edit.card_skills)
            .map_err(|error| ProfileProblem::BadFrontmatter(error.to_string()))?,
    );

    let yaml = serde_yaml_bw::to_string(&root)
        .map_err(|error| ProfileProblem::BadFrontmatter(error.to_string()))?;
    let updated = format!("---\n{}---\n{body}", yaml.trim_end().to_string() + "\n");
    parse_profile(stem, &updated)?;

    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("agent.md");
    let temporary = path.with_file_name(format!(".{filename}.{}.tmp", std::process::id()));
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(updated.as_bytes())?;
        file.sync_all()?;
        file.set_permissions(fs::metadata(path)?.permissions())?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(crate::registry::trust::content_hash(&updated))
}
