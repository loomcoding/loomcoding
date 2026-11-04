//! Cuerpo de la petición a la Messages API: mensajes en el formato del proveedor, puntos de caché,
//! presupuesto de razonamiento y herramientas.

use crate::provider::ProviderRequest;

use super::*;

/// Traduce un nivel de esfuerzo al presupuesto de razonamiento de Anthropic.
/// Cabe señalar que esta tabla es una suposición pendiente de contrastar con el
/// servicio real; un nivel fuera de ella no envía nada.
pub(super) fn anthropic_budget(effort: &str) -> Option<u32> {
    match effort {
        "minimal" => Some(1024),
        "low" => Some(2048),
        "medium" => Some(8192),
        "high" => Some(16384),
        "max" => Some(32000),
        _ => None,
    }
}

/// Marca puntos de caché de prompts en un cuerpo ya construido: como mucho dos (el límite del proveedor
/// es cuatro). Uno en el sistema, que por el orden del proveedor cubre también las herramientas, y otro
/// en el último bloque del último mensaje. Ver `docs/specs/hechos/2026-10-31-codeloom-prompt-cache-points.md`.
pub(super) fn apply_cache_points(body: &mut serde_json::Value) {
    let marker = serde_json::json!({ "type": "ephemeral" });
    // Punto 1: el sistema. Cadena a bloque de texto; si está vacío, la última herramienta.
    match body.get("system").cloned() {
        Some(serde_json::Value::String(text)) if !text.is_empty() => {
            body["system"] = serde_json::json!([
                { "type": "text", "text": text, "cache_control": marker }
            ]);
        }
        _ => {
            if let Some(tool) = body
                .get_mut("tools")
                .and_then(serde_json::Value::as_array_mut)
                .and_then(|tools| tools.last_mut())
            {
                tool["cache_control"] = marker.clone();
            }
        }
    }
    // Punto 2: el último bloque del último mensaje. Los bloques de razonamiento no admiten el marcador.
    let Some(message) = body
        .get_mut("messages")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|messages| messages.last_mut())
    else {
        return;
    };
    if let Some(text) = message["content"].as_str().map(str::to_owned) {
        if text.is_empty() {
            return;
        }
        message["content"] = serde_json::json!([
            { "type": "text", "text": text, "cache_control": marker }
        ]);
        return;
    }
    if let Some(block) = message["content"]
        .as_array_mut()
        .and_then(|blocks| blocks.last_mut())
    {
        let markable = matches!(
            block["type"].as_str(),
            Some("text" | "tool_use" | "tool_result" | "image")
        );
        if markable {
            block["cache_control"] = marker;
        }
    }
}

impl AnthropicProvider {
    pub(super) fn build_body(&self, req: &ProviderRequest) -> serde_json::Value {
        let messages = wire_messages(&req.messages, &req.model, &self.base_url);
        let mut system = req.system.clone();
        for message in req.messages.iter().filter(|message| {
            message.role == crate::message::Role::System && !message.text().is_empty()
        }) {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(&message.text());
        }
        let mut tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                let mut tool = serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema
                });
                if req.programmatic_tool_calling && t.programmatic_allowed {
                    tool["allowed_callers"] =
                        serde_json::json!(["direct", "code_execution_20260120"]);
                }
                tool
            })
            .collect();
        if req.programmatic_tool_calling && req.tools.iter().any(|tool| tool.programmatic_allowed) {
            tools.push(serde_json::json!({
                "type": "code_execution_20260521",
                "name": "code_execution"
            }));
        }
        let mut cuerpo = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
            "stream": !req.programmatic_tool_calling,
            "temperature": req.temperature
        });
        if self.is_openrouter_haiku_55(&req.model) {
            cuerpo.as_object_mut().unwrap().remove("temperature");
            cuerpo["provider"] = serde_json::json!({
                "order": ["anthropic"],
                "allow_fallbacks": false
            });
        // Sin esfuerzo (o con un nivel fuera de la tabla), el cuerpo no cambia.
        } else if let Some(presupuesto) = req.effort.as_deref().and_then(anthropic_budget) {
            cuerpo["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": presupuesto
            });
            // La respuesta visible va detrás del razonamiento: max_tokens debe superar
            // el presupuesto, así que se amplía en vez de recortar el presupuesto.
            cuerpo["max_tokens"] =
                serde_json::json!(presupuesto.saturating_add(req.max_tokens.max(1)));
            // Con `thinking` la API no admite una temperatura personalizada: se omite.
            if let Some(obj) = cuerpo.as_object_mut() {
                obj.remove("temperature");
            }
        }
        if let Some(container) = req.messages.iter().rev().find_map(|message| {
            message
                .provider_state
                .as_ref()
                .filter(|state| {
                    state.provider == "anthropic"
                        && state.data["model"] == req.model
                        && state.data["base_url"] == self.base_url
                })
                .and_then(|state| state.data.get("container_id"))
                .and_then(serde_json::Value::as_str)
        }) {
            cuerpo["container"] = serde_json::json!({ "id": container });
        }
        if self.prompt_cache {
            apply_cache_points(&mut cuerpo);
        }
        cuerpo
    }
}

