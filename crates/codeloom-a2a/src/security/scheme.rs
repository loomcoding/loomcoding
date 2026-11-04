//! Esquemas de seguridad de la tarjeta: lectura acotada, saneado y serialización.

use super::*;

pub(super) const MAX_ESQUEMAS: usize = 20;

pub(super) const MAX_ALTERNATIVAS: usize = 10;

pub(super) const MAX_POR_ALTERNATIVA: usize = 10;

pub(super) const MAX_NOMBRE: usize = 200;

/// Tope del JSON opaco que el núcleo guarda junto a la tarjeta.
pub(super) const MAX_JSON_BYTES: usize = 16 * 1024;

/// Dónde viaja una clave de API. La cookie no se admite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyLocation {
    Header,
    Query,
}

/// Tipo de un esquema de la tarjeta, reducido a lo que el cliente sabe hacer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SchemeKind {
    ApiKey {
        location: KeyLocation,
        name: String,
    },
    HttpBearer,
    HttpBasic,
    /// OAuth2 con el flujo `client_credentials`: la dirección del servidor de tokens y los ámbitos.
    OAuth2ClientCredentials {
        token_url: String,
        scopes: Vec<String>,
    },
    /// OpenID Connect: se descubre el servidor de tokens en el documento de esta dirección.
    OpenIdConnect {
        discovery_url: String,
    },
    /// mTLS: el cliente presenta un certificado propio en el apretón de manos TLS.
    Mtls,
    /// Nombre legible de lo que no se soporta («OAuth2», «mTLS»…).
    Unsupported {
        what: String,
    },
}

/// Lo que la tarjeta pide, ya acotado y saneado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardSecurity {
    pub(super) schemes: BTreeMap<String, SchemeKind>,
    /// Cada alternativa es un conjunto de nombres de esquema que se han de cumplir a la vez.
    pub(super) alternatives: Vec<Vec<String>>,
}

/// Quita saltos y caracteres de control (el texto es de un tercero y acaba en mensajes) y acota.
pub(super) fn limpiar(texto: &str) -> String {
    let plano: String = texto
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let plano = plano.trim();
    if plano.len() <= MAX_NOMBRE {
        return plano.to_string();
    }
    let mut fin = MAX_NOMBRE;
    while !plano.is_char_boundary(fin) {
        fin -= 1;
    }
    format!("{}…", &plano[..fin])
}

pub(super) fn no_soportado(que: &str) -> SchemeKind {
    SchemeKind::Unsupported {
        what: que.to_string(),
    }
}

/// Ámbitos de OAuth2 que se envían como mucho, y su longitud máxima.
pub(super) const MAX_AMBITOS: usize = 20;

pub(super) const MAX_LARGO_AMBITO: usize = 100;

/// Un ámbito válido según RFC 6749 §3.3: caracteres imprimibles sin espacio, comillas ni barra invertida.
pub(super) fn ambito_valido(ambito: &str) -> bool {
    !ambito.is_empty()
        && ambito.len() <= MAX_LARGO_AMBITO
        && ambito
            .bytes()
            .all(|b| (0x21..=0x7e).contains(&b) && b != b'"' && b != b'\\')
}

