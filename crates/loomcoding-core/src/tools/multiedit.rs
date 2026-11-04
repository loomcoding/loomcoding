use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::{
    error::{CoreError, Result},
    permission::{Decision, PermissionRequest},
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct MultiEditTool;

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "multiedit"
    }
    fn description(&self) -> &str {
        "Apply 1-20 exact unique replacements to one file atomically after validation. Params: path, edits [{old,new}]."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"edits":{"type":"array","minItems":1,"maxItems":20,"items":{"type":"object","properties":{"old":{"type":"string"},"new":{"type":"string"}},"required":["old","new"]}}},"required":["path","edits"]})
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
        let Some(edits) = input["edits"]
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= 20)
        else {
            return Ok(ToolOutput::err("edits must contain 1 to 20 replacements"));
        };
        let cwd = std::fs::canonicalize(&ctx.cwd).map_err(|e| CoreError::Tool(e.to_string()))?;
        let full = std::fs::canonicalize(cwd.join(path))
            .map_err(|e| CoreError::Tool(format!("multiedit {path}: {e}")))?;
        if !full.starts_with(&cwd) || !full.is_file() {
            return Ok(ToolOutput::err("file must be inside the workspace"));
        }
        let original = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| CoreError::Tool(format!("multiedit {path}: {e}")))?;
        let mut updated = original.clone();
        for (i, edit) in edits.iter().enumerate() {
            let old =
                super::match_line_endings(edit["old"].as_str().unwrap_or_default(), &original);
            let new =
                super::match_line_endings(edit["new"].as_str().unwrap_or_default(), &original);
            let count = updated.matches(&old).count();
            if old.is_empty() || count != 1 {
                return Ok(ToolOutput::err(format!(
                    "edit {} expected one match, found {count}; file unchanged",
                    i + 1
                )));
            }
            updated = updated.replacen(&old, &new, 1);
        }
        let req = PermissionRequest {
            tool: "multiedit".into(),
            action: "write".into(),
            detail: path.into(),
        };
        if tokio::select! { _ = cancel.cancelled() => return Ok(ToolOutput::err("cancelled")), d = ctx.permission.check(req) => d }
            == Decision::Deny
        {
            return Ok(ToolOutput::err("permission denied"));
        }
        tokio::fs::write(full, updated)
            .await
            .map_err(|e| CoreError::Tool(format!("multiedit {path}: {e}")))?;
        Ok(ToolOutput::ok(format!(
            "applied {} edits to {path}",
            edits.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn validates_all_replacements_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one two").unwrap();
        let ctx = ToolCtx {
            cwd: dir.path().into(),
            store: None,
            permission: std::sync::Arc::new(crate::permission::PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: std::sync::Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        let out = MultiEditTool.run(ctx, serde_json::json!({"path":"a.txt","edits":[{"old":"one","new":"1"},{"old":"missing","new":"2"}]}), CancellationToken::new()).await.unwrap();
        assert!(out.is_error);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "one two"
        );
    }
}
