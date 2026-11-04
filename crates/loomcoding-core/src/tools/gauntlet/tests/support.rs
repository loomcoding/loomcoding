//! Piezas comunes de los tests del Gauntlet.

use super::*;

/// Un hijo lanzado por el coordinador, con lo que se le dijo y si iba de solo lectura.
#[derive(Clone)]
pub(super) struct Launch {
    pub(super) child: SessionId,
    pub(super) agent: String,
    pub(super) read_only: bool,
    pub(super) prompt: String,
}

pub(super) struct TestLauncher {
    next: AtomicI64,
    children: Mutex<Vec<SubagentDescriptor>>,
    pub(super) launches: Mutex<Vec<Launch>>,
    pub(super) cancelled: Mutex<Vec<SessionId>>,
}

impl TestLauncher {
    pub(super) fn new() -> Self {
        Self {
            next: AtomicI64::new(100),
            children: Mutex::new(Vec::new()),
            launches: Mutex::new(Vec::new()),
            cancelled: Mutex::new(Vec::new()),
        }
    }

    fn add_child(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: String,
        read_only: bool,
    ) -> (SessionId, JobId) {
        let child = SessionId(self.next.fetch_add(1, Ordering::SeqCst));
        self.launches.lock().unwrap().push(Launch {
            child,
            agent: agent.clone(),
            read_only,
            prompt,
        });
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

    pub(super) fn last(&self) -> Launch {
        self.launches.lock().unwrap().last().cloned().unwrap()
    }

    pub(super) fn count(&self) -> usize {
        self.launches.lock().unwrap().len()
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

    pub(super) fn end_with(&self, child: SessionId, state: TaskState) {
        let mut children = self.children.lock().unwrap();
        children
            .iter_mut()
            .find(|item| item.child == child)
            .unwrap()
            .status = TaskStatus::new(state);
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
        Ok(self.add_child(parent, label, prompt, agent, true))
    }

    async fn start_local_workflow_agent(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: String,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        Ok(self.add_child(parent, label, prompt, agent, false))
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
        self.cancelled.lock().unwrap().push(child);
        self.end_with(child, TaskState::Canceled);
        Ok(self
            .children
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.child == child)
            .cloned()
            .unwrap())
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

/// Responde a la pregunta de aceptación final con «sí» o con otra cosa.
pub(super) struct TestResponder {
    pub(super) accept: bool,
}

#[async_trait]
impl PermissionResponder for TestResponder {
    async fn decide(&self, _request: &PermissionRequest) -> Decision {
        Decision::Allow
    }

    async fn ask(&self, questions: &[crate::question::Question]) -> Option<Vec<Answer>> {
        questions.first().map(|question| {
            let choice = match question.header.as_str() {
                "Final acceptance" if self.accept => "Accept and complete",
                _ => "Cancel",
            };
            vec![Answer {
                question: question.question.clone(),
                selected: vec![choice.into()],
            }]
        })
    }
}

pub(super) struct Entorno {
    pub(super) project: tempfile::TempDir,
    _db: tempfile::TempDir,
    pub(super) store: Arc<SqliteStore>,
    pub(super) session: SessionId,
    pub(super) launcher: Arc<TestLauncher>,
}

impl Entorno {
    /// Proyecto con `out/portada.md` (artefacto) y `ref/portada.md` (barra).
    pub(super) async fn nuevo() -> Self {
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(project.path().join("out")).unwrap();
        std::fs::create_dir_all(project.path().join("ref")).unwrap();
        std::fs::write(
            project.path().join("out/portada.md"),
            "versión del constructor",
        )
        .unwrap();
        std::fs::write(project.path().join("ref/portada.md"), "la referencia").unwrap();
        let db = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(db.path().join("g.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let session = store.create_session("gauntlet-test").await.unwrap();
        Self {
            project,
            _db: db,
            store,
            session,
            launcher: Arc::new(TestLauncher::new()),
        }
    }

    pub(super) fn ctx(&self, accept: bool) -> ToolCtx {
        ToolCtx {
            cwd: self.project.path().to_path_buf(),
            store: Some(self.store.clone()),
            permission: Arc::new(crate::permission::PermissionService::with_responder(
                Box::new(TestResponder { accept }),
            )),
            session: self.session,
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: crate::event::EventBus::new(32),
            subagents: Some(self.launcher.clone()),
        }
    }

    pub(super) async fn run(&self, input: serde_json::Value) -> Result<ToolOutput> {
        self.run_as(true, input).await
    }

    pub(super) async fn run_as(
        &self,
        accept: bool,
        input: serde_json::Value,
    ) -> Result<ToolOutput> {
        GauntletTool
            .run(self.ctx(accept), input, CancellationToken::new())
            .await
    }

    pub(super) fn start_input() -> serde_json::Value {
        serde_json::json!({
            "action": "start", "goal": "pulir la portada", "artifact": "out/portada.md",
            "bar": ["ref/portada.md"], "builder": "constructor", "critic": "critico"
        })
    }

    pub(super) async fn start(&self) {
        let out = self.run(Self::start_input()).await.unwrap();
        assert!(!out.is_error, "{}", out.content);
    }

    pub(super) async fn active(&self) -> crate::gauntlet::Gauntlet {
        self.store
            .active_gauntlet(self.session)
            .await
            .unwrap()
            .unwrap()
    }

    pub(super) async fn latest(&self) -> crate::gauntlet::Gauntlet {
        self.store
            .latest_gauntlet(self.session)
            .await
            .unwrap()
            .unwrap()
    }

    /// Termina el hijo activo con `report` y llama a `advance`.
    pub(super) async fn finish_and_advance(&self, report: &str) -> ToolOutput {
        let child = self.active().await.active_child.unwrap();
        self.launcher.complete(child, report);
        self.run(serde_json::json!({"action": "advance"}))
            .await
            .unwrap()
    }

    /// Etiqueta («A» o «B») que el crítico ve para la barra en el juicio en curso.
    async fn bar_label(&self) -> &'static str {
        match self.active().await.rounds.last().unwrap().order {
            crate::gauntlet::AbOrder::ArtifactFirst => "B",
            crate::gauntlet::AbOrder::BarFirst => "A",
        }
    }

    /// Informe de un crítico que prefiere la barra: hay brecha.
    pub(super) async fn gap(&self) -> String {
        self.gap_with("los márgenes están descuadrados").await
    }

    pub(super) async fn gap_with(&self, text: &str) -> String {
        let bar = self.bar_label().await;
        format!("Hay diferencias.\n{{\"preferred\":\"{bar}\",\"biggest_gap\":\"{text}\"}}")
    }

    /// Informe de un crítico que prefiere el artefacto: no hay brecha.
    pub(super) async fn no_gap(&self) -> String {
        let artifact = if self.bar_label().await == "A" {
            "B"
        } else {
            "A"
        };
        format!("Todo a la altura.\n{{\"preferred\":\"{artifact}\"}}")
    }

    /// Lleva un gauntlet de `start` hasta el juicio de la primera ronda.
    pub(super) async fn hasta_el_juicio(&self) {
        self.start().await;
        let out = self.finish_and_advance("hecho: reescribí la portada").await;
        assert!(!out.is_error, "{}", out.content);
        assert_eq!(self.active().await.phase, GauntletPhase::Judging);
    }
}
