use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const BANNED: [&str; 3] = ["rm -rf /", "mkfs", ":(){"];

pub struct BashTool;

/// Bash tool for the local server: the child process receives only this project's environment.
pub struct ProjectBashTool {
    env: Arc<HashMap<String, String>>,
}

impl ProjectBashTool {
    pub fn new(env: HashMap<String, String>) -> Self {
        Self { env: Arc::new(env) }
    }
}

async fn run_command(
    ctx: super::ToolCtx,
    input: serde_json::Value,
    cancel: CancellationToken,
    project_env: Option<&HashMap<String, String>>,
) -> Result<super::ToolOutput> {
    let command = input["command"].as_str().unwrap_or_default();
    if BANNED.iter().any(|b| command.contains(b)) {
        return Ok(super::ToolOutput::err("command blocked by policy"));
    }
    let (prog, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let mut process = tokio::process::Command::new(prog);
    process
        .arg(flag)
        .arg(command)
        .current_dir(&ctx.cwd)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(env) = project_env {
        process.env_clear().envs(env);
    }
    let child = process
        .spawn()
        .map_err(|e| CoreError::Tool(e.to_string()))?;
    let out = tokio::select! {
        out = child.wait_with_output() => out.map_err(|e| CoreError::Tool(e.to_string()))?,
        _ = cancel.cancelled() => return Ok(super::ToolOutput::err("cancelled")),
    };
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(super::ToolOutput::ok(text))
    } else {
        Ok(super::ToolOutput::err(text))
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }
    fn description(&self) -> &str {
        "Run a shell command in the workspace. Params: command (string)."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]})
    }
    fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        run_command(ctx, input, cancel, None).await
    }
}

#[async_trait]
impl Tool for ProjectBashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Run a shell command in the workspace with that project's environment. Params: command (string)."
    }

    fn schema(&self) -> serde_json::Value {
        BashTool.schema()
    }

    fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }

    async fn run(
        &self,
        ctx: super::ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        run_command(ctx, input, cancel, Some(&self.env)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionService;
    use std::sync::Arc;

    fn ctx(dir: &std::path::Path) -> ToolCtx {
        ToolCtx {
            cwd: dir.to_path_buf(),
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        }
    }

    #[tokio::test]
    async fn bash_runs_command() {
        let dir = tempfile::tempdir().unwrap();
        let out = BashTool
            .run(
                ctx(dir.path()),
                serde_json::json!({ "command": "echo hi" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("hi"));
    }

    #[tokio::test]
    async fn project_bash_only_receives_the_supplied_project_environment() {
        let host_key = format!("CODELOOM_HOST_SECRET_{}", std::process::id());
        let project_key = format!("CODELOOM_PROJECT_VALUE_{}", std::process::id());
        let previous = std::env::var_os(&host_key);
        std::env::set_var(&host_key, "host-secret");
        let mut env = HashMap::from([(project_key.clone(), "project-only".to_string())]);
        for key in [
            "PATH",
            "PATHEXT",
            "SYSTEMROOT",
            "WINDIR",
            "COMSPEC",
            "TEMP",
            "TMP",
        ] {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.into(), value);
            }
        }
        let command = if cfg!(windows) {
            format!("echo %{project_key}% %{host_key}%")
        } else {
            format!("printf '%s %s' \"${project_key}\" \"${host_key}\"")
        };
        let dir = tempfile::tempdir().unwrap();
        let output = ProjectBashTool::new(env)
            .run(
                ctx(dir.path()),
                serde_json::json!({ "command": command }),
                CancellationToken::new(),
            )
            .await;
        if let Some(previous) = previous {
            std::env::set_var(&host_key, previous);
        } else {
            std::env::remove_var(&host_key);
        }

        let output = output.unwrap();
        assert!(output.content.contains("project-only"));
        assert!(!output.content.contains("host-secret"));
    }
}
