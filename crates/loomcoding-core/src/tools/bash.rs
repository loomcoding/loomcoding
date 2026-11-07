use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
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

fn shell_description() -> &'static str {
    if cfg!(windows) {
        "Run a command using Windows cmd.exe (/C), not Bash. Use cmd syntax; for PowerShell, invoke `powershell -NoProfile -Command ...`. Params: command (string)."
    } else {
        "Run a shell command using sh -c. Params: command (string)."
    }
}

/// Mata el proceso y todos sus descendientes. `kill_on_drop` solo alcanza al proceso directo: el shell
/// muere y su hijo (`cargo`, `ping`…) queda huérfano y vivo.
pub(crate) async fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    #[cfg(windows)]
    let mut killer = {
        let mut killer = tokio::process::Command::new("taskkill");
        killer.args(["/PID", &pid.to_string(), "/T", "/F"]);
        killer
    };
    // En Unix el shell lidera su propio grupo de procesos (`process_group(0)` al lanzarlo).
    #[cfg(not(windows))]
    let mut killer = {
        let mut killer = tokio::process::Command::new("kill");
        killer.args(["-KILL", "--", &format!("-{pid}")]);
        killer
    };
    let _ = killer
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
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
    // En Windows, `cmd.exe` no interpreta la línea de comandos con las reglas MSVCRT que aplica
    // `Command::arg`: al pasar el comando como argumento, Rust escapaba las comillas (`"` -> `\"`)
    // y cmd las recibía literales. Se entrega la línea cruda con `/s`, que hace que cmd quite solo
    // la primera y la última comilla y deje intacto el resto, incluidas las comillas internas.
    #[cfg(windows)]
    let mut process = {
        let mut process = tokio::process::Command::new("cmd");
        process.raw_arg(format!("/d /s /c \"{command}\""));
        process
    };
    #[cfg(not(windows))]
    let mut process = {
        let mut process = tokio::process::Command::new("sh");
        process.arg("-c").arg(command);
        process
    };
    #[cfg(unix)]
    process.process_group(0);
    process
        .current_dir(&ctx.cwd)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(env) = project_env {
        process.env_clear().envs(env);
    }
    let mut child = process
        .spawn()
        .map_err(|e| CoreError::Tool(e.to_string()))?;
    let pid = child.id();
    // El `child` debe seguir vivo mientras se mata el árbol: soltarlo antes mataría solo al shell y
    // `taskkill /T` ya no encontraría a sus descendientes.
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(CoreError::Tool("child pipes unavailable".into()));
    };
    let (mut out_bytes, mut err_bytes) = (Vec::new(), Vec::new());
    let status = tokio::select! {
        result = async {
            let (status, _, _) = tokio::join!(
                child.wait(),
                stdout.read_to_end(&mut out_bytes),
                stderr.read_to_end(&mut err_bytes),
            );
            status
        } => result.map_err(|e| CoreError::Tool(e.to_string()))?,
        _ = cancel.cancelled() => {
            kill_tree(pid).await;
            return Ok(super::ToolOutput::err("cancelled"));
        }
    };
    let mut text = String::from_utf8_lossy(&out_bytes).to_string();
    text.push_str(&String::from_utf8_lossy(&err_bytes));
    if status.success() {
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
        shell_description()
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"command":{"type":"string"},"raw":{"type":"boolean","description":"true devuelve la salida sin compactar"}},"required":["command"]})
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
        shell_description()
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
pub(crate) mod tests {
    use super::*;
    use crate::permission::PermissionService;
    use std::sync::Arc;

    #[test]
    fn shell_description_matches_the_host() {
        let description = BashTool.description();
        if cfg!(windows) {
            assert!(description.contains("cmd.exe"));
            assert!(description.contains("PowerShell"));
        } else {
            assert!(description.contains("sh -c"));
        }
    }

