use super::{Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

const MAX_ENTRIES: usize = 200;

pub struct LsTool;

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }
    fn description(&self) -> &str {
        "List non-ignored entries in a workspace directory. Params: path (optional, defaults to .)."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"path":{"type":"string"}}})
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        _cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let path = input["path"].as_str().unwrap_or(".");
        let full = ctx.cwd.join(path);
        let cwd = std::fs::canonicalize(&ctx.cwd).map_err(|e| CoreError::Tool(e.to_string()))?;
        let dir =
            std::fs::canonicalize(&full).map_err(|e| CoreError::Tool(format!("ls {path}: {e}")))?;
        if !dir.starts_with(&cwd) || !dir.is_dir() {
            return Ok(ToolOutput::err("directory must be inside the workspace"));
        }
        let walk = ignore::WalkBuilder::new(&dir)
            .max_depth(Some(1))
            .hidden(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build();
        let mut entries = Vec::new();
        let mut total = 0usize;
        for entry in walk.flatten().skip(1).take(MAX_ENTRIES + 1) {
            total += 1;
            if entries.len() < MAX_ENTRIES {
                let kind = if entry.file_type().is_some_and(|t| t.is_dir()) {
                    "dir"
                } else {
                    "file"
                };
                entries.push(format!(
                    "{kind}\t{}",
                    entry
                        .path()
                        .strip_prefix(&cwd)
                        .unwrap_or(entry.path())
                        .display()
                ));
            }
        }
        if total > MAX_ENTRIES {
            entries.push(format!(
                "[{MAX_ENTRIES} entries shown; more entries omitted]"
            ));
        }
        Ok(ToolOutput::ok(
            super::limit_output(&entries.join("\n")).into_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn lists_direct_entries_but_not_ignored_ones() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "ignored\n").unwrap();
        std::fs::write(dir.path().join("visible"), "").unwrap();
        std::fs::write(dir.path().join("ignored"), "").unwrap();
        let ctx = ToolCtx {
            cwd: dir.path().into(),
            store: None,
            permission: std::sync::Arc::new(crate::permission::PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: std::sync::Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        let out = LsTool
            .run(ctx, serde_json::json!({}), CancellationToken::new())
            .await
            .unwrap();
        assert!(out.content.contains("visible"));
        assert!(!out.content.contains("ignored\t"));
    }
    #[tokio::test]
    async fn rejects_paths_outside_workspace() {
        let workspace_dir = tempfile::tempdir().unwrap();
        let external_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            external_dir.path().join("sentinel.txt"),
            "this file must not appear",
        )
        .unwrap();
        let ctx = ToolCtx {
            cwd: workspace_dir.path().into(),
            store: None,
            permission: std::sync::Arc::new(crate::permission::PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: std::sync::Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        let out = LsTool
            .run(
                ctx,
                serde_json::json!({"path": external_dir.path().to_str().unwrap()}),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
        assert!(!out.content.contains("sentinel.txt"));
    }
}
