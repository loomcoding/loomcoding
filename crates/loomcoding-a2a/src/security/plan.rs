//! Plan de autenticación del cliente: elección de alternativa, cumplimiento y aplicación.

use super::*;

/// Certificado y clave de cliente (PEM) para mTLS. Contiene una clave privada: no se puede mostrar.
#[derive(Clone)]
pub struct ClientIdentity(pub(super) Vec<u8>);

impl ClientIdentity {
    pub fn pem(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for ClientIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClientIdentity(..)")
    }
}

/// Lo que hay que añadir a cada petición para cumplir la alternativa elegida.
pub struct AuthPlan {
    pub(super) pasos: Vec<Paso>,
}

impl fmt::Debug for AuthPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AuthPlan({} pasos)", self.pasos.len())
    }
}

pub(super) fn valor_sensible(texto: &str) -> Option<HeaderValue> {
    let mut valor = HeaderValue::from_str(texto).ok()?;
    valor.set_sensitive(true);
    Some(valor)
}

impl AuthPlan {
    /// Sin autenticación: el remoto es abierto.
    pub fn open() -> AuthPlan {
        AuthPlan { pasos: Vec::new() }
    }

    /// `true` si no añade nada a las peticiones.
    pub fn is_open(&self) -> bool {
        self.pasos.is_empty()
    }

