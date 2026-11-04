//! El enrutador del servidor (A8b): tarjeta pública y punto JSON-RPC autenticado.
//!
//! Cabe señalar que el orden de las comprobaciones importa y es deliberado: primero la
//! autenticación (sin credencial no se lee ni el cuerpo), luego el tope del cuerpo, después el
//! JSON-RPC y la versión, y solo al final el método. Los errores de JSON-RPC viajan con HTTP 200,
//! como es habitual; solo 401, 413 y 503 usan el estado HTTP.
//!
//! Lo que el servidor sabe hacer con una tarea lo abstrae [`TaskHost`], de modo que el enrutador
//! se prueba en memoria con un anfitrión falso, sin abrir ningún puerto.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde_json::{json, Value};

use super::auth::Authenticator;
use super::card::ServedCard;
use super::config::ServeAuthScheme;
use crate::card::VERSION_SOPORTADA;
use crate::wire::{WireMessage, WireRole};

mod card;
mod contract;
mod methods;
mod reply;

use card::*;
use contract::*;
pub use contract::{
    EventStream, HostError, ServeState, TaskHost, TaskView, MAX_BODY_BYTES, MAX_PROMPT_BYTES,
};
use methods::*;
use reply::*;

#[cfg(test)]
mod tests;

/// Enrutador completo: tarjeta pública en `/.well-known/agent-card.json` y JSON-RPC en `/a2a`.
pub fn router(estado: Arc<ServeState>) -> Router {
    Router::new()
        .route("/.well-known/agent-card.json", get(tarjeta))
        .route(
            "/agents/{tenant}/.well-known/agent-card.json",
            get(tarjeta_agente),
        )
        .route("/a2a", post(rpc))
        .with_state(estado)
}
