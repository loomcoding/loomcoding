use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::job::{JobResult, JobSpec};
use crate::job_watch::{ChunkDecoder, WatchSpec};
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct JobStartTool;

pub(crate) fn track_job(ctx: &ToolCtx, id: &crate::job::JobId) -> Result<()> {
    let mut updates = ctx.jobs.subscribe(ctx.session, id)?;
    let initial = updates.borrow_and_update().clone();
    let settled = |status| {
        matches!(
            status,
            crate::job::JobStatus::Completed
                | crate::job::JobStatus::Killed
                | crate::job::JobStatus::Failed
        )
    };
    let bus = ctx.bus.clone();
    bus.publish(crate::event::LiveEvent::JobChanged(initial.clone()));
    if !settled(initial.status) {
        tokio::spawn(async move {
            while updates.changed().await.is_ok() {
                let snapshot = updates.borrow_and_update().clone();
                let is_settled = settled(snapshot.status);
                bus.publish(crate::event::LiveEvent::JobChanged(snapshot));
                if is_settled {
                    break;
                }
            }
        });
    }
    Ok(())
}

/// Construye la vigilancia desde `pattern` y `max_events` (de `job_start` o `job_watch`).
pub(crate) fn watch_spec(pattern: &str, max_events: &serde_json::Value) -> Result<WatchSpec> {
    let max_events = match max_events {
        serde_json::Value::Null => None,
        value => Some(
            value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| {
                    CoreError::Tool("watch max_events must be an integer from 1 to 100".into())
                })?,
        ),
    };
    WatchSpec::parse(pattern, max_events)
}

fn forward_output<R: AsyncRead + Unpin + Send + 'static>(mut stream: R, tx: mpsc::Sender<Vec<u8>>) {
    tokio::spawn(async move {
        let mut buf = [0; 4096];
        loop {
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) if tx.send(buf[..n].to_vec()).await.is_err() => break,
                Ok(_) => {}
            }
        }
    });
}

#[async_trait]
impl Tool for JobStartTool {
    fn name(&self) -> &str {
        "job_start"
    }

