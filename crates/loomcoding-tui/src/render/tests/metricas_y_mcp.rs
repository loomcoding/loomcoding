use super::*;

#[test]
fn context_breakdown_line_shows_total_and_the_three_main_parts() {
    let desglose = loomcoding_core::agent::compact::ContextBreakdown {
        system: 100,
        tools: 40,
        messages: 20,
        framing: 3,
    };
    assert_eq!(
        context_breakdown_line(&desglose),
        "Context ~163 · sys 100 · tools 40 · msgs 20"
    );
}

#[test]
fn cache_usage_line_is_absent_without_data_and_sums_the_records() {
    assert_eq!(cache_usage_line(&[]), None);
    assert_eq!(cache_usage_line(&[record_with_cache(0, 0)]), None);
    let registros = [record_with_cache(900, 40), record_with_cache(100, 0)];
    assert_eq!(
        cache_usage_line(&registros).as_deref(),
        Some("Cache read 1000 · write 40")
    );
}

fn mcp_server(id: &str, tools: usize, tokens: u64) -> crate::app::McpServerCapability {
    crate::app::McpServerCapability {
        id: id.into(),
        initialized: true,
        tool_count: tools,
        schema_tokens: tokens,
        issue: None,
    }
}

#[test]
fn mcp_server_summary_shows_tools_and_estimated_tokens() {
    assert_eq!(
        mcp_server_summary(&mcp_server("fs", 12, 3400)),
        "fs · 12 tools · ~3400 tok"
    );
    assert_eq!(
        mcp_server_summary(&mcp_server("git", 1, 80)),
        "git · 1 tool · ~80 tok"
    );
}

#[test]
fn mcp_load_warning_stays_silent_under_both_thresholds() {
    let servers = [mcp_server("a", 10, 4000), mcp_server("b", 10, 5000)];
    assert_eq!(mcp_load_warning(&servers), None);
}

#[test]
fn mcp_load_warning_names_the_threshold_that_was_exceeded() {
    let many = [mcp_server("a", 20, 100), mcp_server("b", 11, 100)];
    let by_tools = mcp_load_warning(&many).unwrap();
    assert!(by_tools.contains("31 tools"), "{by_tools}");
    let heavy = [mcp_server("a", 2, 6000), mcp_server("b", 2, 4001)];
    let by_tokens = mcp_load_warning(&heavy).unwrap();
    assert!(by_tokens.contains("10001"), "{by_tokens}");
}

#[test]
fn mcp_load_warning_ignores_servers_that_failed_to_initialize() {
    let mut down = mcp_server("a", 40, 20_000);
    down.initialized = false;
    assert_eq!(mcp_load_warning(&[down]), None);
}

#[test]
fn crush_layout_has_only_a_right_rail_when_wide() {
    let mut app = crush_ctx();
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("MODEL"));
    assert!(content.contains("mistral-nemo"));
    assert!(!content.contains("PROJECT")); // secciones del sidebar izquierdo
    assert!(!content.contains("FLOCK"));
    assert!(!content.contains("Token usage")); // panel de métricas
    assert!(!content.contains("Conversation")); // panel del chat
}
