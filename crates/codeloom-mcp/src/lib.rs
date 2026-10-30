use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codeloom_core::permission::{Decision, PermissionRequest};
use codeloom_core::tools::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde::Deserialize;
use tokio::process::Command;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const MAX_SERVERS: usize = 8;
const MAX_TOOLS: usize = 128;
const MAX_TOOL_SCHEMA_BYTES: usize = 32_768;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 4_000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

type Client = RunningService<RoleClient, ()>;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    mcp: McpSection,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct McpSection {
    servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    /// Child environment name -> host environment variable name; values stay out of config.
    #[serde(default)]
    env: BTreeMap<String, String>,
}

impl McpConfig {
    /// Loads user-owned configuration. Project profiles cannot add or launch servers.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(format!("cannot read MCP config: {error}")),
        };
        let config: Self =
            toml::from_str(&text).map_err(|error| format!("invalid MCP config: {error}"))?;
        if config.mcp.servers.len() > MAX_SERVERS {
            return Err(format!("MCP config supports at most {MAX_SERVERS} servers"));
        }
        for (id, server) in &config.mcp.servers {
            validate_id(id)?;
            if server.command.trim().is_empty()
                || server.command.len() > 1024
                || server.command.chars().any(char::is_control)
                || server.args.len() > 64
                || server
                    .args
                    .iter()
                    .any(|arg| arg.len() > 4096 || arg.contains('\0'))
            {
                return Err(format!(
                    "invalid command or arguments for MCP server `{id}`"
                ));
            }
            for (child_name, host_name) in &server.env {
                if !valid_env_name(child_name) || !valid_env_name(host_name) {
                    return Err(format!(
                        "invalid environment variable mapping for MCP server `{id}`"
                    ));
                }
            }
        }
        Ok(config)
    }

    pub fn server_ids(&self) -> Vec<String> {
        self.mcp.servers.keys().cloned().collect()
    }
}

#[derive(Default)]
pub struct McpRuntime {
    tools: Vec<Arc<dyn Tool>>,
    issues: Vec<McpIssue>,
    servers: Vec<McpServerStatus>,
    // Holding each service keeps the configured child process alive for its tools.
    _clients: Vec<Arc<Client>>,
}

#[derive(Debug, Clone)]
pub struct McpIssue {
    pub server_id: String,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerStatus {
    pub server_id: String,
    pub initialized: bool,
    pub tool_count: usize,
    pub issue: Option<String>,
}

impl McpRuntime {
    pub async fn start(config: McpConfig, cwd: &Path) -> Self {
        let configured = config.server_ids();
        let mut pending = JoinSet::new();
        for (id, server) in config.mcp.servers {
            let cwd = cwd.to_path_buf();
            pending.spawn(async move { connect_server(id, server, cwd).await });
        }

        let mut runtime = Self::default();
        while let Some(result) = pending.join_next().await {
            match result {
                Ok(Ok((id, client, tools))) => {
                    tracing::info!(server = %id, tools = tools.len(), "MCP server connected");
                    runtime.servers.push(McpServerStatus {
                        server_id: id,
                        initialized: true,
                        tool_count: tools.len(),
                        issue: None,
                    });
                    runtime._clients.push(client);
                    runtime.tools.extend(tools);
                }
                Ok(Err((id, _))) => {
                    tracing::warn!(server = %id, "MCP server unavailable");
                    runtime.issues.push(McpIssue {
                        server_id: id.clone(),
                        reason: "could not start or list tools",
                    });
                    runtime.servers.push(McpServerStatus {
                        server_id: id,
                        initialized: false,
                        tool_count: 0,
                        issue: Some("could not start or list tools".into()),
                    });
                }
                Err(_) => tracing::warn!("MCP connection task failed"),
            }
        }
        for server_id in configured {
            if !runtime
                .servers
                .iter()
                .any(|status| status.server_id == server_id)
            {
                runtime.servers.push(McpServerStatus {
                    server_id,
                    initialized: false,
                    tool_count: 0,
                    issue: Some("connection task failed".into()),
                });
            }
        }
        runtime
            .servers
            .sort_by(|a, b| a.server_id.cmp(&b.server_id));
        runtime
    }

    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }

    pub fn issues(&self) -> &[McpIssue] {
        &self.issues
    }

    pub fn server_statuses(&self) -> &[McpServerStatus] {
        &self.servers
    }
}

