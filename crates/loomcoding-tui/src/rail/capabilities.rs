//! Sección de capacidades: MCP, LSP y skills.

use super::*;

/// Capacidades de ejecución: MCP, LSP y skills anunciadas.
pub(super) fn push_capabilities<'a>(out: &mut Vec<Line<'a>>, app: &'a App, width: usize) {
    if let Some(capabilities) = &app.runtime_capabilities {
        push_gap(out);
        out.push(title("CAPABILITIES", app));
        let (mcp_summary, mcp_issue) = match capabilities.mcp_config {
            crate::app::McpConfigState::Unknown => ("snapshot pending".to_owned(), None),
            crate::app::McpConfigState::NotConfigured => ("not configured".to_owned(), None),
            crate::app::McpConfigState::Invalid => ("config invalid".to_owned(), None),
            crate::app::McpConfigState::Loaded if capabilities.mcp_servers.is_empty() => {
                ("configured · no servers".to_owned(), None)
            }
            crate::app::McpConfigState::Loaded => {
                let ready = capabilities
                    .mcp_servers
                    .iter()
                    .filter(|server| server.initialized)
                    .count();
                let tools = capabilities
                    .mcp_servers
                    .iter()
                    .map(|server| server.tool_count)
                    .sum::<usize>();
                let issue = capabilities.mcp_servers.iter().find_map(|server| {
                    server
                        .issue
                        .as_ref()
                        .map(|issue| format!("{}: {issue}", server.id))
                });
                (
                    format!(
                        "{ready}/{} initialized · {tools} tools · delegates",
                        capabilities.mcp_servers.len()
                    ),
                    issue,
                )
            }
        };
        out.push(Line::from(vec![
            Span::styled("MCP  ", app.theme.styles.accent),
            Span::raw(recortar(&mcp_summary, width.saturating_sub(5))),
        ]));
        for server in capabilities.mcp_servers.iter().take(2) {
            let (icon, style, state) = if server.initialized {
                ("✓ ", app.theme.styles.success, "initialized")
            } else {
                ("! ", app.theme.styles.error, "failed")
            };
            out.push(Line::from(vec![
                Span::styled(icon, style),
                Span::raw(recortar(
                    &format!("{} · {state} · {} tools", server.id, server.tool_count),
                    width.saturating_sub(2),
                )),
            ]));
        }
        if capabilities.mcp_servers.len() > 2 {
            out.push(Line::from(Span::styled(
                format!("… {} more MCP servers", capabilities.mcp_servers.len() - 2),
                app.theme.styles.muted,
            )));
        }
        if let Some(issue) = mcp_issue {
            out.push(Line::from(vec![
                Span::styled("! ", app.theme.styles.error),
                Span::raw(recortar(&issue, width.saturating_sub(2))),
            ]));
        }
        let lsp_status = if app.lsp_invoked {
            "query called this session".to_owned()
        } else {
            match (
                capabilities.lsp_registered,
                capabilities.rust_analyzer_available,
            ) {
                (false, _) => "tool unavailable".to_owned(),
                (true, false) => "registered · rust-analyzer missing".to_owned(),
                (true, true) => "rust-analyzer found · on demand".to_owned(),
            }
        };
        out.push(Line::from(vec![
            Span::styled("LSP  ", app.theme.styles.accent),
            Span::raw(recortar(&lsp_status, width.saturating_sub(5))),
        ]));
        let skills = format!(
            "{}/{} cards · {} scopes declared{}",
            capabilities.available_skill_total,
            capabilities.announced_skill_total,
            capabilities.declared_skill_scope_total,
            if capabilities.local_skill_loader_available {
                ""
            } else {
                " · SKILL.md runtime absent"
            }
        );
        out.push(Line::from(vec![
            Span::styled("SKILLS  ", app.theme.styles.accent),
            Span::raw(recortar(&skills, width.saturating_sub(8))),
        ]));
        for skill in capabilities.announced_skills.iter().take(2) {
            let status = if skill.available {
                "available"
            } else {
                "pending"
            };
            out.push(Line::from(Span::styled(
                recortar(&format!("{} · {status}", skill.name), width),
                app.theme.styles.muted,
            )));
        }
    }
}