    fn description(&self) -> &str {
        "Start a shell command as a background job. Params: command and optional label. Optional watch: {pattern, max_events?}, a regex whose matching output lines are sent to you as notices while the job runs (max_events: 1 to 100, default 20). Use job_list/job_output/job_watch/job_kill to manage it."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "minLength": 1, "maxLength": 16384 },
                "label": { "type": "string", "maxLength": 128 },
                "watch": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "minLength": 1, "maxLength": 256 },
                        "max_events": { "type": "integer", "minimum": 1, "maximum": 100 }
                    },
                    "required": ["pattern"]
                }
            },
            "required": ["command"]
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
        let command = input["command"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 16_384)
            .ok_or_else(|| CoreError::Tool("command must be 1 to 16384 bytes".into()))?
            .to_string();
        let label = input["label"]
            .as_str()
            .filter(|s| s.len() <= 128)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("background command")
            .to_string();
        // La validación de la vigilancia va antes de arrancar: un patrón malo no deja nada corriendo.
        let watch = match &input["watch"] {
            serde_json::Value::Null => None,
            params => Some(watch_spec(
                params["pattern"].as_str().unwrap_or(""),
                &params["max_events"],
            )?),
        };
        let watch_note = watch
            .as_ref()
            .map(|w| {
                format!(
                    " Watching output for /{}/ (up to {} notices).",
                    w.pattern.as_str(),
                    w.max_events
                )
            })
            .unwrap_or_default();
        let cwd = ctx.cwd.clone();
        let owner = ctx.session;
        let job = ctx.jobs.start(JobSpec {
            kind: "shell".into(),
            label,
            owner,
            output_limit: 64 * 1024,
            watch,
            run: Box::new(move |handle, cancel| {
                Box::pin(async move {
                    let (program, flag) = if cfg!(windows) {
                        ("cmd", "/C")
                    } else {
                        ("sh", "-c")
                    };
                    let mut process = tokio::process::Command::new(program);
                    process
                        .arg(flag)
                        .arg(command)
                        .current_dir(cwd)
                        .kill_on_drop(true)
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped());
                    // El shell lidera su propio grupo para poder matar también a sus descendientes.
                    #[cfg(unix)]
                    process.process_group(0);
                    let mut child = process
                        .spawn()
                        .map_err(|e| CoreError::Tool(e.to_string()))?;
                    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(8);
                    if let Some(stdout) = child.stdout.take() {
                        forward_output(stdout, tx.clone());
                    }
                    if let Some(stderr) = child.stderr.take() {
                        forward_output(stderr, tx.clone());
                    }
                    drop(tx);
                    let mut decoder = ChunkDecoder::default();
                    let status = loop {
                        tokio::select! {
                            result = child.wait() => break result.map_err(|e| CoreError::Tool(e.to_string()))?,
                            chunk = rx.recv() => if let Some(chunk) = chunk {
                                handle.append(&decoder.decode(&chunk));
                            },
                            _ = cancel.cancelled() => {
                                super::bash::kill_tree(child.id()).await;
                                let _ = child.kill().await;
                                return Ok(None);
                            }
                        }
                    };
                    while let Some(chunk) = rx.recv().await {
                        handle.append(&decoder.decode(&chunk));
                    }
                    let rest = decoder.finish();
                    if !rest.is_empty() {
                        handle.append(&rest);
                    }
                    if status.success() {
                        Ok(Some(JobResult { content: "exit 0".into() }))
                    } else {
                        Err(CoreError::Tool(format!("command exited with {status}")))
                    }
                })
            }),
        })?;
        track_job(&ctx, job.id())?;
        Ok(ToolOutput::ok(format!(
            "Started background job {}.{watch_note}",
            job.id().0
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobRegistry, LocalJobRegistry};
    use crate::permission::PermissionService;
    use crate::session::SessionId;
    use std::sync::Arc;

    #[tokio::test]
    async fn starts_command_and_retains_its_output() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let bus = crate::event::EventBus::new(8);
        let mut events = bus.subscribe();
        let ctx = ToolCtx {
            cwd: std::env::current_dir().unwrap(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: SessionId(12),
            jobs: jobs.clone(),
            bus,
            subagents: None,
        };
        let started = JobStartTool
            .run(
                ctx,
                serde_json::json!({ "command": "echo m2-job-test" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let id = started
            .content
            .split_whitespace()
            .nth(3)
            .unwrap()
            .trim_end_matches('.');
        let event = events.recv().await.unwrap();
        assert!(
            matches!(event, crate::event::LiveEvent::JobChanged(snapshot) if snapshot.id.0 == id)
        );
        let outcome = jobs
            .wait(SessionId(12), &crate::job::JobId(id.into()))
            .await
            .unwrap();
        assert_eq!(outcome.status, crate::job::JobStatus::Completed);
        let output = jobs
            .read(SessionId(12), &crate::job::JobId(id.into()), 0)
            .unwrap();
        assert!(output.text.contains("m2-job-test"));
    }

    #[tokio::test]
    async fn killing_a_background_job_leaves_no_process_alive() {
        use crate::tools::bash::tests::survivors;
        let jobs = Arc::new(LocalJobRegistry::default());
        let ctx = ToolCtx {
            cwd: std::env::current_dir().unwrap(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: SessionId(13),
            jobs: jobs.clone(),
            bus: crate::event::EventBus::new(8),
            subagents: None,
        };
        #[cfg(windows)]
        let command = "ping -n 37 127.0.0.1 >nul";
        #[cfg(not(windows))]
        let command = "sleep 37";
        let started = JobStartTool
            .run(
                ctx,
                serde_json::json!({ "command": command }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let id = crate::job::JobId(
            started
                .content
                .split_whitespace()
                .nth(3)
                .unwrap()
                .trim_end_matches('.')
                .into(),
        );
        let began = std::time::Instant::now();
        while survivors() == 0 {
            assert!(began.elapsed().as_secs() < 10, "el proceso nunca arrancó");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        jobs.kill(SessionId(13), &id).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while survivors() > 0 && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(survivors(), 0, "matar el job dejó procesos vivos");
    }

    fn ctx_for(jobs: Arc<LocalJobRegistry>, session: i64) -> ToolCtx {
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

    #[tokio::test]
    async fn invalid_watch_pattern_fails_before_starting_the_command() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let result = JobStartTool
            .run(
                ctx_for(jobs.clone(), 21),
                serde_json::json!({ "command": "echo nunca", "watch": { "pattern": "error[" } }),
                CancellationToken::new(),
            )
            .await;
        assert!(result.is_err());
        assert!(jobs.list(SessionId(21)).is_empty());
    }

    #[tokio::test]
    async fn watch_is_registered_and_catches_the_output_of_the_job() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let started = JobStartTool
            .run(
                ctx_for(jobs.clone(), 22),
                serde_json::json!({
                    "command": "echo FAIL-printed",
                    "watch": { "pattern": "FAIL", "max_events": 5 }
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(started.content.contains("Watching output for /FAIL/"));
        let id = crate::job::JobId(
            started
                .content
                .split_whitespace()
                .nth(3)
                .unwrap()
                .trim_end_matches('.')
                .into(),
        );
        jobs.wait(SessionId(22), &id).await.unwrap();
        let batches = jobs.take_watch_batches(SessionId(22), std::time::Instant::now());
        assert_eq!(batches[0].1.lines, vec!["FAIL-printed".to_string()]);
        let listed = jobs.list(SessionId(22));
        assert_eq!(listed[0].watch.as_ref().unwrap().pattern, "FAIL");
    }

    #[tokio::test]
    async fn job_without_watch_has_no_watch_state() {
        let jobs = Arc::new(LocalJobRegistry::default());
        let started = JobStartTool
            .run(
                ctx_for(jobs.clone(), 23),
                serde_json::json!({ "command": "echo FAIL-but-unwatched" }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(!started.content.contains("Watching"));
        let id = crate::job::JobId(
            started
                .content
                .split_whitespace()
                .nth(3)
                .unwrap()
                .trim_end_matches('.')
                .into(),
        );
        jobs.wait(SessionId(23), &id).await.unwrap();
        assert!(jobs.list(SessionId(23))[0].watch.is_none());
        assert!(jobs
            .take_watch_batches(SessionId(23), std::time::Instant::now())
            .is_empty());
    }
}
