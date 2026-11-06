use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct JobListTool;

#[async_trait]
impl Tool for JobListTool {
    fn name(&self) -> &str {
        "job_list"
    }

    fn description(&self) -> &str {
        "List background jobs owned by the current session."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Parallel
    }

    async fn run(
        &self,
        ctx: ToolCtx,
        _: serde_json::Value,
        _: CancellationToken,
    ) -> Result<ToolOutput> {
        let jobs: Vec<_> = ctx
            .jobs
            .list(ctx.session)
            .into_iter()
            .map(|job| {
                serde_json::json!({
                    "id": job.id.0,
                    "kind": job.kind,
                    "label": job.label,
                    "status": format!("{:?}", job.status).to_lowercase(),
                    "progress": job.progress.map(|p| serde_json::json!({
                        "fraction": p.fraction,
                        "label": p.label
                    })),
                    "output_end": job.output_end,
                    "detail": job.detail,
                    "watch": job.watch,
                })
            })
            .collect();
        Ok(ToolOutput::ok(serde_json::to_string(&jobs)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobRegistry, JobSpec, LocalJobRegistry};
    use crate::job_watch::WatchSpec;
    use crate::permission::PermissionService;
    use crate::session::SessionId;
    use std::sync::Arc;

    #[tokio::test]
    async fn listing_shows_the_watch_of_each_job() {
        let jobs = Arc::new(LocalJobRegistry::default());
        jobs.start(JobSpec {
            kind: "shell".into(),
            label: "vigilado".into(),
            owner: SessionId(31),
            output_limit: 1024,
            watch: Some(WatchSpec::parse("panic|FAILED", Some(7)).unwrap()),
            run: Box::new(|_, cancel| {
                Box::pin(async move {
                    cancel.cancelled().await;
                    Ok(None)
                })
            }),
        })
        .unwrap();
        let ctx = ToolCtx {
            cwd: std::env::current_dir().unwrap(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: SessionId(31),
            jobs: jobs.clone(),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        let output = JobListTool
            .run(ctx, serde_json::json!({}), CancellationToken::new())
            .await
            .unwrap();
        let listed: serde_json::Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(listed[0]["watch"]["pattern"], "panic|FAILED");
        assert_eq!(listed[0]["watch"]["limit_reached"], false);
    }
}
