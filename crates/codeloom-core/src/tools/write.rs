use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::{
    error::{CoreError, Result},
    permission::{Decision, PermissionRequest},
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "Create a new file in the workspace; refuses to overwrite. Params: path, content."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]})
    }
    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let path = input["path"].as_str().unwrap_or_default();
        let content = input["content"].as_str().unwrap_or_default();
        let relative = std::path::Path::new(path);
        if relative.as_os_str().is_empty() || relative.file_name().is_none() {
            return Ok(ToolOutput::err("invalid file path"));
        }
        if content.len() > super::MAX_TOOL_OUTPUT_BYTES * 100 {
            return Ok(ToolOutput::err("file content exceeds 2 MB"));
        }
        let cwd = std::fs::canonicalize(&ctx.cwd).map_err(|e| CoreError::Tool(e.to_string()))?;
        let parent =
            std::fs::canonicalize(cwd.join(relative.parent().unwrap_or(std::path::Path::new("."))))
                .map_err(|e| CoreError::Tool(format!("parent directory: {e}")))?;
        if !parent.starts_with(&cwd) {
            return Ok(ToolOutput::err("path must be inside the workspace"));
        }
        let dest = parent.join(relative.file_name().unwrap());
        let req = PermissionRequest {
            tool: "write".into(),
            action: "create".into(),
            detail: path.into(),
        };
        if tokio::select! { _ = cancel.cancelled() => return Ok(ToolOutput::err("cancelled")), d = ctx.permission.check(req) => d }
            == Decision::Deny
        {
            return Ok(ToolOutput::err("permission denied"));
        }
        let mut file = match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)
            .await
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Ok(ToolOutput::err(format!("refusing to overwrite {path}")))
            }
            Err(e) => return Err(CoreError::Tool(format!("write {path}: {e}"))),
        };
        use tokio::io::AsyncWriteExt;
        if let Err(e) = file.write_all(content.as_bytes()).await {
            let _ = tokio::fs::remove_file(&dest).await;
            return Err(CoreError::Tool(format!("write {path}: {e}")));
        }
        Ok(ToolOutput::ok(format!("created {path}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn creates_but_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = || ToolCtx {
            cwd: dir.path().into(),
            store: None,
            permission: std::sync::Arc::new(crate::permission::PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: std::sync::Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        let input = serde_json::json!({"path":"new.txt","content":"first"});
        assert!(
            !WriteTool
                .run(ctx(), input.clone(), CancellationToken::new())
                .await
                .unwrap()
                .is_error
        );
        assert!(
            WriteTool
                .run(ctx(), input, CancellationToken::new())
                .await
                .unwrap()
                .is_error
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "first"
        );
    }
}
