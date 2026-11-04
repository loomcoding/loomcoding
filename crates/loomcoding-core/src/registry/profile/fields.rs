//! Piezas de lectura: frontmatter, textos, enteros y nombre válido de agente.

use super::*;

/// Parte el texto (ya en LF) en frontmatter y cuerpo. El frontmatter va entre dos líneas
/// `---`; el cuerpo es todo lo que sigue a la segunda.
pub(super) fn separar_frontmatter(texto: &str) -> Result<(&str, &str), ProfileProblem> {
    let fin_primera = texto.find('\n').unwrap_or(texto.len());
    if texto[..fin_primera].trim_end() != "---" {
        return Err(ProfileProblem::BadFrontmatter(
            "el fichero debe empezar por una línea `---`".to_string(),
        ));
    }

    let resto_inicio = (fin_primera + 1).min(texto.len());
    let resto = &texto[resto_inicio..];
    let mut inicio_linea = 0;
    for linea in resto.split_inclusive('\n') {
        if linea.trim_end() == "---" {
            let frontmatter = &resto[..inicio_linea];
            let cuerpo = &resto[inicio_linea + linea.len()..];
            return Ok((frontmatter, cuerpo));
        }
        inicio_linea += linea.len();
    }
    Err(ProfileProblem::BadFrontmatter(
        "el frontmatter no se cierra con una línea `---`".to_string(),
    ))
}

/// Texto de un campo: admite cadenas y números (p. ej. `version: 1`).
pub(super) fn valor_texto(valor: &Value, clave: &str) -> Result<String, ProfileProblem> {
    match valor {
        Value::String(texto, _) => Ok(texto.clone()),
        Value::Number(numero, _) => Ok(numero.to_string()),
        _ => Err(ProfileProblem::BadFrontmatter(format!(
            "el campo `{clave}` debe ser texto"
        ))),
    }
}

/// Campo de texto opcional: ausente o `null` cuentan como no declarado.
pub(super) fn texto_opcional(
    mapa: &Mapping,
    clave: &str,
) -> Result<Option<String>, ProfileProblem> {
    match mapa.get(clave) {
        None | Some(Value::Null(_)) => Ok(None),
        Some(valor) => valor_texto(valor, clave).map(Some),
    }
}

/// Campo de texto obligatorio: ausente, `null` o vacío dan `MissingField`.
pub(super) fn texto_obligatorio(
    mapa: &Mapping,
    clave: &'static str,
) -> Result<String, ProfileProblem> {
    match texto_opcional(mapa, clave)? {
        Some(texto) if !texto.trim().is_empty() => Ok(texto),
        _ => Err(ProfileProblem::MissingField(clave)),
    }
}

/// Entero sin signo opcional, para `max_turns` y `max_depth`.
pub(super) fn entero_opcional(mapa: &Mapping, clave: &str) -> Result<Option<u32>, ProfileProblem> {
    match mapa.get(clave) {
        None | Some(Value::Null(_)) => Ok(None),
        Some(valor) => valor
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| {
                ProfileProblem::BadFrontmatter(format!("`{clave}` debe ser un entero no negativo"))
            }),
    }
}

/// `true` si `name` es un identificador de agente válido: solo `a-z`, `0-9` y `-`, no vacío,
/// sin `-` al principio ni al final, y de 64 caracteres como máximo.
pub fn is_valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
