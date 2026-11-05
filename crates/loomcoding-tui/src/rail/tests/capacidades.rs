//! Tests: capacidades del rail (una línea tenue y solo las incidencias de MCP).

use super::support::*;
use super::*;
use crate::app::{McpConfigState, McpServerCapability, RuntimeCapabilities};

/// Capacidades con un servidor MCP sano, LSP con rust-analyzer y sin skills anunciadas.
fn capacidades_sanas() -> RuntimeCapabilities {
    RuntimeCapabilities {
        mcp_config: McpConfigState::Loaded,
        mcp_servers: vec![McpServerCapability {
            id: "fs".into(),
            initialized: true,
            tool_count: 3,
            schema_tokens: 0,
            issue: None,
        }],
        lsp_registered: true,
        rust_analyzer_available: true,
        ..Default::default()
    }
}

#[test]
fn capabilities_are_one_muted_line() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.runtime_capabilities = Some(capacidades_sanas());

    // Ancho amplio: el resumen completo cabe en una sola fila.
    let rows = rows_of(&app, 120, 30);
    let fila = rows
        .iter()
        .find(|row| row.contains("MCP 1/1 initialized"))
        .expect("el rail muestra el resumen de MCP");
    assert!(
        fila.contains("LSP rust-analyzer found") && fila.contains("skills 0/0 cards"),
        "el resumen no va en una sola línea: {fila:?}"
    );
    assert_eq!(
        estilo_de(&app, "MCP 1/1"),
        app.theme.styles.muted,
        "el resumen de capacidades no va tenue"
    );
    assert!(
        !rows.iter().any(|row| row.contains("Capabilities")),
        "sigue el título Capabilities: {rows:?}"
    );
    assert!(
        !rows
            .iter()
            .any(|row| row.contains("MCP  ") || row.contains("LSP  ") || row.contains("SKILLS")),
        "sobreviven las líneas de acento antiguas: {rows:?}"
    );
    assert!(
        !rows.iter().any(|row| row.contains("✓ fs")),
        "un servidor sano no debe listarse: {rows:?}"
    );
}

#[test]
fn failed_mcp_server_keeps_an_error_line() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let mut capacidades = capacidades_sanas();
    capacidades.mcp_servers.push(McpServerCapability {
        id: "git".into(),
        initialized: false,
        tool_count: 0,
        schema_tokens: 0,
        issue: Some("spawn failed".into()),
    });
    app.runtime_capabilities = Some(capacidades);

    let rows = rows_of(&app, 120, 30);
    assert!(
        rows.iter().any(|row| row.contains("! git: spawn failed")),
        "falta la línea de incidencia del servidor fallido: {rows:?}"
    );
    assert_eq!(
        estilo_de(&app, "! git"),
        app.theme.styles.error,
        "la incidencia no va en estilo de error"
    );
    assert!(
        !rows.iter().any(|row| row.contains("! fs")),
        "un servidor sano no debe llevar línea de incidencia: {rows:?}"
    );
}

#[test]
fn a_narrow_rail_wraps_the_capabilities_instead_of_truncating_them() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.runtime_capabilities = Some(capacidades_sanas());

    // Con 33 columnas el rail deja 31 útiles: el resumen no cabe en una fila y no puede perderse.
    let rows = rows_of(&app, 33, 30);
    let todo = rows.join(" ");
    for palabra in [
        "MCP",
        "initialized",
        "LSP",
        "rust-analyzer",
        "found",
        "skills",
        "cards",
    ] {
        assert!(todo.contains(palabra), "falta {palabra:?} en {rows:?}");
    }
    assert!(
        !rows.iter().any(|row| row.contains('…')),
        "el resumen se recorta con «…»: {rows:?}"
    );
}