async fn connect_server(
    id: String,
    config: McpServerConfig,
    cwd: std::path::PathBuf,
) -> Result<(String, Arc<Client>, Vec<Arc<dyn Tool>>), (String, String)> {
    let mut command = Command::new(&config.command);
    command
        .args(&config.args)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    for key in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "TMPDIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    for (child_name, host_name) in config.env {
        let value = std::env::var_os(&host_name).ok_or_else(|| {
            (
                id.clone(),
                format!("environment variable `{host_name}` is missing"),
            )
        })?;
        command.env(child_name, value);
    }

    let connect = async {
        let transport = TokioChildProcess::new(command).map_err(|error| error.to_string())?;
        let client = ().serve(transport).await.map_err(|error| error.to_string())?;
        let response = client
            .peer()
            .list_tools(None)
            .await
            .map_err(|error| error.to_string())?;
        Ok::<_, String>((client, response.tools))
    };
    let (client, advertised) = timeout(CONNECT_TIMEOUT, connect)
        .await
        .map_err(|_| (id.clone(), "connection timed out".into()))?
        .map_err(|error| (id.clone(), error))?;
    let client = Arc::new(client);
    let tools = tools_from_server(&id, client.clone(), advertised);
    Ok((id, client, tools))
}

fn tools_from_server(
    id: &str,
    client: Arc<Client>,
    advertised: Vec<rmcp::model::Tool>,
) -> Vec<Arc<dyn Tool>> {
    let mut tools = Vec::new();
    let mut seen_names = HashSet::new();
    for advertised_tool in advertised.into_iter().take(MAX_TOOLS) {
        let name = advertised_tool.name.to_string();
        if !valid_tool_name(&name) || !seen_names.insert(name.clone()) {
            tracing::warn!(server = %id, tool = %name, "ignoring MCP tool with unsupported or duplicate name");
            continue;
        }
        let input_schema = advertised_tool.schema_as_json_value();
        let schema_bytes =
            serde_json::to_vec(&input_schema).map_or(usize::MAX, |bytes| bytes.len());
        if !input_schema.is_object() || schema_bytes > MAX_TOOL_SCHEMA_BYTES {
            tracing::warn!(server = %id, tool = %name, "ignoring MCP tool with invalid or oversized schema");
            continue;
        }
        let public_name = format!("mcp__{id}__{name}");
        if public_name.len() > 64 {
            tracing::warn!(server = %id, tool = %name, "ignoring MCP tool whose namespaced name exceeds 64 bytes");
            continue;
        }
        tools.push(Arc::new(McpTool {
            client: client.clone(),
            server_id: id.to_owned(),
            remote_name: name,
            public_name,
            description: codeloom_core::tools::limit_bytes(
                advertised_tool.description.as_deref().unwrap_or("MCP tool"),
                MAX_TOOL_DESCRIPTION_BYTES,
            )
            .into_owned(),
            input_schema,
        }) as Arc<dyn Tool>);
    }
    tools
}

struct McpTool {
    client: Arc<Client>,
    server_id: String,
    remote_name: String,
    public_name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.public_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> serde_json::Value {
        self.input_schema.clone()
    }

    fn mcp_server_id(&self) -> Option<&str> {
        Some(&self.server_id)
    }

    fn execution_mode(&self, _input: &serde_json::Value) -> ExecutionMode {
        // Server-provided annotations are not a permission decision; assume tools may write.
        ExecutionMode::Exclusive
    }

    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> codeloom_core::error::Result<ToolOutput> {
        let detail =
            serde_json::to_string(&input).unwrap_or_else(|_| "(arguments unavailable)".into());
        let permission = ctx
            .permission
            .check(PermissionRequest {
                tool: "mcp".into(),
                action: format!("{}:{}", self.server_id, self.remote_name),
                detail: format!(
                    "{} · {}",
                    self.server_id,
                    codeloom_core::tools::limit_bytes(&detail, 2048)
                ),
            })
            .await;
        if permission != Decision::Allow {
            return Ok(ToolOutput::err("MCP tool call denied"));
        }
        let arguments = input.as_object().cloned().unwrap_or_default();
        let params = CallToolRequestParams::new(self.remote_name.clone()).with_arguments(arguments);
        let result = tokio::select! {
            result = timeout(CALL_TIMEOUT, self.client.call_tool(params)) => match result {
                Ok(result) => result,
                Err(_) => return Ok(ToolOutput::err("MCP tool call timed out")),
            },
            _ = cancel.cancelled() => return Ok(ToolOutput::err("cancelled")),
        }
        .map_err(|_| codeloom_core::error::CoreError::Tool("MCP tool call failed".into()))?;
        let content = match LimitedJson::serialize(&result) {
            Ok(content) => content,
            Err(true) => "MCP result exceeded the 20000 byte output limit".into(),
            Err(false) => "MCP returned an unrenderable result".into(),
        };
        Ok(if result.is_error.unwrap_or(false) {
            ToolOutput::err(content)
        } else {
            ToolOutput::ok(content)
        })
    }
}

