//! Tests de Anthropic: effort.

use super::*;
use crate::provider::ToolSpec;
use serde_json::json;

/// Petición de prueba con una herramienta, para que el cuerpo sea representativo.
fn peticion(effort: Option<&str>, max_tokens: u32) -> ProviderRequest {
    ProviderRequest {
        model: "m".into(),
        system: "sys".into(),
        messages: vec![],
        tools: vec![ToolSpec {
            name: "view".into(),
            description: "Ve un fichero".into(),
            input_schema: json!({ "type": "object" }),
            programmatic_allowed: true,
        }],
        temperature: 0.5,
        max_tokens,
        effort: effort.map(String::from),
        programmatic_tool_calling: false,
    }
}

#[test]
fn anthropic_body_without_effort_is_unchanged() {
    // Cuerpo exacto que enviaba `build_body` antes de existir el esfuerzo.
    let previo = json!({
        "model": "m",
        "max_tokens": 16,
        "system": "sys",
        "messages": [],
        "tools": [{
            "name": "view",
            "description": "Ve un fichero",
            "input_schema": { "type": "object" }
        }],
        "stream": true,
        "temperature": 0.5
    });
    let p = AnthropicProvider::new("http://x", "k");
    assert_eq!(p.build_body(&peticion(None, 16)), previo);
}

#[test]
fn anthropic_effort_enables_thinking_and_raises_max_tokens() {
    let p = AnthropicProvider::new("http://x", "k");
    let cuerpo = p.build_body(&peticion(Some("high"), 1000));
    assert_eq!(cuerpo["thinking"]["type"], "enabled");
    assert_eq!(cuerpo["thinking"]["budget_tokens"], 16384);
    let max = cuerpo["max_tokens"].as_u64().unwrap();
    assert!(
        max > 16384,
        "max_tokens ({max}) debe superar el presupuesto"
    );
    assert_eq!(max, 16384 + 1000);
}

#[test]
fn anthropic_effort_omits_temperature() {
    let p = AnthropicProvider::new("http://x", "k");
    let cuerpo = p.build_body(&peticion(Some("low"), 16));
    assert!(cuerpo.get("temperature").is_none());
}

#[test]
fn anthropic_budget_table() {
    assert_eq!(anthropic_budget("minimal"), Some(1024));
    assert_eq!(anthropic_budget("low"), Some(2048));
    assert_eq!(anthropic_budget("medium"), Some(8192));
    assert_eq!(anthropic_budget("high"), Some(16384));
    assert_eq!(anthropic_budget("max"), Some(32000));
    assert_eq!(anthropic_budget("xhigh"), None);
}

#[test]
fn unknown_effort_sends_nothing() {
    // Un nivel fuera de la tabla deja la petición exactamente como la de «default».
    let p = AnthropicProvider::new("http://x", "k");
    assert_eq!(
        p.build_body(&peticion(Some("turbo"), 16)),
        p.build_body(&peticion(None, 16))
    );
}
