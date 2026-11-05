use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::job::{JobResult, JobSpec};
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
        "Start a shell command as a background job. Params: command and optional label. Use job_list/job_output/job_kill to manage it."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "minLength": 1, "maxLength": 16384 },
                "label": { "type": "string", "maxLength": 128 }
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
        let cwd = ctx.cwd.clone();
        let owner = ctx.session;
        let job = ctx.jobs.start(JobSpec {
            kind: "shell".into(),
            label,
            owner,
            output_limit: 64 * 1024,
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
                    let status = loop {
                        tokio::select! {
                            result = child.wait() => break result.map_err(|e| CoreError::Tool(e.to_string()))?,
                            chunk = rx.recv() => if let Some(chunk) = chunk {
                                handle.append(&String::from_utf8_lossy(&chunk));
                            },
                            _ = cancel.cancelled() => {
                                super::bash::kill_tree(child.id()).await;
                                let _ = child.kill().await;
                                return Ok(None);
                            }
                        }
                    };
                    while let Some(chunk) = rx.recv().await {
                        handle.append(&String::from_utf8_lossy(&chunk));
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
            "Started background job {}.",
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
}
