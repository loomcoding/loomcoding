//! Seguridad de la tarjeta de un remoto y plan de autenticación del cliente (A8d).
//!
//! Cabe señalar que A2A no prescribe un modelo de autorización: la tarjeta solo **declara** lo que
//! pide (`securitySchemes`, `securityRequirements`). Aquí se interpreta esa declaración, se elige la
//! primera alternativa que este cliente sabe cumplir y se construye lo que hay que añadir a cada
//! petición. Lo que no se sabe cumplir (flujos de OAuth2 con persona, mTLS, clave en cookie…) se
//! rechaza con un motivo legible; jamás se intenta sin credencial. OAuth2 `client_credentials` y
//! OpenID Connect se cumplen pidiendo un token (módulo `oauth`).
//!
//! La lectura de `securityRequirements` como «alternativas (O) de conjuntos (Y)» es la semántica de
//! OpenAPI, no una frase de A2A: es lectura nuestra y así consta en la spec.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use reqwest::header::{HeaderName, HeaderValue, AUTHORIZATION};
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::oauth::{url_permitida, Origen, TokenSource};
use crate::remote::leer_pem;

mod plan;
mod problem;
mod scheme;
mod secret;

pub use plan::{AuthPlan, ClientIdentity};
pub use problem::AuthProblem;
use problem::*;
use scheme::*;
pub use scheme::{CardSecurity, KeyLocation, SchemeKind};
pub use secret::Secret;

#[cfg(test)]
mod tests;