    /// Elige la primera alternativa cuyos esquemas se soportan todos y tienen credencial. Si
    /// ninguna sirve, devuelve el problema de la primera. `credentials` mapea esquema → nombre de
    /// variable; `env` lee una variable (inyectable para las pruebas).
    pub fn choose(
        seguridad: &CardSecurity,
        credentials: &BTreeMap<String, String>,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<AuthPlan, AuthProblem> {
        if seguridad.alternatives.is_empty() {
            return Ok(AuthPlan::open());
        }
        let mut primero = None;
        for alternativa in &seguridad.alternatives {
            match Self::cumplir(seguridad, alternativa, credentials, env) {
                Ok(plan) => return Ok(plan),
                Err(problema) => {
                    primero.get_or_insert(problema);
                }
            }
        }
        Err(primero.expect("hay al menos una alternativa"))
    }

    fn cumplir(
        seguridad: &CardSecurity,
        alternativa: &[String],
        credentials: &BTreeMap<String, String>,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<AuthPlan, AuthProblem> {
        let mut pasos = Vec::new();
        for nombre in alternativa {
            let tipo = seguridad
                .schemes
                .get(nombre)
                .cloned()
                .unwrap_or_else(|| no_soportado("esquema no definido en la tarjeta"));
            if let SchemeKind::Unsupported { what } = &tipo {
                return Err(AuthProblem::Unsupported {
                    scheme: nombre.clone(),
                    kind: what.clone(),
                });
            }
            let Some(variable) = credentials.get(nombre) else {
                return Err(AuthProblem::MissingCredential {
                    scheme: nombre.clone(),
                    env: None,
                });
            };
            let Some(secreto) = env(variable).and_then(Secret::new) else {
                return Err(AuthProblem::MissingCredential {
                    scheme: nombre.clone(),
                    env: Some(variable.clone()),
                });
            };
            let invalida = || AuthProblem::InvalidCredential {
                scheme: nombre.clone(),
                env: variable.clone(),
            };
            let paso = match tipo {
                SchemeKind::ApiKey {
                    location: KeyLocation::Header,
                    name,
                } => {
                    let cabecera =
                        HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalida())?;
                    Paso::Cabecera(
                        cabecera,
                        valor_sensible(secreto.expose().trim()).ok_or_else(invalida)?,
                    )
                }
                SchemeKind::ApiKey {
                    location: KeyLocation::Query,
                    name,
                } => Paso::Consulta(name, secreto),
                SchemeKind::HttpBearer => Paso::Cabecera(
                    AUTHORIZATION,
                    valor_sensible(&format!("Bearer {}", secreto.expose().trim()))
                        .ok_or_else(invalida)?,
                ),
                SchemeKind::HttpBasic => {
                    if !secreto.expose().contains(':') {
                        return Err(invalida());
                    }
                    Paso::Cabecera(
                        AUTHORIZATION,
                        valor_sensible(&format!("Basic {}", STANDARD.encode(secreto.expose())))
                            .ok_or_else(invalida)?,
                    )
                }
                SchemeKind::OAuth2ClientCredentials { token_url, scopes } => {
                    let origen =
                        Origen::Directo(url_permitida(&token_url).map_err(|_| invalida())?);
                    Paso::Token(Arc::new(
                        TokenSource::new(origen, &secreto, scopes).ok_or_else(invalida)?,
                    ))
                }
                SchemeKind::OpenIdConnect { discovery_url } => {
                    let origen =
                        Origen::Descubrir(url_permitida(&discovery_url).map_err(|_| invalida())?);
                    Paso::Token(Arc::new(
                        TokenSource::new(origen, &secreto, Vec::new()).ok_or_else(invalida)?,
                    ))
                }
                SchemeKind::Mtls => {
                    // La variable nombra un fichero PEM con el certificado y su clave privada.
                    let malo = |reason: &str| AuthProblem::BadIdentity {
                        scheme: nombre.clone(),
                        env: variable.clone(),
                        reason: reason.to_string(),
                    };
                    let pem = leer_pem(secreto.expose().trim()).map_err(|motivo| malo(&motivo))?;
                    reqwest::Identity::from_pem(&pem)
                        .map_err(|_| malo("no es un PEM con certificado y clave privada"))?;
                    Paso::Identidad(ClientIdentity(pem))
                }
                SchemeKind::Unsupported { .. } => unreachable!("descartado arriba"),
            };
            pasos.push(paso);
        }
        Ok(AuthPlan { pasos })
    }

    /// Plan para un remoto ya descargado: `security` es el JSON opaco que guardó la tarjeta,
    /// `requires_auth` si la tarjeta declaró requisitos. Si los declaró y no se pueden leer (por
    /// ejemplo, por pasarse del tope), el remoto no se usa: jamás se intenta sin credencial.
    /// El error ya está redactado para mostrarlo.
    pub fn for_card(
        security: Option<&str>,
        requires_auth: bool,
        credentials: &BTreeMap<String, String>,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<AuthPlan, String> {
        let seguridad = match security {
            Some(texto) => CardSecurity::from_json(texto)
                .ok_or_else(|| "la seguridad de la tarjeta no se pudo leer".to_string())?,
            None if requires_auth => {
                return Err(
                    "la seguridad de la tarjeta es demasiado grande para interpretarla".to_string(),
                )
            }
            None => CardSecurity::default(),
        };
        AuthPlan::choose(&seguridad, credentials, env).map_err(|problema| problema.to_string())
    }

    /// Añade la autenticación a una petición. Con un esquema de token puede pedirlo al servidor de
    /// tokens; si no lo consigue devuelve el motivo y la petición no debe enviarse sin credencial.
    pub async fn apply(&self, mut peticion: RequestBuilder) -> Result<RequestBuilder, String> {
        for paso in &self.pasos {
            peticion = match paso {
                Paso::Identidad(_) => peticion,
                Paso::Cabecera(nombre, valor) => peticion.header(nombre.clone(), valor.clone()),
                Paso::Consulta(nombre, secreto) => {
                    peticion.query(&[(nombre.as_str(), secreto.expose())])
                }
                Paso::Token(fuente) => peticion.header(AUTHORIZATION, fuente.bearer().await?),
            };
        }
        Ok(peticion)
    }

    /// Identidad TLS que el plan exige presentar, si la hay: obliga a un cliente HTTP propio.
    pub fn client_identity(&self) -> Option<&ClientIdentity> {
        self.pasos.iter().find_map(|p| match p {
            Paso::Identidad(identidad) => Some(identidad),
            _ => None,
        })
    }

    /// `true` si alguna parte del plan es un token que puede caducar o ser revocado.
    pub fn uses_token(&self) -> bool {
        self.pasos.iter().any(|p| matches!(p, Paso::Token(_)))
    }

    /// Olvida los tokens en caché: el remoto los rechazó y el siguiente uso pide otros.
    pub async fn invalidate(&self) {
        for paso in &self.pasos {
            if let Paso::Token(fuente) = paso {
                fuente.invalidate().await;
            }
        }
    }
}
