//! Piezas comunes de los tests.

use super::*;

pub(super) struct TestLauncher {
    pub(super) next: AtomicI64,
    pub(super) children: Mutex<Vec<SubagentDescriptor>>,
    pub(super) read_only_reviews: Mutex<Vec<String>>,
    pub(super) review_prompts: Mutex<Vec<String>>,
}

impl TestLauncher {
    pub(super) fn new() -> Self {
        Self {
            next: AtomicI64::new(100),
            children: Mutex::new(Vec::new()),
            read_only_reviews: Mutex::new(Vec::new()),
            review_prompts: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn add_child(
        &self,
        parent: SessionId,
        label: String,
        agent: String,
        read_only: bool,
    ) -> (SessionId, JobId) {
        let child = SessionId(self.next.fetch_add(1, Ordering::SeqCst));
        if read_only {
            self.read_only_reviews.lock().unwrap().push(agent.clone());
        }
        self.children.lock().unwrap().push(SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child,
            job_id: format!("job-{}", child.0),
            label,
            status: TaskStatus::new(TaskState::Working),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: Some(agent),
            agent_origin: Some("project:test".into()),
            reference_task_ids: Vec::new(),
        });
        (child, JobId(format!("job-{}", child.0)))
    }

    pub(super) fn complete(&self, child: SessionId, report: &str) {
        let mut children = self.children.lock().unwrap();
        let descriptor = children
            .iter_mut()
            .find(|item| item.child == child)
            .unwrap();
        descriptor.status = TaskStatus::new(TaskState::Completed);
        descriptor.artifacts = vec![Artifact {
            artifact_id: format!("artifact-{}", child.0),
            name: Some("report".into()),
            description: None,
            parts: vec![Part::text(report)],
            metadata: None,
            extensions: Vec::new(),
        }];
    }
}

#[async_trait]
impl SubagentLauncher for TestLauncher {
    async fn start(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: Option<String>,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        Err(CoreError::Tool("unexpected generic launch".into()))
    }

    async fn start_read_only_review(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: String,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        self.review_prompts.lock().unwrap().push(prompt);
        Ok(self.add_child(parent, label, agent, true))
    }

    async fn start_local_workflow_agent(
        &self,
        parent: SessionId,
        label: String,
        _prompt: String,
        agent: String,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        Ok(self.add_child(parent, label, agent, false))
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

    async fn list(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
        Ok(self
            .children
            .lock()
            .unwrap()
            .iter()
            .filter(|child| child.parent == parent)
            .cloned()
            .collect())
    }

    async fn deliver_notices(&self, _parent: SessionId) -> Result<Vec<crate::message::Message>> {
        Ok(Vec::new())
    }
}

pub(super) struct TestResponder {
    pub(super) approve_plan: bool,
}

#[async_trait]
impl PermissionResponder for TestResponder {
    async fn decide(&self, _request: &PermissionRequest) -> Decision {
        Decision::Allow
    }

    async fn ask(&self, questions: &[crate::question::Question]) -> Option<Vec<Answer>> {
        questions.first().map(|question| {
            let choice = match question.header.as_str() {
                "Plan approval" if self.approve_plan => "Approve and start implementation",
                "Plan approval" => "Approve",
                "Final acceptance" => "Accept and complete",
                _ => "Cancel",
            };
            vec![Answer {
                question: question.question.clone(),
                selected: vec![choice.into()],
            }]
        })
    }
}

pub(super) fn context(
    cwd: &std::path::Path,
    session: SessionId,
    store: Arc<dyn crate::session::SessionStore>,
    launcher: Arc<dyn SubagentLauncher>,
    approve_plan: bool,
) -> ToolCtx {
    ToolCtx {
        cwd: cwd.to_path_buf(),
        store: Some(store),
        permission: Arc::new(crate::permission::PermissionService::with_responder(
            Box::new(TestResponder { approve_plan }),
        )),
        session,
        jobs: Arc::new(crate::job::LocalJobRegistry::default()),
        bus: crate::event::EventBus::new(32),
        subagents: Some(launcher),
    }
}

pub(super) async fn store_and_session(path: &std::path::Path) -> (Arc<SqliteStore>, SessionId) {
    let store = Arc::new(SqliteStore::open(path.to_str().unwrap()).await.unwrap());
    let session = store.create_session("workflow-test").await.unwrap();
    (store, session)
}

pub(super) fn write_artifacts(project: &std::path::Path) {
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::write(project.join("docs/spec.md"), "approved specification").unwrap();
    std::fs::write(project.join("docs/plan.md"), "approved plan").unwrap();
}

pub(super) fn planner_report() -> &'static str {
    "Spec: docs/spec.md\nPlan: docs/plan.md"
}

/// Contexto como el de `--yolo`: el servicio de permisos lo permite todo y no hay nadie a quien preguntar.
pub(super) fn yolo_context(
    cwd: &std::path::Path,
    session: SessionId,
    store: Arc<dyn crate::session::SessionStore>,
    launcher: Arc<dyn SubagentLauncher>,
) -> ToolCtx {
    ToolCtx {
        permission: Arc::new(crate::permission::PermissionService::allow_all()),
        ..context(cwd, session, store, launcher, true)
    }
}

/// Lleva un flujo hasta que el implementador entrega, con `implementer_report` como informe, y devuelve
/// el lanzador y la sesión una vez avanzado a revisión.
pub(super) async fn workflow_in_review(
    project: &std::path::Path,
    store: Arc<SqliteStore>,
    session: SessionId,
    implementer_report: &str,
) -> Arc<TestLauncher> {
    write_artifacts(project);
    let launcher = Arc::new(TestLauncher::new());
    let tool = DevelopmentWorkflowTool;
    let run = |input: serde_json::Value| {
        tool.run(
            context(project, session, store.clone(), launcher.clone(), true),
            input,
            CancellationToken::new(),
        )
    };
    run(serde_json::json!({
        "action":"start", "task":"test workflow",
        "planner":"planner", "implementer":"builder", "reviewer":"reviewer"
    }))
    .await
    .unwrap();
    let planner_child = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap()
        .active_child
        .unwrap();
    launcher.complete(planner_child, planner_report());
    run(serde_json::json!({"action":"advance", "spec_path":"docs/spec.md", "plan_path":"docs/plan.md"}))
        .await
        .unwrap();
    let implementer_child = store
        .active_development_workflow(session)
        .await
        .unwrap()
        .unwrap()
        .active_child
        .unwrap();
    launcher.complete(implementer_child, implementer_report);
    run(serde_json::json!({"action":"advance"})).await.unwrap();
    launcher
}
