//! Errores HTTP de Anthropic y reconocimiento del rechazo de llamadas programáticas.

use crate::capability::{classify_rejection, Capability};

/// Cabe señalar que solo un `400` con una marca inequívoca de PTC autoriza el fallback.
pub(super) fn is_openrouter_ptc_rejection(status: reqwest::StatusCode, body: &str) -> bool {
    status == reqwest::StatusCode::BAD_REQUEST
        && classify_rejection(status.as_u16(), body, Capability::ProgrammaticToolCalling)
            .allows_fallback()
}

pub(super) fn http_error(status: reqwest::StatusCode, body: &str) -> String {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.pointer("/error/message")?.as_str().map(str::to_owned));
    match message.filter(|message| !message.trim().is_empty()) {
        Some(message) => format!(
            "Anthropic API returned HTTP {}: {}",
            status.as_u16(),
            message.chars().take(512).collect::<String>()
        ),
        None => format!("Anthropic API returned HTTP {}", status.as_u16()),
    }
}