pub(super) fn role_str(r: crate::message::Role) -> &'static str {
    use crate::message::Role::{Assistant, System, Tool, User};
    match r {
        System => "system",
        User => "user",
        Assistant => "assistant",
        Tool => "user",
    }
}

/// Traduce los mensajes al formato Messages: bloques `tool_use` en el
/// asistente y `tool_result` en un mensaje de usuario (uno por ToolResult).
pub(super) fn wire_messages(
    messages: &[crate::message::Message],
    model: &str,
    base_url: &str,
) -> Vec<serde_json::Value> {
    use crate::message::{ContentPart, Role};
    let mut out = Vec::new();
    for m in messages {
        match m.role {
            Role::System => {}
            Role::Assistant => {
                if let Some(content) = m
                    .provider_state
                    .as_ref()
                    .filter(|state| {
                        state.provider == "anthropic"
                            && state.data["model"] == model
                            && state.data["base_url"] == base_url
                    })
                    .and_then(|state| state.data.get("assistant_content"))
                {
                    out.push(serde_json::json!({ "role": "assistant", "content": content }));
                    continue;
                }
                let mut blocks: Vec<serde_json::Value> = Vec::new();
                let text = m.text();
                if !text.is_empty() {
                    blocks.push(serde_json::json!({ "type": "text", "text": text }));
                }
                for p in &m.parts {
                    if let ContentPart::ToolCall { id, name, input } = p {
                        blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }));
                    }
                }
                // Un turno sin contenido se omite: un bloque de texto vacío da un 400.
                if blocks.is_empty() {
                    continue;
                }
                out.push(serde_json::json!({ "role": "assistant", "content": blocks }));
            }
            Role::Tool => {
                let mut blocks: Vec<serde_json::Value> = Vec::new();
                for p in &m.parts {
                    if let ContentPart::ToolResult {
                        call_id,
                        content,
                        is_error,
                    } = p
                    {
                        blocks.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": call_id,
                            "content": crate::tools::limit_output(content),
                            "is_error": is_error,
                        }));
                    }
                }
                if !blocks.is_empty() {
                    out.push(serde_json::json!({ "role": "user", "content": blocks }));
                }
            }
            other => {
                let has_image = m.parts.iter().any(|part| {
                    matches!(
                        part,
                        ContentPart::Image { .. } | ContentPart::ImageUrl { .. }
                    )
                });
                let content = if has_image {
                    serde_json::Value::Array(
                        m.parts
                            .iter()
                            .filter_map(|part| match part {
                                ContentPart::Text { text } => Some(serde_json::json!({
                                    "type": "text",
                                    "text": text
                                })),
                                ContentPart::Image {
                                    mime_type, data, ..
                                } => Some(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": mime_type,
                                        "data": data
                                    }
                                })),
                                ContentPart::ImageUrl { url } => Some(serde_json::json!({
                                    "type": "image",
                                    "source": { "type": "url", "url": url }
                                })),
                                _ => None,
                            })
                            .collect(),
                    )
                } else {
                    serde_json::Value::String(m.text())
                };
                out.push(serde_json::json!({ "role": role_str(other), "content": content }));
            }
        }
    }
    out
}
