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
}

#[derive(Default)]
pub struct McpRuntime {
    tools: Vec<Arc<dyn Tool>>,
    issues: Vec<McpIssue>,
    // Holding each service keeps the configured child process alive for its tools.
    _clients: Vec<Arc<Client>>,
}

#[derive(Debug, Clone)]
pub struct McpIssue {
    pub server_id: String,
    pub reason: &'static str,
}

impl McpRuntime {
    pub async fn start(config: McpConfig, cwd: &Path) -> Self {
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
                    runtime._clients.push(client);
                    runtime.tools.extend(tools);
                }
                Ok(Err((id, _))) => {
                    tracing::warn!(server = %id, "MCP server unavailable");
                    runtime.issues.push(McpIssue {
                        server_id: id,
                        reason: "could not start or list tools",
                    });
                }
                Err(_) => tracing::warn!("MCP connection task failed"),
            }
        }
        runtime
    }

    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }

    pub fn issues(&self) -> &[McpIssue] {
        &self.issues
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
            server_id: id.clone(),
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
    Ok((id, client, tools))
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
        .map_err(|error| {
            codeloom_core::error::CoreError::Tool(format!("MCP call failed: {error}"))
        })?;
        let content = serde_json::to_string(&result)
            .unwrap_or_else(|_| "MCP returned an unrenderable result".into());
        let content = codeloom_core::tools::limit_output(&content).into_owned();
        Ok(if result.is_error.unwrap_or(false) {
            ToolOutput::err(content)
        } else {
            ToolOutput::ok(content)
        })
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
    use std::sync::atomic::{AtomicU64, Ordering};

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
}
