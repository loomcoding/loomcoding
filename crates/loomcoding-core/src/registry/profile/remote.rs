//! Perfil remoto: endpoint y credenciales declaradas por nombre de variable.

use super::*;

/// Claves que se leen y se declaran, pero que no se aplican todavía.
pub(super) const CLAVES_NO_APLICADAS: &[&str] = &["scope", "delegation", "transport", "a2a"];

/// Valor de `transport` que declara un agente remoto.
pub(super) const TRANSPORTE_REMOTO: &str = "a2a-remote";

/// `a2a.endpoint` de un perfil remoto. `None` si el perfil no es remoto; si lo es y falta el
/// endpoint o no es una URL admisible (HTTPS, o HTTP solo en bucle local), el perfil no carga.
pub(super) fn endpoint_remoto(mapa: &Mapping) -> Result<Option<String>, ProfileProblem> {
    if texto_opcional(mapa, "transport")?.as_deref() != Some(TRANSPORTE_REMOTO) {
        return Ok(None);
    }
    let endpoint = match mapa.get("a2a") {
        Some(a2a) => match a2a.as_mapping() {
            Some(a2a) => texto_opcional(a2a, "endpoint")?,
            None => {
                return Err(ProfileProblem::BadFrontmatter(
                    "`a2a` debe ser un mapa".to_string(),
                ))
            }
        },
        None => None,
    };
    let Some(endpoint) = endpoint
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
    else {
        return Err(ProfileProblem::MissingField("a2a.endpoint"));
    };
    crate::config::validate_base_url(&endpoint).map_err(|motivo| {
        ProfileProblem::BadFrontmatter(format!("`a2a.endpoint` no es admisible: {motivo}"))
    })?;
    Ok(Some(endpoint))
}

/// Máximo de credenciales declaradas en un perfil.
pub(super) const MAX_CREDENCIALES: usize = 20;

/// Máximo de bytes de un nombre de esquema o de variable.
pub(super) const MAX_NOMBRE_CREDENCIAL: usize = 128;

/// Nombre de variable de entorno admisible: `[A-Za-z_][A-Za-z0-9_]*`.
pub(super) fn es_nombre_de_variable(texto: &str) -> bool {
    let mut caracteres = texto.chars();
    texto.len() <= MAX_NOMBRE_CREDENCIAL
        && caracteres
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && caracteres.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `a2a.credentials` de un perfil remoto. El valor ha de ser el **nombre** de una variable de
/// entorno: si parece un secreto escrito a mano, el perfil no carga y el mensaje no lo repite.
pub(super) fn credenciales_remotas(
    mapa: &Mapping,
) -> Result<BTreeMap<String, String>, ProfileProblem> {
    let Some(a2a) = mapa.get("a2a").and_then(Value::as_mapping) else {
        return Ok(BTreeMap::new());
    };
    let credenciales = match a2a.get("credentials") {
        None | Some(Value::Null(_)) => return Ok(BTreeMap::new()),
        Some(valor) => valor.as_mapping().ok_or_else(|| {
            ProfileProblem::BadFrontmatter("`a2a.credentials` debe ser un mapa".to_string())
        })?,
    };
    if credenciales.len() > MAX_CREDENCIALES {
        return Err(ProfileProblem::BadFrontmatter(format!(
            "`a2a.credentials` admite como mucho {MAX_CREDENCIALES} entradas"
        )));
    }
    let mut resultado = BTreeMap::new();
    for (esquema, variable) in credenciales.iter() {
        let esquema = esquema.as_str().filter(|e| !e.is_empty()).ok_or_else(|| {
            ProfileProblem::BadFrontmatter(
                "`a2a.credentials`: el nombre del esquema debe ser texto no vacío".to_string(),
            )
        })?;
        let variable =
            variable
                .as_str()
                .map(str::trim)
                .filter(|v| es_nombre_de_variable(v))
                .ok_or_else(|| {
                    ProfileProblem::BadFrontmatter(format!(
                    "`a2a.credentials`: el valor de «{}» debe ser el nombre de una variable de \
                     entorno, no el secreto",
                    esquema.chars().take(MAX_NOMBRE_CREDENCIAL).collect::<String>()
                ))
                })?;
        resultado.insert(esquema.to_string(), variable.to_string());
    }
    Ok(resultado)
}
