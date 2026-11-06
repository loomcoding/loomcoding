use crate::error::{CoreError, Result};
use crate::job_watch::{Watch, WatchBatch, WatchInfo, WatchSpec};
use crate::session::SessionId;
use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JobId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Running,
    Stopping,
    Completed,
    Killed,
    Failed,
}

impl JobStatus {
    fn settled(self) -> bool {
        matches!(self, Self::Completed | Self::Killed | Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobProgress {
    pub fraction: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobResult {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub id: JobId,
    pub kind: String,
    pub label: String,
    pub owner: SessionId,
    pub status: JobStatus,
    pub progress: Option<JobProgress>,
    /// Retained UTF-8 output; `output_start..output_end` are absolute byte offsets.
    pub output: String,
    pub output_start: usize,
    pub output_end: usize,
    pub detail: Option<String>,
    pub result: Option<JobResult>,
    /// Vigilancia activa (`watch`); ausente en snapshots anteriores a esta función.
    #[serde(default)]
    pub watch: Option<WatchInfo>,
}

#[derive(Debug, Clone)]
pub struct JobRead {
    pub snapshot: JobSnapshot,
    pub text: String,
    pub next: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct JobOutcome {
    pub status: JobStatus,
    pub detail: Option<String>,
    pub result: Option<JobResult>,
}

pub type JobBody = Box<
    dyn FnOnce(JobHandle, CancellationToken) -> BoxFuture<'static, Result<Option<JobResult>>>
        + Send,
>;

pub struct JobSpec {
    pub kind: String,
    pub label: String,
    pub owner: SessionId,
    /// Maximum retained output bytes per job.
    pub output_limit: usize,
    /// Vigilancia inicial (`job_start` con `watch`); `None` sin vigilancia.
    pub watch: Option<WatchSpec>,
    pub run: JobBody,
}

/// Vigilancia compartida entre el `JobHandle` (que alimenta la salida) y el registro (que la
/// cambia y la drena). Orden de cierre siempre: primero esta ranura, después el canal de estado.
type WatchSlot = Arc<Mutex<Option<Watch>>>;

#[derive(Clone)]
pub struct JobHandle {
    id: JobId,
    state: watch::Sender<JobSnapshot>,
    output_limit: usize,
    watch: WatchSlot,
}

impl JobHandle {
    pub fn id(&self) -> &JobId {
        &self.id
    }

    pub fn append(&self, text: &str) {
        let mut current = self.watch.lock().unwrap();
        if let Some(live) = current.as_mut() {
            live.feed(text);
        }
        self.state.send_modify(|snapshot| {
            snapshot.output.push_str(text);
            snapshot.output_end = snapshot.output_end.saturating_add(text.len());
            if snapshot.output.len() > self.output_limit {
                let mut cut = snapshot.output.len() - self.output_limit;
                while !snapshot.output.is_char_boundary(cut) {
                    cut += 1;
                }
                snapshot.output.drain(..cut);
                snapshot.output_start = snapshot.output_start.saturating_add(cut);
            }
            snapshot.watch = current.as_ref().map(Watch::info);
        });
    }

    pub fn update_progress(&self, fraction: f64, label: impl Into<String>) {
        self.state.send_modify(|snapshot| {
            snapshot.progress = Some(JobProgress {
                fraction: if fraction.is_finite() {
                    fraction.clamp(0.0, 1.0)
                } else {
                    0.0
                },
                label: label.into(),
            });
        });
    }
}

#[async_trait]
pub trait JobRegistry: Send + Sync {
    fn start(&self, spec: JobSpec) -> Result<JobHandle>;
    fn list(&self, owner: SessionId) -> Vec<JobSnapshot>;
    fn read(&self, owner: SessionId, id: &JobId, from: usize) -> Result<JobRead>;
    fn subscribe(&self, owner: SessionId, id: &JobId) -> Result<watch::Receiver<JobSnapshot>>;
    fn kill(&self, owner: SessionId, id: &JobId) -> Result<()>;
    async fn wait(&self, owner: SessionId, id: &JobId) -> Result<JobOutcome>;
    fn pending_settled_notices(&self, owner: SessionId) -> Vec<JobSnapshot>;
    fn mark_notice_delivered(&self, owner: SessionId, id: &JobId) -> Result<()>;
    /// Añade, sustituye (`Some`) o quita (`None`) la vigilancia de un job en marcha. Solo vigila
    /// líneas posteriores; lo pendiente de la vigilancia anterior se descarta.
    fn set_watch(&self, owner: SessionId, id: &JobId, spec: Option<WatchSpec>) -> Result<()>;
    /// Drena un lote por cada job vigilado del dueño que tenga avisos listos en `now`.
    fn take_watch_batches(&self, owner: SessionId, now: Instant) -> Vec<(JobSnapshot, WatchBatch)>;
    /// Dice si `take_watch_batches` entregaría algún lote del dueño en `now`, sin consumir nada.
    fn watch_ready(&self, owner: SessionId, now: Instant) -> bool;
}

struct Entry {
    state: watch::Sender<JobSnapshot>,
    cancel: CancellationToken,
    notice_delivered: bool,
    watch: WatchSlot,
}

#[derive(Default)]
pub struct LocalJobRegistry {
    // shortcut: entries live until registry drop; add bounded retention when job history policy is set.
    jobs: Mutex<HashMap<JobId, Entry>>,
    next: AtomicU64,
}

impl LocalJobRegistry {
    fn entry(
        &self,
        owner: SessionId,
        id: &JobId,
    ) -> Result<(watch::Receiver<JobSnapshot>, CancellationToken)> {
        let jobs = self.jobs.lock().unwrap();
        let entry = jobs
            .get(id)
            .ok_or_else(|| CoreError::Tool("unknown job".into()))?;
        let snapshot = entry.state.borrow();
        if snapshot.owner != owner {
            return Err(CoreError::Tool("unknown job".into()));
        }
        Ok((entry.state.subscribe(), entry.cancel.clone()))
    }

    /// Canal de estado y ranura de vigilancia de un job del dueño. Suelta el mapa antes de usarlos.
    fn watch_parts(
        &self,
        owner: SessionId,
        id: &JobId,
    ) -> Result<(watch::Sender<JobSnapshot>, WatchSlot)> {
        let jobs = self.jobs.lock().unwrap();
        let entry = jobs
            .get(id)
            .filter(|entry| entry.state.borrow().owner == owner)
            .ok_or_else(|| CoreError::Tool("unknown job".into()))?;
        Ok((entry.state.clone(), entry.watch.clone()))
    }
}

#[async_trait]
impl JobRegistry for LocalJobRegistry {
    fn start(&self, spec: JobSpec) -> Result<JobHandle> {
        if spec.kind.is_empty()
            || !spec
                .kind
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(CoreError::Tool(
                "job kind must contain only letters, digits, '-' or '_'".into(),
            ));
        }
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|error| CoreError::Tool(format!("job requires a Tokio runtime: {error}")))?;
        let id = JobId(format!(
            "{}-{}",
            spec.kind,
            self.next.fetch_add(1, Ordering::Relaxed) + 1
        ));
        let cancel = CancellationToken::new();
        let live = spec.watch.map(Watch::new);
        let slot: WatchSlot = Arc::new(Mutex::new(live));
        let initial = JobSnapshot {
            id: id.clone(),
            kind: spec.kind,
            label: spec.label,
            owner: spec.owner,
            status: JobStatus::Running,
            progress: None,
            output: String::new(),
            output_start: 0,
            output_end: 0,
            detail: None,
            result: None,
            watch: slot.lock().unwrap().as_ref().map(Watch::info),
        };
        let (state, _) = watch::channel(initial);
        let handle = JobHandle {
            id: id.clone(),
            state: state.clone(),
            output_limit: spec.output_limit,
            watch: slot.clone(),
        };
        self.jobs.lock().unwrap().insert(
            id.clone(),
            Entry {
                state: state.clone(),
                cancel: cancel.clone(),
                notice_delivered: false,
                watch: slot.clone(),
            },
        );

        let body = spec.run;
        let run_handle = handle.clone();
        let run_cancel = cancel.clone();
        let task = runtime.spawn(async move { body(run_handle, run_cancel).await });
        runtime.spawn(async move {
            let (status, detail, result) = match task.await {
                Ok(_) if cancel.is_cancelled() => (JobStatus::Killed, None, None),
                Ok(Ok(result)) => (JobStatus::Completed, None, result),
                Ok(Err(error)) => (JobStatus::Failed, Some(error.to_string()), None),
                Err(_) if cancel.is_cancelled() => (JobStatus::Killed, None, None),
                Err(error) => (JobStatus::Failed, Some(error.to_string()), None),
            };
            // El resto sin salto se prueba al cerrar; el cierre va bajo la ranura para que
            // `set_watch` no pueda colarse entre ambos pasos.
            let mut current = slot.lock().unwrap();
            if let Some(live) = current.as_mut() {
                live.finish();
            }
            state.send_modify(|snapshot| {
                snapshot.status = status;
                snapshot.detail = detail;
                snapshot.result = result;
                snapshot.watch = current.as_ref().map(Watch::info);
            });
        });
        Ok(handle)
    }

    fn list(&self, owner: SessionId) -> Vec<JobSnapshot> {
        let mut snapshots: Vec<_> = self
            .jobs
            .lock()
            .unwrap()
            .values()
            .map(|entry| entry.state.borrow().clone())
            .filter(|snapshot| snapshot.owner == owner)
            .collect();
        snapshots.sort_by(|a, b| a.id.cmp(&b.id));
        snapshots
    }

    fn read(&self, owner: SessionId, id: &JobId, from: usize) -> Result<JobRead> {
        let (rx, _) = self.entry(owner, id)?;
        let snapshot = rx.borrow().clone();
        let requested = from.max(snapshot.output_start).min(snapshot.output_end);
        let mut local = requested - snapshot.output_start;
        while !snapshot.output.is_char_boundary(local) {
            local += 1;
        }
        Ok(JobRead {
            text: snapshot.output[local..].to_string(),
            next: snapshot.output_end,
            truncated: from < snapshot.output_start,
            snapshot,
        })
    }

    fn subscribe(&self, owner: SessionId, id: &JobId) -> Result<watch::Receiver<JobSnapshot>> {
        self.entry(owner, id).map(|(rx, _)| rx)
    }

    fn kill(&self, owner: SessionId, id: &JobId) -> Result<()> {
        let (rx, cancel) = self.entry(owner, id)?;
        if !rx.borrow().status.settled() {
            cancel.cancel();
            let jobs = self.jobs.lock().unwrap();
            if let Some(entry) = jobs.get(id) {
                entry.state.send_modify(|snapshot| {
                    if snapshot.status == JobStatus::Running {
                        snapshot.status = JobStatus::Stopping;
                    }
                });
            }
        }
        Ok(())
    }

    async fn wait(&self, owner: SessionId, id: &JobId) -> Result<JobOutcome> {
        let (mut rx, _) = self.entry(owner, id)?;
        loop {
            let snapshot = rx.borrow_and_update().clone();
            if snapshot.status.settled() {
                return Ok(JobOutcome {
                    status: snapshot.status,
                    detail: snapshot.detail,
                    result: snapshot.result,
                });
            }
            rx.changed()
                .await
                .map_err(|_| CoreError::Tool("job state channel closed".into()))?;
        }
    }

    fn pending_settled_notices(&self, owner: SessionId) -> Vec<JobSnapshot> {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .filter_map(|entry| {
                let snapshot = entry.state.borrow().clone();
                if snapshot.owner == owner
                    && snapshot.kind != "subagent"
                    && snapshot.status.settled()
                    && !entry.notice_delivered
                {
                    Some(snapshot)
                } else {
                    None
                }
            })
            .collect()
    }

    fn mark_notice_delivered(&self, owner: SessionId, id: &JobId) -> Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let entry = jobs
            .get_mut(id)
            .filter(|entry| entry.state.borrow().owner == owner)
            .ok_or_else(|| CoreError::Tool("unknown job".into()))?;
        entry.notice_delivered = true;
        Ok(())
    }

    fn set_watch(&self, owner: SessionId, id: &JobId, spec: Option<WatchSpec>) -> Result<()> {
        let (state, slot) = self.watch_parts(owner, id)?;
        let mut current = slot.lock().unwrap();
        if state.borrow().status.settled() {
            return Err(CoreError::Tool(
                "job already finished; its watch cannot change".into(),
            ));
        }
        *current = spec.map(Watch::new);
        state.send_modify(|snapshot| snapshot.watch = current.as_ref().map(Watch::info));
        Ok(())
    }

    fn watch_ready(&self, owner: SessionId, now: Instant) -> bool {
        // Las ranuras se copian antes de bloquearlas: el mapa de jobs no se mantiene abierto.
        let slots: Vec<WatchSlot> = self
            .jobs
            .lock()
            .unwrap()
            .values()
            .filter(|entry| entry.state.borrow().owner == owner)
            .map(|entry| entry.watch.clone())
            .collect();
        slots.iter().any(|slot| {
            slot.lock()
                .unwrap()
                .as_ref()
                .is_some_and(|live| live.ready(now))
        })
    }

    fn take_watch_batches(&self, owner: SessionId, now: Instant) -> Vec<(JobSnapshot, WatchBatch)> {
        let parts: Vec<_> = self
            .jobs
            .lock()
            .unwrap()
            .values()
            .filter(|entry| entry.state.borrow().owner == owner)
            .map(|entry| (entry.state.clone(), entry.watch.clone()))
            .collect();
        let mut batches: Vec<_> = parts
            .into_iter()
            .filter_map(|(state, slot)| {
                let mut current = slot.lock().unwrap();
                let batch = current.as_mut()?.drain(now)?;
                state.send_modify(|snapshot| snapshot.watch = current.as_ref().map(Watch::info));
                let snapshot = state.borrow().clone();
                Some((snapshot, batch))
            })
            .collect();
        batches.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        batches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn spec(owner: SessionId, output_limit: usize, run: JobBody) -> JobSpec {
        JobSpec {
            kind: "test".into(),
            label: "test job".into(),
            owner,
            output_limit,

            watch: None,
            run,
        }
    }

    #[test]
    fn start_without_runtime_returns_error() {
        let registry = LocalJobRegistry::default();
        let result = registry.start(spec(
            SessionId(1),
            10,
            Box::new(|_, _| Box::pin(async { Ok(None) })),
        ));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn output_is_bounded_and_offset_reads_report_truncation() {
        let registry = LocalJobRegistry::default();
        let job = registry
            .start(spec(
                SessionId(1),
                5,
                Box::new(|handle, _| {
                    Box::pin(async move {
                        handle.append("abcdef");
                        handle.update_progress(2.0, "done");
                        Ok(Some(JobResult {
                            content: "ok".into(),
                        }))
                    })
                }),
            ))
            .unwrap();
        let outcome = registry.wait(SessionId(1), job.id()).await.unwrap();
        assert_eq!(outcome.status, JobStatus::Completed);
        assert_eq!(outcome.result.unwrap().content, "ok");
        let read = registry.read(SessionId(1), job.id(), 0).unwrap();
        assert_eq!(read.text, "bcdef");
        assert!(read.truncated);
        assert_eq!(read.next, 6);
        assert_eq!(read.snapshot.progress.unwrap().fraction, 1.0);
    }

    #[tokio::test]
    async fn settled_notices_are_owner_scoped_and_delivered_once() {
        let registry = LocalJobRegistry::default();
        let job = registry
            .start(spec(
                SessionId(1),
                20,
                Box::new(|_, _| Box::pin(async { Ok(None) })),
            ))
            .unwrap();
        registry.wait(SessionId(1), job.id()).await.unwrap();

        assert!(registry.pending_settled_notices(SessionId(2)).is_empty());
        assert_eq!(registry.pending_settled_notices(SessionId(1)).len(), 1);
        registry
            .mark_notice_delivered(SessionId(1), job.id())
            .unwrap();
        assert!(registry.pending_settled_notices(SessionId(1)).is_empty());
    }

    #[tokio::test]
    async fn reads_clamp_future_and_non_character_offsets() {
        let registry = LocalJobRegistry::default();
        let job = registry
            .start(spec(
                SessionId(1),
                20,
                Box::new(|handle, _| {
                    Box::pin(async move {
                        handle.append("a🙂b");
                        Ok(None)
                    })
                }),
            ))
            .unwrap();
        registry.wait(SessionId(1), job.id()).await.unwrap();

        let middle_of_emoji = registry.read(SessionId(1), job.id(), 2).unwrap();
        assert_eq!(middle_of_emoji.text, "b");
        assert_eq!(middle_of_emoji.next, 6);
        let past_end = registry.read(SessionId(1), job.id(), 100).unwrap();
        assert!(past_end.text.is_empty());
        assert_eq!(past_end.next, 6);
    }

    #[tokio::test]
    async fn kill_is_owner_fenced_and_reports_killed() {
        let registry = LocalJobRegistry::default();
        let job = registry
            .start(spec(
                SessionId(1),
                20,
                Box::new(|_, cancel| {
                    Box::pin(async move {
                        cancel.cancelled().await;
                        Ok(None)
                    })
                }),
            ))
            .unwrap();
        assert!(registry.read(SessionId(2), job.id(), 0).is_err());
        assert!(registry.kill(SessionId(2), job.id()).is_err());
        registry.kill(SessionId(1), job.id()).unwrap();
        let outcome = tokio::time::timeout(
            Duration::from_secs(1),
            registry.wait(SessionId(1), job.id()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(outcome.status, JobStatus::Killed);
    }
}
