//! Herramienta `context`: el contexto estructural de un símbolo en una sola llamada.

use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::context::cache::index_for;
use crate::context::pack::{compose, PackOptions};
use crate::error::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct ContextTool;

#[async_trait]
impl Tool for ContextTool {
    fn name(&self) -> &str {
        "context"
    }
    fn description(&self) -> &str {
        "Structural context of a Rust symbol in ONE call: signature, location, body excerpt, callers, \
         tests and the matching CODE-MAP section. Use it BEFORE grep/view to locate code. \
         Params: query (symbol name, `Type::method` or partial text), budget (max characters, optional)."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "budget": { "type": "integer" }
            },
            "required": ["query"]
        })
    }
    fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Parallel
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        if query.is_empty() {
            return Ok(ToolOutput::err("query is required"));
        }
        let mut opts = PackOptions::default();
        if let Some(budget) = input["budget"].as_u64() {
            opts.max_chars = budget as usize;
        }
        let root = ctx.cwd.clone();
        // Indexar es trabajo de CPU: fuera del ejecutor asíncrono y respetando la cancelación.
        let job = tokio::task::spawn_blocking(move || {
            let index = index_for(&root);
            compose(&index, &root, &query, &opts)
        });
        tokio::select! {
            _ = cancel.cancelled() => Ok(ToolOutput::err("cancelled")),
            packed = job => match packed {
                Ok(text) => Ok(ToolOutput::ok(text)),
                Err(error) => Ok(ToolOutput::err(format!("context failed: {error}"))),
            },
        }
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
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        }
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("lib.rs"),
            "pub fn alfa() -> u32 {\n    1\n}\npub fn usa() {\n    alfa();\n}\n",
        )
        .unwrap();
        dir
    }

    async fn run(dir: &std::path::Path, input: serde_json::Value) -> ToolOutput {
        ContextTool
            .run(ctx(dir), input, CancellationToken::new())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn returns_a_pack_for_an_existing_symbol() {
        let dir = project();
        let out = run(dir.path(), serde_json::json!({"query": "alfa"})).await;
        assert!(!out.is_error);
        assert!(out.content.contains("lib.rs:1"), "{}", out.content);
        assert!(out.content.contains("usos: lib.rs:5"), "{}", out.content);
    }

    #[tokio::test]
    async fn reports_when_nothing_matches() {
        let dir = project();
        let out = run(dir.path(), serde_json::json!({"query": "zzz_no_existe"})).await;
        assert!(out.content.contains("no symbol"), "{}", out.content);
    }

    #[tokio::test]
    async fn a_missing_query_is_an_error() {
        let dir = project();
        let out = run(dir.path(), serde_json::json!({})).await;
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn the_budget_bounds_the_answer() {
        let dir = project();
        let out = run(dir.path(), serde_json::json!({"query": "a", "budget": 400})).await;
        assert!(out.content.chars().count() <= 400, "{}", out.content);
    }

    #[test]
    fn it_is_registered_in_the_project_registry() {
        let registry = crate::tools::project_registry(std::collections::HashMap::new());
        assert!(registry.specs().iter().any(|spec| spec.name == "context"));
    }
}
