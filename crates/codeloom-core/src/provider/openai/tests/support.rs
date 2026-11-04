//! Piezas comunes de los tests.

use super::*;

/// Petición de prueba con una herramienta, para que el cuerpo sea representativo.
pub(super) fn peticion(effort: Option<&str>) -> ProviderRequest {
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
        max_tokens: 16,
        effort: effort.map(String::from),
        programmatic_tool_calling: false,
    }
}

pub(super) fn request() -> ProviderRequest {
    ProviderRequest {
        model: "model-with-ptc".into(),
        system: "system".into(),
        messages: vec![],
        tools: vec![
            ToolSpec {
                name: "view".into(),
                description: "read a file".into(),
                input_schema: json!({"type":"object"}),
                programmatic_allowed: true,
            },
            ToolSpec {
                name: "write".into(),
                description: "write a file".into(),
                input_schema: json!({"type":"object"}),
                programmatic_allowed: false,
            },
        ],
        temperature: 0.0,
        max_tokens: 128,
        effort: None,
        programmatic_tool_calling: true,
    }
}
