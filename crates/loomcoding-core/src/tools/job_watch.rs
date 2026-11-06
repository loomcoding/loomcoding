use super::job_start::watch_spec;
use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::job::JobId;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

/// Añade, cambia o quita la vigilancia de un job en marcha.
pub struct JobWatchTool;

#[async_trait]
impl Tool for JobWatchTool {
    fn name(&self) -> &str {
        "job_watch"
    }

    fn description(&self) -> &str {
        "Add, change or remove the output watch of a running job owned by the current session. Params: job_id, optional pattern (regex; omitted or empty removes the watch) and optional max_events (1 to 100, default 20). Only lines printed after this call are watched; use job_output to read earlier output."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "job_id": { "type": "string" },
                "pattern": { "type": "string", "maxLength": 256 },
                "max_events": { "type": "integer", "minimum": 1, "maximum": 100 }
            },
            "required": ["job_id"]
        })
    }

    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }

    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        _: CancellationToken,
    ) -> Result<ToolOutput> {
        let id = input["job_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| CoreError::Tool("job_id is required".into()))?;
        let spec = match input.get("pattern") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(pattern)) if pattern.is_empty() => None,
            Some(serde_json::Value::String(pattern)) => {
                Some(watch_spec(pattern, &input["max_events"])?)
            }
            Some(_) => return Err(CoreError::Tool("pattern must be a string".into())),
        };
        let removing = spec.is_none();
        ctx.jobs.set_watch(ctx.session, &JobId(id.into()), spec)?;
        Ok(ToolOutput::ok(if removing {
            format!("Watch removed from job {id}.")
        } else {
            format!("Watching job {id} from now on.")
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobRegistry, JobSpec, JobStatus, LocalJobRegistry};
    use crate::permission::PermissionService;
    use crate::session::SessionId;
    use std::sync::Arc;

    fn ctx(jobs: Arc<LocalJobRegistry>, session: i64) -> ToolCtx {
        ToolCtx {
            cwd: std::env::current_dir().unwrap(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: SessionId(session),
            jobs,
            bus: crate::event::EventBus::new(8),
            subagents: None,
        }
    }

    /// Job que sigue vivo hasta `kill`: el caso típico de «job en marcha».
    fn running(jobs: &LocalJobRegistry, owner: i64, watch: Option<&str>) -> JobId {
        let job = jobs
            .start(JobSpec {
                kind: "shell".into(),
                label: "en marcha".into(),
                owner: SessionId(owner),
                output_limit: 1024,
                watch: watch.map(|p| crate::job_watch::WatchSpec::parse(p, None).unwrap()),
                run: Box::new(|_, cancel| {
                    Box::pin(async move {
                        cancel.cancelled().await;
                        Ok(None)
                    })
                }),
            })
            .unwrap();
        job.id().clone()
    }

    fn pattern_of(jobs: &LocalJobRegistry, owner: i64, id: &JobId) -> Option<String> {
        jobs.list(SessionId(owner))
            .into_iter()
            .find(|job| job.id == *id)
            .and_then(|job| job.watch.map(|w| w.pattern))
    }

    #[tokio::test]
    async fn adds_a_watch_to_a_running_job() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let id = running(&jobs, 41, None);
        let output = JobWatchTool
            .run(
                ctx(jobs.clone(), 41),
                serde_json::json!({ "job_id": id.0, "pattern": "panic|FAILED", "max_events": 5 }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(output.content.contains("Watching job"));
        assert_eq!(pattern_of(&jobs, 41, &id).as_deref(), Some("panic|FAILED"));
        jobs.kill(SessionId(41), &id).unwrap();
    }

    #[tokio::test]
    async fn empty_or_missing_pattern_removes_the_watch() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let id = running(&jobs, 42, Some("FAIL"));
        for input in [
            serde_json::json!({ "job_id": id.0 }),
            serde_json::json!({ "job_id": id.0, "pattern": "" }),
        ] {
            JobWatchTool
                .run(
                    ctx(jobs.clone(), 42),
                    input.clone(),
                    CancellationToken::new(),
                )
                .await
                .unwrap();
            assert!(pattern_of(&jobs, 42, &id).is_none(), "{input}");
            // Lo siguiente vuelve a ponerla para la segunda iteración.
            jobs.set_watch(
                SessionId(42),
                &id,
                Some(crate::job_watch::WatchSpec::parse("FAIL", None).unwrap()),
            )
            .unwrap();
        }
        jobs.kill(SessionId(42), &id).unwrap();
    }

    #[tokio::test]
    async fn invalid_pattern_is_an_error_and_changes_nothing() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let id = running(&jobs, 43, Some("FAIL"));
        let result = JobWatchTool
            .run(
                ctx(jobs.clone(), 43),
                serde_json::json!({ "job_id": id.0, "pattern": "error[" }),
                CancellationToken::new(),
            )
            .await;
        assert!(result.is_err());
        assert_eq!(pattern_of(&jobs, 43, &id).as_deref(), Some("FAIL"));
        jobs.kill(SessionId(43), &id).unwrap();
    }

    #[tokio::test]
    async fn finished_or_foreign_jobs_are_errors() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let id = running(&jobs, 44, None);
        jobs.kill(SessionId(44), &id).unwrap();
        jobs.wait(SessionId(44), &id).await.unwrap();
        let finished = JobWatchTool
            .run(
                ctx(jobs.clone(), 44),
                serde_json::json!({ "job_id": id.0, "pattern": "FAIL" }),
                CancellationToken::new(),
            )
            .await;
        assert!(finished.is_err());
        assert_eq!(jobs.list(SessionId(44))[0].status, JobStatus::Killed);

        let live = running(&jobs, 45, None);
        let foreign = JobWatchTool
            .run(
                ctx(jobs.clone(), 46),
                serde_json::json!({ "job_id": live.0, "pattern": "FAIL" }),
                CancellationToken::new(),
            )
            .await;
        assert!(foreign.is_err());
        jobs.kill(SessionId(45), &live).unwrap();
    }

    #[tokio::test]
    async fn missing_job_id_or_non_string_pattern_is_an_error() {
        let jobs = Arc::new(LocalJobRegistry::default());
        for input in [
            serde_json::json!({ "pattern": "FAIL" }),
            serde_json::json!({ "job_id": "shell-1", "pattern": 7 }),
        ] {
            let result = JobWatchTool
                .run(ctx(jobs.clone(), 47), input, CancellationToken::new())
                .await;
            assert!(result.is_err());
        }
    }
}
