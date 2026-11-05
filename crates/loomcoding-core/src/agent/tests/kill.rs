use super::*;
use crate::job::{JobRegistry, JobSpec, JobStatus, LocalJobRegistry};
use crate::session::SubagentDescriptor;
use crate::subagent::SubagentLauncher;
use crate::task::{TaskState, TaskStatus};

fn descriptor(parent: SessionId, child: i64, state: TaskState) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent,
        child: SessionId(child),
        job_id: format!("subagent-{child}"),
        label: "hijo".into(),
        status: TaskStatus::new(state),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

/// Lanzador falso: lista hijos fijos y anota a cuáles se pidió cancelar.
struct FakeLauncher {
    children: Vec<SubagentDescriptor>,
    canceled: Mutex<Vec<SessionId>>,
}

#[async_trait]
impl SubagentLauncher for FakeLauncher {
    async fn start(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: Option<String>,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, crate::job::JobId)> {
        unreachable!("la parada de emergencia no lanza hijos")
    }
    async fn control(
        &self,
        _parent: SessionId,
        _child: SessionId,
        _action: &str,
        _message: Option<String>,
    ) -> Result<()> {
        Ok(())
    }
    async fn cancel(&self, _parent: SessionId, child: SessionId) -> Result<SubagentDescriptor> {
        self.canceled.lock().unwrap().push(child);
        Ok(self
            .children
            .iter()
            .find(|d| d.child == child)
            .unwrap()
            .clone())
    }
    async fn list(&self, _parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
        Ok(self.children.clone())
    }
    async fn deliver_notices(&self, _parent: SessionId) -> Result<Vec<Message>> {
        Ok(Vec::new())
    }
}

async fn agent_with(
    launcher: Arc<FakeLauncher>,
    jobs: Arc<LocalJobRegistry>,
) -> (AgentLoop, SessionId, broadcast::Receiver<LiveEvent>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("k.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let bus = EventBus::new(64);
    let events = bus.subscribe();
    let agent = AgentLoop::new(
        Arc::new(QueueProvider {
            scripts: Mutex::new(VecDeque::new()),
        }),
        store,
        Arc::new(ToolRegistry::new()),
        bus,
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.keep(),
    )
    .with_jobs(jobs)
    .with_subagent_launcher(launcher);
    (agent, sid, events)
}

fn spin_job(jobs: &LocalJobRegistry, owner: SessionId) -> crate::job::JobId {
    jobs.start(JobSpec {
        kind: "test".into(),
        label: "espera".into(),
        owner,
        output_limit: 1024,
        run: Box::new(|_, cancel| {
            Box::pin(async move {
                cancel.cancelled().await;
                Ok(None)
            })
        }),
    })
    .unwrap()
    .id()
    .clone()
}

#[tokio::test]
async fn kill_everything_cancels_running_children_and_jobs() {
    let jobs = Arc::new(LocalJobRegistry::default());
    let launcher = Arc::new(FakeLauncher {
        children: vec![
            descriptor(SessionId(1), 10, TaskState::Working),
            descriptor(SessionId(1), 11, TaskState::InputRequired),
            descriptor(SessionId(1), 12, TaskState::Completed),
        ],
        canceled: Mutex::new(Vec::new()),
    });
    let (agent, sid, mut events) = agent_with(launcher.clone(), jobs.clone()).await;
    let running = spin_job(&jobs, sid);

    let report = agent.kill_everything(sid).await;

    assert_eq!(report.children, 2, "los dos hijos no terminales");
    assert_eq!(report.jobs, 1);
    assert_eq!(
        *launcher.canceled.lock().unwrap(),
        vec![SessionId(10), SessionId(11)],
        "el hijo ya completado no se toca"
    );
    let outcome = tokio::time::timeout(Duration::from_secs(2), jobs.wait(sid, &running))
        .await
        .expect("el job debía asentarse")
        .unwrap();
    assert_eq!(outcome.status, JobStatus::Killed);
    let mut notice = None;
    while let Ok(event) = events.try_recv() {
        if let LiveEvent::RuntimeNotice { message } = event {
            notice = Some(message);
        }
    }
    assert_eq!(
        notice.as_deref(),
        Some("emergency stop: 2 children, 1 jobs")
    );
}

#[tokio::test]
async fn kill_everything_with_nothing_running_reports_zero() {
    let launcher = Arc::new(FakeLauncher {
        children: vec![],
        canceled: Mutex::new(Vec::new()),
    });
    let (agent, sid, _events) = agent_with(launcher, Arc::new(LocalJobRegistry::default())).await;
    let report = agent.kill_everything(sid).await;
    assert_eq!((report.children, report.jobs), (0, 0));
}
