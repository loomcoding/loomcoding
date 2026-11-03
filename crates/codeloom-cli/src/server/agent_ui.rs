//! Interfaz de un agente con el cliente: órdenes, estado compartido y respuesta de permisos.

use super::*;

pub(super) enum AgentCommand {
    Prompt(codeloom_tui::app::PendingPrompt),
    Wake,
    Compact,
    Session(codeloom_tui::run::SessionCommand),
    ProviderSettings {
        id: u64,
        settings: codeloom_tui::app::ProviderSettings,
        api_key: Option<String>,
    },
    A2aPrompt {
        task: Arc<RuntimeTask>,
        session: SessionId,
        prompt: String,
        timeout: Duration,
        _permit: tokio::sync::OwnedSemaphorePermit,
    },
}

pub(super) struct AgentUi {
    pub(super) events: RwLock<mpsc::Sender<ServerFrame>>,
    pub(super) connected: AtomicBool,
    pub(super) decisions: Mutex<HashMap<u64, oneshot::Sender<Decision>>>,
    pub(super) questions: Mutex<HashMap<u64, oneshot::Sender<Option<Vec<Answer>>>>>,
    pub(super) next_id: AtomicU64,
}

impl AgentUi {
    pub(super) fn new(events: mpsc::Sender<ServerFrame>) -> Self {
        Self {
            events: RwLock::new(events),
            connected: AtomicBool::new(true),
            decisions: Mutex::new(HashMap::new()),
            questions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    pub(super) async fn replace_client(&self, events: mpsc::Sender<ServerFrame>) {
        *self.events.write().await = events;
        self.connected.store(true, Ordering::Release);
        for (_, reply) in self.decisions.lock().await.drain() {
            let _ = reply.send(Decision::Deny);
        }
        for (_, reply) in self.questions.lock().await.drain() {
            let _ = reply.send(None);
        }
    }

    pub(super) async fn send(&self, event: ServerEvent) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        let events = self.events.read().await.clone();
        tokio::time::timeout(
            Duration::from_secs(2),
            events.send(ServerFrame::event(bounded_wire(event))),
        )
        .await
        .is_ok_and(|result| result.is_ok())
    }

    pub(super) async fn disconnect(&self) {
        self.connected.store(false, Ordering::Release);
        for (_, reply) in self.decisions.lock().await.drain() {
            let _ = reply.send(Decision::Deny);
        }
        for (_, reply) in self.questions.lock().await.drain() {
            let _ = reply.send(None);
        }
    }

    pub(super) async fn decide(&self, request: &PermissionRequest) -> Decision {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.decisions.lock().await.insert(id, tx);
        let bounded = PermissionRequest {
            tool: codeloom_core::tools::limit_bytes(&request.tool, 128).into_owned(),
            action: codeloom_core::tools::limit_bytes(&request.action, 128).into_owned(),
            detail: codeloom_core::tools::limit_bytes(&request.detail, 16_000).into_owned(),
        };
        if !self
            .send(ServerEvent::PermissionRequest {
                id,
                request: bounded,
            })
            .await
        {
            self.decisions.lock().await.remove(&id);
            return Decision::Deny;
        }
        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(decision)) => decision,
            _ => {
                self.decisions.lock().await.remove(&id);
                Decision::Deny
            }
        }
    }

    pub(super) async fn ask_user(&self, questions: &[Question]) -> Option<Vec<Answer>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.questions.lock().await.insert(id, tx);
        if !self
            .send(ServerEvent::Questions {
                id,
                questions: questions
                    .iter()
                    .take(4)
                    .map(|question| Question {
                        header: codeloom_core::tools::limit_bytes(&question.header, 128)
                            .into_owned(),
                        question: codeloom_core::tools::limit_bytes(&question.question, 4096)
                            .into_owned(),
                        options: question
                            .options
                            .iter()
                            .take(6)
                            .map(|option| codeloom_core::question::QuestionOption {
                                label: codeloom_core::tools::limit_bytes(&option.label, 256)
                                    .into_owned(),
                                description: codeloom_core::tools::limit_bytes(
                                    &option.description,
                                    512,
                                )
                                .into_owned(),
                            })
                            .collect(),
                        multi_select: question.multi_select,
                    })
                    .collect(),
            })
            .await
        {
            self.questions.lock().await.remove(&id);
            return None;
        }
        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(answers)) => answers,
            _ => {
                self.questions.lock().await.remove(&id);
                None
            }
        }
    }

    pub(super) async fn resolve_decision(&self, id: u64, decision: Decision) {
        if let Some(reply) = self.decisions.lock().await.remove(&id) {
            let _ = reply.send(decision);
        }
    }

    pub(super) async fn resolve_questions(&self, id: u64, answers: Option<Vec<Answer>>) {
        if let Some(reply) = self.questions.lock().await.remove(&id) {
            let _ = reply.send(answers);
        }
    }
}

pub(super) struct SharedAgentUi(pub(super) Arc<AgentUi>);

#[async_trait]
impl PermissionResponder for SharedAgentUi {
    async fn decide(&self, request: &PermissionRequest) -> Decision {
        self.0.decide(request).await
    }

    async fn ask(&self, questions: &[Question]) -> Option<Vec<Answer>> {
        self.0.ask_user(questions).await
    }

    fn requires_user(&self) -> bool {
        true
    }
}
