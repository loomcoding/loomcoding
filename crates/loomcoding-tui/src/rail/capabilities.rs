//! Sección de capacidades: una línea tenue con MCP, LSP y skills, y debajo solo las incidencias.

use super::*;
use crate::render::chrome::capabilities_parts;

/// Capacidades de ejecución. Cabe señalar que el resumen ocupa una única línea tenue, sin título,
/// y que solo se añade una línea por cada servidor MCP fallido o con incidencia: los servidores
/// sanos y las skills ya quedan resumidos en esa línea.
pub(super) fn push_capabilities<'a>(out: &mut Vec<Line<'a>>, app: &'a App, width: usize) {
    let Some(capabilities) = &app.runtime_capabilities else {
        return;
    };
    let Some([mcp, lsp, skills]) = capabilities_parts(app) else {
        return;
    };
    push_gap(out);
    // Cabe señalar que el resumen se parte por palabras en lugar de recortarse: el rail es estrecho
    // y lo que importa (MCP, LSP, skills) iría al final de una línea cortada.
    for linea in partir_por_palabras(&format!("{mcp} · {lsp} · {skills}"), width) {
        out.push(Line::from(Span::styled(linea, app.theme.styles.muted)));
    }
    for server in &capabilities.mcp_servers {
        if server.initialized && server.issue.is_none() {
            continue;
        }
        let motivo = server.issue.as_deref().unwrap_or("failed");
        out.push(Line::from(Span::styled(
            recortar(&format!("! {}: {motivo}", server.id), width),
            app.theme.styles.error,
        )));
    }
}