    fn ctx(dir: &std::path::Path) -> ToolCtx {
        ToolCtx {
            cwd: dir.to_path_buf(),
            store: None,
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
    async fn bash_preserves_quotes_in_the_command() {
        let dir = tempfile::tempdir().unwrap();
        // `echo` imprime su argumento tal cual. Si el proceso recibe `\"quoted\"` en vez de
        // `"quoted"`, las comillas se han escapado por el camino.
        let out = BashTool
            .run(
                ctx(dir.path()),
                serde_json::json!({ "command": "echo \"quoted\"" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            !out.content.contains("\\\""),
            "el comando llegó con comillas escapadas: {:?}",
            out.content
        );
        assert!(out.content.contains("\"quoted\""));
    }

    #[tokio::test]
    async fn bash_keeps_shell_operators_and_variables_working() {
        let dir = tempfile::tempdir().unwrap();
        // `/s` solo debe afectar al entrecomillado externo: los operadores y la expansión de
        // variables tienen que seguir funcionando igual.
        let out = BashTool
            .run(
                ctx(dir.path()),
                serde_json::json!({ "command": "echo uno & echo dos" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("uno"), "salida: {:?}", out.content);
        assert!(out.content.contains("dos"), "salida: {:?}", out.content);

        let out = BashTool
            .run(
                ctx(dir.path()),
                serde_json::json!({ "command": "echo %LOOMCODING_TEST_VAR%" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // Sin la variable definida, cmd deja el literal: lo importante es que no se rompa.
        assert!(!out.content.contains("\\\""), "salida: {:?}", out.content);
    }

    #[tokio::test]
    async fn bash_handles_nested_quotes() {
        let dir = tempfile::tempdir().unwrap();
        // Comillas dentro de comillas: el caso que más fácil rompe al reentrecomillar.
        let out = BashTool
            .run(
                ctx(dir.path()),
                serde_json::json!({ "command": "echo \"a b\" c" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            !out.content.contains("\\\""),
            "el comando llegó con comillas escapadas: {:?}",
            out.content
        );
        assert!(out.content.contains("a b"), "salida: {:?}", out.content);
    }

    #[tokio::test]
    async fn project_bash_only_receives_the_supplied_project_environment() {
        let host_key = format!("LOOMCODING_HOST_SECRET_{}", std::process::id());
        let project_key = format!("LOOMCODING_PROJECT_VALUE_{}", std::process::id());
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

    /// Cuántos procesos de la prueba siguen vivos (el comando lleva un plazo poco habitual, 37, que los distingue).
    pub(crate) fn survivors() -> usize {
        #[cfg(windows)]
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "@(Get-CimInstance Win32_Process -Filter \"Name='PING.EXE'\" | Where-Object { $_.CommandLine -match '-n 37 ' }).Count",
            ])
            .output();
        #[cfg(not(windows))]
        let output = std::process::Command::new("sh")
            .args(["-c", "pgrep -f 'sleep 37' | wc -l"])
            .output();
        let output = output.expect("no se pudo contar los procesos");
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn cancelling_a_bash_call_leaves_no_process_alive() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let command = "ping -n 37 127.0.0.1 >nul";
        #[cfg(not(windows))]
        let command = "sleep 37";
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let cwd = dir.path().to_path_buf();
        let task = tokio::spawn(async move {
            BashTool
                .run(ctx(&cwd), serde_json::json!({ "command": command }), token)
                .await
        });
        // Sin esperar a que arranque el proceso la prueba sería vacua: debe existir antes de cancelar.
        let started = std::time::Instant::now();
        while survivors() == 0 {
            assert!(started.elapsed().as_secs() < 10, "el proceso nunca arrancó");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        cancel.cancel();
        let out = task.await.unwrap().unwrap();
        assert!(out.is_error && out.content.contains("cancelled"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while survivors() > 0 && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(survivors(), 0, "la cancelación dejó procesos vivos");
    }
}