/// OAuth2: solo `client_credentials`; cualquier otro flujo exige una persona delante.
pub(super) fn leer_oauth2(cuerpo: &Value) -> SchemeKind {
    let Some(flujo) = cuerpo
        .get("flows")
        .and_then(|f| f.get("clientCredentials"))
        .filter(|f| f.is_object())
    else {
        return no_soportado("OAuth2 sin el flujo client_credentials");
    };
    let token_url = flujo.get("tokenUrl").and_then(Value::as_str).unwrap_or("");
    if let Err(motivo) = url_permitida(token_url) {
        return no_soportado(&format!("OAuth2 (tokenUrl: {motivo})"));
    }
    let scopes = flujo
        .get("scopes")
        .and_then(Value::as_object)
        .map(|mapa| {
            mapa.keys()
                .filter(|k| ambito_valido(k))
                .take(MAX_AMBITOS)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    SchemeKind::OAuth2ClientCredentials {
        token_url: token_url.to_string(),
        scopes,
    }
}

pub(super) fn leer_esquema(valor: &Value) -> SchemeKind {
    let Some(objeto) = valor.as_object() else {
        return no_soportado("esquema mal formado");
    };
    let Some((miembro, cuerpo)) = objeto.iter().next() else {
        return no_soportado("esquema vacío");
    };
    let texto = |clave: &str| cuerpo.get(clave).and_then(Value::as_str).unwrap_or("");
    match miembro.as_str() {
        "apiKeySecurityScheme" => {
            let nombre = limpiar(texto("name"));
            let ubicacion = texto("location").to_ascii_lowercase();
            match ubicacion.as_str() {
                "header" if HeaderName::from_bytes(nombre.as_bytes()).is_ok() => {
                    SchemeKind::ApiKey {
                        location: KeyLocation::Header,
                        name: nombre,
                    }
                }
                "query" if !nombre.is_empty() => SchemeKind::ApiKey {
                    location: KeyLocation::Query,
                    name: nombre,
                },
                "cookie" => no_soportado("clave de API en cookie"),
                _ => no_soportado("clave de API con ubicación o nombre inválidos"),
            }
        }
        "httpAuthSecurityScheme" => match texto("scheme").to_ascii_lowercase().as_str() {
            "bearer" => SchemeKind::HttpBearer,
            "basic" => SchemeKind::HttpBasic,
            otro => no_soportado(&format!("HTTP {}", limpiar(otro))),
        },
        "oauth2SecurityScheme" => leer_oauth2(cuerpo),
        "openIdConnectSecurityScheme" => match url_permitida(texto("openIdConnectUrl")) {
            Ok(_) => SchemeKind::OpenIdConnect {
                discovery_url: texto("openIdConnectUrl").to_string(),
            },
            Err(motivo) => no_soportado(&format!("OpenID Connect ({motivo})")),
        },
        "mtlsSecurityScheme" => SchemeKind::Mtls,
        _ => no_soportado("tipo de esquema desconocido"),
    }
}

impl CardSecurity {
    /// Lee `securitySchemes` y `securityRequirements` de la tarjeta. Tolerante: lo que no entiende lo
    /// marca como no soportado en lugar de callarlo. Acotado en número y en tamaño de nombre.
    pub fn from_card_json(tarjeta: &Value) -> CardSecurity {
        let schemes = tarjeta
            .get("securitySchemes")
            .and_then(Value::as_object)
            .map(|mapa| {
                mapa.iter()
                    .take(MAX_ESQUEMAS)
                    .map(|(nombre, esquema)| (limpiar(nombre), leer_esquema(esquema)))
                    .collect()
            })
            .unwrap_or_default();
        let alternatives = tarjeta
            .get("securityRequirements")
            .and_then(Value::as_array)
            .map(|lista| {
                lista
                    .iter()
                    .take(MAX_ALTERNATIVAS)
                    .map(|requisito| {
                        requisito
                            .get("schemes")
                            .and_then(Value::as_object)
                            .map(|mapa| {
                                mapa.keys()
                                    .take(MAX_POR_ALTERNATIVA)
                                    .map(|nombre| limpiar(nombre))
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        CardSecurity {
            schemes,
            alternatives,
        }
    }

    /// `true` si la tarjeta no pide nada: remoto abierto.
    pub fn is_empty(&self) -> bool {
        self.alternatives.is_empty()
    }

    /// Forma opaca que guarda el núcleo junto a la tarjeta. `None` si no pide nada o si pasaría del tope.
    pub fn to_json(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        serde_json::to_string(self)
            .ok()
            .filter(|texto| texto.len() <= MAX_JSON_BYTES)
    }

    pub fn from_json(texto: &str) -> Option<CardSecurity> {
        if texto.len() > MAX_JSON_BYTES {
            return None;
        }
        serde_json::from_str(texto).ok()
    }
}
