//! Gestor de agentes del daemon y su implementación de `TaskHost` para A2A.

use super::*;

pub(super) struct AgentManager {
    pub(super) agents: RwLock<HashMap<String, Arc<AgentEntry>>>,
    pub(super) a2a_tasks: RwLock<HashMap<String, Arc<RuntimeTask>>>,
    pub(super) a2a_task_slots: Arc<Semaphore>,
    pub(super) a2a_cards: Arc<StdRwLock<HashMap<String, codeloom_a2a::serve::card::ServedCard>>>,
    pub(super) a2a_interface: Option<String>,
    pub(super) a2a_auth_scheme: Option<codeloom_a2a::serve::config::ServeAuthScheme>,
    pub(super) a2a_task_timeout: Duration,
    pub(super) runtime_generation: String,
    pub(super) data_dir: PathBuf,
    pub(super) a2a_auth_env: Option<String>,
    pub(super) peer_profile_lock: Mutex<()>,
    pub(super) supervisor: Option<SupervisorAgent>,
    pub(super) connected: AtomicUsize,
    pub(super) last_disconnected: StdMutex<Instant>,
    pub(super) keep_alive: AtomicBool,
}

impl AgentManager {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        a2a_cards: Arc<StdRwLock<HashMap<String, codeloom_a2a::serve::card::ServedCard>>>,
        a2a_interface: Option<String>,
        a2a_auth_scheme: Option<codeloom_a2a::serve::config::ServeAuthScheme>,
        runtime_generation: String,
        data_dir: PathBuf,
        a2a_auth_env: Option<String>,
        a2a_max_tasks: usize,
        a2a_task_timeout: Duration,
        supervisor: Option<SupervisorAgent>,
    ) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            a2a_tasks: RwLock::new(HashMap::new()),
            a2a_task_slots: Arc::new(Semaphore::new(a2a_max_tasks)),
            a2a_cards,
            a2a_interface,
            a2a_auth_scheme,
            a2a_task_timeout,
            runtime_generation,
            data_dir,
            a2a_auth_env,
            peer_profile_lock: Mutex::new(()),
            supervisor,
            connected: AtomicUsize::new(0),
            last_disconnected: StdMutex::new(Instant::now()),
            keep_alive: AtomicBool::new(false),
        }
    }

    pub(super) fn client_connected(&self) {
        self.connected.fetch_add(1, Ordering::AcqRel);
        *self.last_disconnected.lock().unwrap() = Instant::now();
    }

    pub(super) fn client_disconnected(&self) {
        self.connected.fetch_sub(1, Ordering::AcqRel);
        *self.last_disconnected.lock().unwrap() = Instant::now();
    }

    pub(super) fn publish_card(&self, entry: &AgentEntry) {
        let (Some(interface), Some(scheme)) =
            (self.a2a_interface.as_deref(), self.a2a_auth_scheme.as_ref())
        else {
            return;
        };
        let card = codeloom_a2a::serve::card::served_card_for_tenant(
            &entry.card,
            interface,
            Some(&entry.id),
            scheme,
        );
        self.a2a_cards
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(entry.id.clone(), card);
    }

    pub(super) async fn refresh_peer_profiles(&self) -> anyhow::Result<()> {
        let (Some(interface), Some(_), Some(auth_env)) = (
            self.a2a_interface.as_deref(),
            self.a2a_auth_scheme.as_ref(),
            self.a2a_auth_env.as_deref(),
        ) else {
            return Ok(());
        };
        let _lock = self.peer_profile_lock.lock().await;
        let agents = self
            .agents
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for recipient in agents.iter().filter(|agent| agent.a2a_consumes) {
            let dir =
                crate::runtime_peer_dir(&self.data_dir, &self.runtime_generation, &recipient.cwd);
            std::fs::create_dir_all(&dir)?;
            let peers = agents
                .iter()
                .filter(|peer| peer.cwd != recipient.cwd)
                .collect::<Vec<_>>();
            for peer in &peers {
                let Some(_) = self
                    .a2a_cards
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get(&peer.id)
                else {
                    continue;
                };
                let folder_name = runtime_peer_name(&peer.cwd, false, &peer.id);
                let duplicate = peers.iter().any(|other| {
                    other.id != peer.id
                        && runtime_peer_name(&other.cwd, false, &other.id) == folder_name
                });
                let name = runtime_peer_name(&peer.cwd, duplicate, &peer.id);
                let profile = runtime_peer_profile(
                    &name,
                    &peer.card.description,
                    &runtime_card_url(interface, &peer.id),
                    auth_env,
                );
                let path = dir.join(format!("{name}.md"));
                if std::fs::read_to_string(&path).ok().as_deref() == Some(profile.as_str()) {
                    continue;
                }
                let temporary = dir.join(format!(".{name}.tmp"));
                std::fs::write(&temporary, profile)?;
                std::fs::rename(temporary, path)?;
            }
        }
        for agent in agents.iter().filter(|agent| agent.a2a_consumes) {
            if let Some(refresh) = &agent.refresher {
                let _ = refresh.send(());
            }
        }
        Ok(())
    }

    pub(super) fn should_stop(&self) -> bool {
        if self.keep_alive.load(Ordering::Acquire) || self.connected.load(Ordering::Acquire) > 0 {
            return false;
        }
        if self
            .agents
            .try_read()
            .map(|agents| {
                agents.values().any(|agent| {
                    agent.working.load(Ordering::Acquire) || !agent.jobs.lock().unwrap().is_empty()
                })
            })
            .unwrap_or(true)
        {
            return false;
        }
        self.last_disconnected.lock().unwrap().elapsed() >= Duration::from_secs(300)
    }

    pub(super) async fn get(&self, id: &str) -> Option<Arc<AgentEntry>> {
        let agent = self.agents.read().await.get(id).cloned()?;
        debug_assert!(agent.internal);
        debug_assert!(!agent.card.name.is_empty());
        Some(agent)
    }

    pub(super) async fn task(&self, tenant: &str, id: &str) -> Result<Arc<RuntimeTask>, HostError> {
        let task = self
            .a2a_tasks
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or(HostError::NotFound)?;
        if task.tenant != tenant {
            return Err(HostError::NotFound);
        }
        Ok(task)
    }

    pub(super) async fn runtime_snapshot(&self) -> String {
        let agents = self.agents.read().await;
        let mut rows = agents
            .values()
            .map(|agent| {
                format!(
                    "{}: {}, connected={}, working={}",
                    agent.card.name,
                    agent.status.read().unwrap(),
                    agent.connected.load(Ordering::Acquire),
                    agent.working.load(Ordering::Acquire)
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        codeloom_core::tools::limit_bytes(&rows.join("\n"), 8 * 1024).into_owned()
    }
}

#[async_trait]
impl TaskHost for AgentManager {
    async fn start(&self, _prompt: String) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn start_for(&self, tenant: Option<&str>, prompt: String) -> Result<TaskView, HostError> {
        let tenant = tenant
            .filter(|tenant| !tenant.is_empty())
            .ok_or(HostError::NotFound)?;
        if tenant == "supervisor" {
            let permit = self
                .a2a_task_slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| HostError::Busy)?;
            let snapshot = self.runtime_snapshot().await;
            let task = Arc::new(RuntimeTask::new(
                generate_agent_id().map_err(|error| HostError::Other(error.to_string()))?,
                tenant.to_string(),
                "supervisor".into(),
            ));
            let mut tasks = self.a2a_tasks.write().await;
            if tasks.len() >= 256 {
                tasks.retain(|_, task| !task.view().terminal);
            }
            if tasks.len() >= 256 {
                return Err(HostError::Busy);
            }
            tasks.insert(task.id.clone(), task.clone());
            drop(tasks);
            let fallback = format!(
                "Deterministic runtime snapshot:\n{snapshot}\nModel request: {}\nLLM supervisor unavailable or not configured; no runtime action was taken.",
                codeloom_core::tools::limit_bytes(&prompt, 1024)
            );
            task.update(codeloom_core::task::TaskState::Working, None);
            if let Some(supervisor) = self.supervisor.clone() {
                let task_copy = task.clone();
                let prompt = codeloom_core::tools::limit_bytes(&prompt, 8 * 1024).into_owned();
                tokio::spawn(async move {
                    ejecutar_supervisor(task_copy, supervisor, prompt, snapshot, fallback, permit)
                        .await;
                });
            } else {
                task.update(codeloom_core::task::TaskState::Completed, Some(&fallback));
                drop(permit);
            }
            return Ok(task.view());
        }
        let entry = self.get(tenant).await.ok_or(HostError::NotFound)?;
        if !entry.connected.load(Ordering::Acquire) {
            return Err(HostError::NotFound);
        }
        let permit = self
            .a2a_task_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| HostError::Busy)?;
        let session = entry
            .store
            .create_session(&entry.project_key)
            .await
            .map_err(|error| HostError::Other(error.to_string()))?;
        let task = Arc::new(RuntimeTask::new(
            generate_agent_id().map_err(|error| HostError::Other(error.to_string()))?,
            tenant.to_string(),
            session.0.to_string(),
        ));
        {
            let mut tasks = self.a2a_tasks.write().await;
            if tasks.len() >= 256 {
                tasks.retain(|_, task| !task.view().terminal);
            }
            if tasks.len() >= 256 {
                return Err(HostError::Busy);
            }
            tasks.insert(task.id.clone(), task.clone());
        }
        if entry
            .commands
            .send(AgentCommand::A2aPrompt {
                task: task.clone(),
                session,
                prompt,
                timeout: self.a2a_task_timeout,
                _permit: permit,
            })
            .await
            .is_err()
        {
            self.a2a_tasks.write().await.remove(&task.id);
            return Err(HostError::NotFound);
        }
        Ok(task.view())
    }

    async fn get(&self, _id: &str) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn get_for(&self, tenant: Option<&str>, id: &str) -> Result<TaskView, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        Ok(self.task(tenant, id).await?.view())
    }

    async fn cancel(&self, _id: &str) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn cancel_for(&self, tenant: Option<&str>, id: &str) -> Result<TaskView, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let task = self.task(tenant, id).await?;
        let current = task.view();
        if current.terminal {
            return Err(HostError::NotCancelable);
        }
        task.cancel.cancel();
        Ok(current)
    }

    async fn wait_terminal(&self, _id: &str, _max: Duration) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn wait_terminal_for(
        &self,
        tenant: Option<&str>,
        id: &str,
        max: Duration,
    ) -> Result<TaskView, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let task = self.task(tenant, id).await?;
        let deadline = Instant::now() + max;
        loop {
            let view = task.view();
            if view.terminal || Instant::now() >= deadline {
                return Ok(view);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn subscribe(&self, _id: &str) -> Result<EventStream, HostError> {
        Err(HostError::NotFound)
    }

    async fn subscribe_for(
        &self,
        tenant: Option<&str>,
        id: &str,
    ) -> Result<EventStream, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let task = self.task(tenant, id).await?;
        let initial = Some(task.view());
        let updates = task.updates.subscribe();
        Ok(futures::stream::unfold(
            (task, updates, initial, false),
            |(task, mut updates, initial, done)| async move {
                if done {
                    return None;
                }
                if let Some(view) = initial {
                    let terminal = view.terminal;
                    let event = json!({"task": view.json});
                    return Some((event, (task, updates, None, terminal)));
                }
                loop {
                    match updates.recv().await {
                        Ok(view) => {
                            let event = json!({
                                "statusUpdate": {
                                    "taskId": task.id,
                                    "contextId": task.context_id,
                                    "status": view.json["status"],
                                }
                            });
                            return Some((event, (task, updates, None, view.terminal)));
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => return None,
                    }
                }
            },
        )
        .boxed())
    }

    async fn start_streaming(&self, _prompt: String) -> Result<EventStream, HostError> {
        Err(HostError::NotFound)
    }

    async fn start_streaming_for(
        &self,
        tenant: Option<&str>,
        prompt: String,
    ) -> Result<EventStream, HostError> {
        let view = self.start_for(tenant, prompt).await?;
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let id = view
            .json
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| HostError::Other("runtime task has no id".into()))?;
        self.subscribe_for(Some(tenant), id).await
    }
}