struct LimitedJson(Vec<u8>);

impl LimitedJson {
    fn serialize(value: &impl serde::Serialize) -> Result<String, bool> {
        let mut output = Self(Vec::with_capacity(
            codeloom_core::tools::MAX_TOOL_OUTPUT_BYTES,
        ));
        match serde_json::to_writer(&mut output, value) {
            Ok(()) => String::from_utf8(output.0).map_err(|_| false),
            Err(_) => Err(output.0.len() == codeloom_core::tools::MAX_TOOL_OUTPUT_BYTES),
        }
    }
}

impl std::io::Write for LimitedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let remaining = codeloom_core::tools::MAX_TOOL_OUTPUT_BYTES.saturating_sub(self.0.len());
        let written = bytes.len().min(remaining);
        self.0.extend_from_slice(&bytes[..written]);
        if written < bytes.len() {
            Err(std::io::Error::other("MCP output limit exceeded"))
        } else {
            Ok(written)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 16
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!(
            "invalid MCP server ID `{id}` (use 1-16 lowercase letters, digits, or hyphens)"
        ));
    }
    Ok(())
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 45
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeloom_core::permission::DenyAll;
    use codeloom_core::tools::ToolRegistry;
    use rmcp::{
        handler::server::router::{
            tool::{SyncTool, ToolBase},
            Router,
        },
        ServiceExt,
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::task::JoinHandle;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn config_file(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codeloom-mcp-{}-{}.toml",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn missing_config_means_no_servers() {
        let path = std::env::temp_dir().join(format!(
            "codeloom-mcp-missing-{}-{}.toml",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(McpConfig::load(&path).unwrap().mcp.servers.is_empty());
    }

    #[test]
    fn config_validates_server_ids_and_environment_names() {
        let bad_id = config_file("[mcp.servers.Files]\ncommand = 'server'\n");
        assert!(McpConfig::load(&bad_id).is_err());
        std::fs::remove_file(bad_id).unwrap();

        let bad_env = config_file(
            "[mcp.servers.files]\ncommand = 'server'\n[mcp.servers.files.env]\nTOKEN = 'value-that-is-not-an-env-name'\n",
        );
        assert!(McpConfig::load(&bad_env).is_err());
        std::fs::remove_file(bad_env).unwrap();

        let valid = config_file(
            "[mcp.servers.files]\ncommand = 'server'\n[mcp.servers.files.env]\nTOKEN = 'FILES_TOKEN'\n",
        );
        assert_eq!(McpConfig::load(&valid).unwrap().mcp.servers.len(), 1);
        std::fs::remove_file(valid).unwrap();
    }

    #[test]
    fn invalid_toml_and_excess_server_count_are_rejected() {
        let invalid = config_file("[mcp.servers.files\ncommand = 'server'\n");
        assert!(McpConfig::load(&invalid).is_err());
        std::fs::remove_file(invalid).unwrap();

        let mut config = String::new();
        for index in 0..=MAX_SERVERS {
            config.push_str(&format!("[mcp.servers.s{index}]\ncommand = 'server'\n"));
        }
        let too_many = config_file(&config);
        assert!(McpConfig::load(&too_many).is_err());
        std::fs::remove_file(too_many).unwrap();
    }

    #[tokio::test]
    async fn missing_transport_environment_fails_before_starting_command() {
        let missing = format!(
            "CODELOOM_TEST_MISSING_{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let config = McpServerConfig {
            command: "this-command-must-not-run".into(),
            args: Vec::new(),
            env: BTreeMap::from([("TOKEN".into(), missing.clone())]),
        };
        let error = connect_server("files".into(), config, std::env::temp_dir())
            .await
            .err()
            .unwrap();
        assert!(error.1.contains(&missing));
    }

    struct FakeMcpServer {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl rmcp::handler::server::ServerHandler for FakeMcpServer {}

    #[derive(Default, serde::Deserialize, schemars::JsonSchema)]
    struct FakeInput {
        #[serde(default)]
        large: bool,
    }

    struct FakeReadTool;

    impl ToolBase for FakeReadTool {
        type Parameter = FakeInput;
        type Output = String;
        type Error = rmcp::ErrorData;

        fn name() -> std::borrow::Cow<'static, str> {
            "read".into()
        }

        fn description() -> Option<std::borrow::Cow<'static, str>> {
            Some("Fake read-only MCP tool".into())
        }
    }

    impl SyncTool<FakeMcpServer> for FakeReadTool {
        fn invoke(service: &FakeMcpServer, input: FakeInput) -> Result<String, Self::Error> {
            service.calls.fetch_add(1, Ordering::SeqCst);
            Ok(if input.large {
                "x".repeat(40_000)
            } else {
                "fake result".into()
            })
        }
    }

    async fn fake_server(
        id: &str,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    ) -> (Arc<Client>, Vec<Arc<dyn Tool>>, JoinHandle<()>) {
        let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move {
            let mut service = Router::new(FakeMcpServer { calls });
            service.tool_router =
                rmcp::handler::server::tool::ToolRouter::new().with_sync_tool::<FakeReadTool>();
            if let Ok(service) = service.serve(server_transport).await {
                let _ = service.waiting().await;
            }
        });
        let client = Arc::new(().serve(client_transport).await.unwrap());
        let advertised = client.peer().list_tools(None).await.unwrap().tools;
        let tools = tools_from_server(id, client.clone(), advertised);
        (client, tools, server)
    }

    fn tool_context(permission: Arc<codeloom_core::permission::PermissionService>) -> ToolCtx {
        ToolCtx {
            cwd: std::env::temp_dir(),
            store: None,
            permission,
            session: codeloom_core::session::SessionId(1),
            jobs: Arc::new(codeloom_core::job::LocalJobRegistry::default()),
            bus: codeloom_core::event::EventBus::new(8),
            subagents: None,
        }
    }

    #[tokio::test]
    async fn scoped_fake_mcp_tools_gate_calls_and_never_expose_denied_servers() {
        let allowed_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let denied_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (allowed_client, allowed_tools, allowed_server) =
            fake_server("files", allowed_calls.clone()).await;
        let (denied_client, denied_tools, denied_server) =
            fake_server("other", denied_calls.clone()).await;
        let mut registry = ToolRegistry::new();
        for tool in allowed_tools.into_iter().chain(denied_tools) {
            registry.register(tool);
        }
        let scoped = registry.with_mcp_scope(&["files".into()]);
        let allowed = scoped.get("mcp__files__read").unwrap();

        let denied = allowed
            .run(
                tool_context(Arc::new(
                    codeloom_core::permission::PermissionService::with_responder(Box::new(DenyAll)),
                )),
                serde_json::json!({}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(denied.is_error);
        assert_eq!(allowed_calls.load(Ordering::SeqCst), 0);
        assert_eq!(denied_calls.load(Ordering::SeqCst), 0);
        assert!(scoped.get("mcp__other__read").is_none());

        let output = allowed
            .run(
                tool_context(Arc::new(
                    codeloom_core::permission::PermissionService::allow_all(),
                )),
                serde_json::json!({}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(!output.is_error);
        assert_eq!(allowed_calls.load(Ordering::SeqCst), 1);
        assert_eq!(denied_calls.load(Ordering::SeqCst), 0);

        drop(scoped);
        drop(registry);
        drop(allowed_client);
        drop(denied_client);
        allowed_server.abort();
        denied_server.abort();
    }

    #[tokio::test]
    async fn oversized_mcp_result_is_bounded_before_serialization_grows_output() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (client, tools, server) = fake_server("files", calls).await;
        let tool = tools.into_iter().next().unwrap();
        let output = tool
            .run(
                tool_context(Arc::new(
                    codeloom_core::permission::PermissionService::allow_all(),
                )),
                serde_json::json!({"large":true}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(output.content.len() < codeloom_core::tools::MAX_TOOL_OUTPUT_BYTES);
        assert!(output
            .content
            .contains("exceeded the 20000 byte output limit"));
        drop(client);
        server.abort();
    }

    #[tokio::test]
    async fn disconnected_fake_server_returns_a_sanitized_error() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (client, tools, server) = fake_server("files", calls).await;
        let tool = tools.into_iter().next().unwrap();
        server.abort();
        let error = tool
            .run(
                tool_context(Arc::new(
                    codeloom_core::permission::PermissionService::allow_all(),
                )),
                serde_json::json!({}),
                CancellationToken::new(),
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("MCP tool call failed"));
        assert!(!error.contains("CODELOOM_TEST_MISSING"));
        drop(client);
    }
}
