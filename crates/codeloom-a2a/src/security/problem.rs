//! Problemas de autenticación y pasos del cumplimiento.

use super::*;

/// Por qué no se puede construir un plan de autenticación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthProblem {
    Unsupported {
        scheme: String,
        kind: String,
    },
    MissingCredential {
        scheme: String,
        env: Option<String>,
    },
    InvalidCredential {
        scheme: String,
        env: String,
    },
    /// La identidad TLS (mTLS) que nombra la variable no se pudo usar.
    BadIdentity {
        scheme: String,
        env: String,
        reason: String,
    },
}

impl fmt::Display for AuthProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthProblem::Unsupported { scheme, kind } => write!(
                f,
                "el esquema «{scheme}» ({kind}) exige autenticación que aún no está soportada"
            ),
            AuthProblem::MissingCredential {
                scheme,
                env: Some(env),
            } => write!(
                f,
                "falta la variable de entorno {env} (credencial del esquema «{scheme}»)"
            ),
            AuthProblem::MissingCredential { scheme, env: None } => write!(
                f,
                "el esquema «{scheme}» necesita una credencial: declárala en `a2a.credentials` del perfil"
            ),
            AuthProblem::InvalidCredential { scheme, env } => write!(
                f,
                "la credencial del esquema «{scheme}» (variable {env}) no es válida para ese esquema"
            ),
            AuthProblem::BadIdentity {
                scheme,
                env,
                reason,
            } => write!(
                f,
                "la identidad TLS del esquema «{scheme}» (variable {env}) no se puede usar: {reason}"
            ),
        }
    }
}

pub(super) enum Paso {
    /// Identidad de cliente TLS: no añade nada a la petición, pero obliga a un cliente HTTP propio.
    Identidad(ClientIdentity),
    Cabecera(HeaderName, HeaderValue),
    Consulta(String, Secret),
    /// Token de acceso que se pide (y se renueva) al servidor de tokens.
    Token(Arc<TokenSource>),
}
