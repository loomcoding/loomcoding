use crate::event::{EventBus, LiveEvent};
use crate::tools::{ExecutionMode, ToolCtx, ToolOutput, ToolRegistry};
use futures::StreamExt;

pub(crate) const WORKFLOW_PARENT_TOOLS: &[&str] = &[
    "development_workflow",
    "gauntlet",
    "view",
    "ls",
    "glob",
    "grep",
    "lsp",
    "web",
];

/// Si el padre tiene un flujo de desarrollo o un gauntlet en marcha: mientras dure, solo usa las
/// herramientas de `WORKFLOW_PARENT_TOOLS`.
pub(crate) async fn active_orchestration(
    store: &dyn crate::session::SessionStore,
    session: crate::session::SessionId,
) -> crate::error::Result<bool> {
    Ok(store.active_development_workflow(session).await?.is_some()
        || store.active_gauntlet(session).await?.is_some())
}

/// Ejecuta tool calls respetando barreras:
/// las `Exclusive` se ejecutan solas y en orden; las `Parallel` en un lote comun.
pub async fn execute_tools(
    registry: &ToolRegistry,
    ctx: ToolCtx,
    calls: Vec<(String, String, serde_json::Value)>,
    cancel: tokio_util::sync::CancellationToken,
    bus: EventBus,
) -> Vec<(String, ToolOutput)> {
    let workflow_start_requested = calls.iter().any(|(_, name, input)| {
        matches!(name.as_str(), "development_workflow" | "gauntlet") && input["action"] == "start"
    });
    let mut batches: Vec<Vec<(String, String, serde_json::Value)>> = Vec::new();
    let mut current: Vec<(String, String, serde_json::Value)> = Vec::new();
    for (id, name, input) in calls {
        let mode = registry
            .get(&name)
            .map(|t| t.execution_mode(&input))
            .unwrap_or(ExecutionMode::Exclusive);
        match mode {
            ExecutionMode::Parallel => current.push((id, name, input)),
            ExecutionMode::Exclusive => {
                if !current.is_empty() {
                    batches.push(std::mem::take(&mut current));
                }
                batches.push(vec![(id, name, input)]);
            }
        }
    }
    if !current.is_empty() {
        batches.push(current);
    }

    let mut results = Vec::new();
    for batch in batches {
        let futs = batch.into_iter().map(|(id, name, input)| {
            let tool = registry.get(&name);
            let ctx = ctx.clone();
            let cancel = cancel.clone();
            let bus = bus.clone();
            async move {
                bus.publish(LiveEvent::ToolStarted {
                    call_id: id.clone(),
                    name: name.clone(),
                });
                match tool {
                    Some(t) => {
                        let workflow_active = match &ctx.store {
                            Some(store) => match active_orchestration(store.as_ref(), ctx.session).await {
                                Ok(active) => active,
                                Err(error) => {
                                    bus.publish(LiveEvent::ToolFinished {
                                        call_id: id.clone(),
                                        is_error: true,
                                    });
                                    return (
                                        id,
                                        ToolOutput::err(format!(
                                            "cannot verify workflow write gate: {error}"
                                        )),
                                    );
                                }
                            },
                            None => false,
                        };
                        if (workflow_active || workflow_start_requested)
                            && !WORKFLOW_PARENT_TOOLS.contains(&name.as_str())
                        {
                            let message = format!(
                                "{name} is blocked while a development workflow is active or starting"
                            );
                            bus.publish(LiveEvent::ToolFinished {
                                call_id: id.clone(),
                                is_error: true,
                            });
                            return (id, ToolOutput::err(message));
                        }
                        let out = t
                            .run(ctx, input, cancel)
                            .await
                            .unwrap_or_else(|e| ToolOutput::err(e.to_string()));
                        bus.publish(LiveEvent::ToolFinished {
                            call_id: id.clone(),
                            is_error: out.is_error,
                        });
                        (id, out)
                    }
                    None => {
                        bus.publish(LiveEvent::ToolFinished {
                            call_id: id.clone(),
                            is_error: true,
                        });
                        (id, ToolOutput::err(format!("unknown tool: {name}")))
                    }
                }
            }
        });
        let collected: Vec<_> = futures::stream::iter(futs)
            .buffer_unordered(8)
            .collect()
            .await;
        results.extend(collected);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionService;
    use crate::session::sqlite::SqliteStore;
    use crate::session::SessionStore;
    use crate::tools::{ExecutionMode, Tool};
    use crate::workflow::{DevelopmentWorkflow, WorkflowPhase};
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    struct Echo;
    #[async_trait]
    impl Tool for Echo {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "e"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
            ExecutionMode::Parallel
        }
        async fn run(
            &self,
            _c: ToolCtx,
            i: serde_json::Value,
            _x: CancellationToken,
        ) -> crate::error::Result<ToolOutput> {
            Ok(ToolOutput::ok(i["v"].as_str().unwrap_or_default()))
        }
    }

    #[tokio::test]
    async fn runs_parallel_batch() {
        let mut r = ToolRegistry::new();
        r.register(Arc::new(Echo));
        let ctx = ToolCtx {
            cwd: ".".into(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        };
        let calls = vec![
            (
                "c1".to_string(),
                "echo".to_string(),
                serde_json::json!({"v":"a"}),
            ),
            (
                "c2".to_string(),
                "echo".to_string(),
                serde_json::json!({"v":"b"}),
            ),
            (
                "c3".to_string(),
                "missing".to_string(),
                serde_json::json!({}),
            ),
        ];
        let mut res =
            execute_tools(&r, ctx, calls, CancellationToken::new(), EventBus::new(16)).await;
        res.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].1.content, "a");
        assert!(res[2].1.is_error);
    }

    struct ExclusiveEcho;
    #[async_trait]
    impl Tool for ExclusiveEcho {
        fn name(&self) -> &str {
            "exclusive_echo"
        }
        fn description(&self) -> &str {
            "e"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
            ExecutionMode::Exclusive
        }
        async fn run(
            &self,
            _c: ToolCtx,
            i: serde_json::Value,
            _x: CancellationToken,
        ) -> crate::error::Result<ToolOutput> {
            Ok(ToolOutput::ok(i["v"].as_str().unwrap_or_default()))
        }
    }

    #[tokio::test]
    async fn exclusive_calls_serialize_but_all_run() {
        let mut r = ToolRegistry::new();
        r.register(Arc::new(Echo));
        r.register(Arc::new(ExclusiveEcho));
        let ctx = ToolCtx {
            cwd: ".".into(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        };
        let calls = vec![
            (
                "c1".to_string(),
                "echo".to_string(),
                serde_json::json!({"v":"a"}),
            ),
            (
                "c2".to_string(),
                "exclusive_echo".to_string(),
                serde_json::json!({"v":"b"}),
            ),
            (
                "c3".to_string(),
                "exclusive_echo".to_string(),
                serde_json::json!({"v":"c"}),
            ),
            (
                "c4".to_string(),
                "echo".to_string(),
                serde_json::json!({"v":"d"}),
            ),
        ];
        let mut res =
            execute_tools(&r, ctx, calls, CancellationToken::new(), EventBus::new(16)).await;
        res.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(res.len(), 4);
        assert_eq!(res[0].1.content, "a");
        assert_eq!(res[1].1.content, "b");
        assert_eq!(res[2].1.content, "c");
        assert_eq!(res[3].1.content, "d");
    }

    struct WriteFlag(Arc<std::sync::atomic::AtomicBool>);
    #[async_trait]
    impl Tool for WriteFlag {
        fn name(&self) -> &str {
            "bash"
        }
        fn description(&self) -> &str {
            "write marker"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn run(
            &self,
            _ctx: ToolCtx,
            _input: serde_json::Value,
            _cancel: CancellationToken,
        ) -> crate::error::Result<ToolOutput> {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(ToolOutput::ok("written"))
        }
    }

    #[tokio::test]
    async fn a_write_is_blocked_even_if_it_precedes_workflow_start_in_one_response() {
        let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(WriteFlag(executed.clone())));
        let ctx = ToolCtx {
            cwd: ".".into(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(0),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        };
        let results = execute_tools(
            &registry,
            ctx,
            vec![
                ("write-1".into(), "bash".into(), serde_json::json!({})),
                (
                    "workflow-1".into(),
                    "development_workflow".into(),
                    serde_json::json!({"action":"start"}),
                ),
            ],
            CancellationToken::new(),
            EventBus::new(8),
        )
        .await;
        assert!(!executed.load(std::sync::atomic::Ordering::SeqCst));
        assert!(results
            .iter()
            .find(|(id, _)| id == "write-1")
            .is_some_and(|(_, result)| result.is_error));
    }

    #[tokio::test]
    async fn parent_write_delegation_and_polling_are_blocked_while_workflow_active() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(directory.path().join("workflow.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let session = store.create_session("/project").await.unwrap();
        store
            .create_development_workflow(
                DevelopmentWorkflow {
                    id: crate::workflow::WorkflowId(0),
                    parent: session,
                    task: "test gate".into(),
                    planner: "planner".into(),
                    implementer: "builder".into(),
                    reviewer: "reviewer".into(),
                    phase: WorkflowPhase::Specifying,
                    version: 0,
                    active_child: None,
                    planner_child: None,
                    implementation_children: Vec::new(),
                    reviewer_children: Vec::new(),
                    correction_round: 0,
                    spec_path: None,
                    plan_path: None,
                    spec_sha256: None,
                    plan_sha256: None,
                    last_report: None,
                    created_at_ms: 0,
                    updated_at_ms: 0,
                },
                "test",
            )
            .await
            .unwrap();

        let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(WriteFlag(executed.clone())));
        registry.register(Arc::new(SubagentFlag(executed.clone())));
        registry.register(Arc::new(JobListFlag(executed.clone())));
        let ctx = ToolCtx {
            cwd: ".".into(),
            store: Some(store),
            permission: Arc::new(PermissionService::allow_all()),
            session,
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        };

        let results = execute_tools(
            &registry,
            ctx,
            vec![
                ("write".into(), "bash".into(), serde_json::json!({})),
                ("delegate".into(), "subagent".into(), serde_json::json!({})),
                ("poll".into(), "job_list".into(), serde_json::json!({})),
            ],
            CancellationToken::new(),
            EventBus::new(8),
        )
        .await;

        assert!(!executed.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(results.len(), 3);
        assert!(results
            .iter()
            .all(|(_, result)| result.is_error && result.content.contains("blocked")));
    }

    struct GauntletFlag(Arc<std::sync::atomic::AtomicBool>);
    #[async_trait]
    impl Tool for GauntletFlag {
        fn name(&self) -> &str {
            "gauntlet"
        }
        fn description(&self) -> &str {
            "coordinate a gauntlet"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn run(
            &self,
            _ctx: ToolCtx,
            _input: serde_json::Value,
            _cancel: CancellationToken,
        ) -> crate::error::Result<ToolOutput> {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(ToolOutput::ok("coordinated"))
        }
    }

    #[tokio::test]
    async fn parent_writes_are_blocked_while_a_gauntlet_is_active_but_the_gauntlet_tool_runs() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(directory.path().join("gauntlet.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let session = store.create_session("/project").await.unwrap();
        store
            .create_gauntlet(
                crate::gauntlet::Gauntlet::new(crate::gauntlet::NewGauntlet {
                    id: crate::gauntlet::GauntletId(0),
                    parent: session,
                    goal: "test gate".into(),
                    artifact: "a.md".into(),
                    bar: vec!["b.md".into()],
                    builder: "builder".into(),
                    critic: "critic".into(),
                    max_rounds: None,
                    now_ms: 0,
                })
                .unwrap(),
                "test",
            )
            .await
            .unwrap();

        let wrote = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let coordinated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(WriteFlag(wrote.clone())));
        registry.register(Arc::new(SubagentFlag(wrote.clone())));
        registry.register(Arc::new(GauntletFlag(coordinated.clone())));
        let ctx = ToolCtx {
            cwd: ".".into(),
            store: Some(store),
            permission: Arc::new(PermissionService::allow_all()),
            session,
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        };

        let results = execute_tools(
            &registry,
            ctx,
            vec![
                ("write".into(), "bash".into(), serde_json::json!({})),
                ("delegate".into(), "subagent".into(), serde_json::json!({})),
                ("status".into(), "gauntlet".into(), serde_json::json!({})),
            ],
            CancellationToken::new(),
            EventBus::new(8),
        )
        .await;

        assert!(!wrote.load(std::sync::atomic::Ordering::SeqCst));
        assert!(coordinated.load(std::sync::atomic::Ordering::SeqCst));
        for (id, result) in &results {
            if id == "status" {
                assert!(!result.is_error, "{}", result.content);
            } else {
                assert!(
                    result.is_error && result.content.contains("blocked"),
                    "{id}"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_gauntlet_start_in_the_same_batch_blocks_writes_like_a_workflow_start() {
        let wrote = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(WriteFlag(wrote.clone())));
        registry.register(Arc::new(GauntletFlag(Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        ))));
        let ctx = ToolCtx {
            cwd: ".".into(),
            store: None,
            permission: Arc::new(PermissionService::allow_all()),
            session: crate::session::SessionId(1),
            jobs: Arc::new(crate::job::LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        };
        let results = execute_tools(
            &registry,
            ctx,
            vec![
                ("write".into(), "bash".into(), serde_json::json!({})),
                (
                    "start".into(),
                    "gauntlet".into(),
                    serde_json::json!({"action":"start"}),
                ),
            ],
            CancellationToken::new(),
            EventBus::new(8),
        )
        .await;
        assert!(!wrote.load(std::sync::atomic::Ordering::SeqCst));
        assert!(results
            .iter()
            .find(|(id, _)| id == "write")
            .is_some_and(|(_, result)| result.is_error));
    }

    struct SubagentFlag(Arc<std::sync::atomic::AtomicBool>);
    #[async_trait]
    impl Tool for SubagentFlag {
        fn name(&self) -> &str {
            "subagent"
        }
        fn description(&self) -> &str {
            "start a child"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn run(
            &self,
            _ctx: ToolCtx,
            _input: serde_json::Value,
            _cancel: CancellationToken,
        ) -> crate::error::Result<ToolOutput> {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(ToolOutput::ok("delegated"))
        }
    }

    struct JobListFlag(Arc<std::sync::atomic::AtomicBool>);
    #[async_trait]
    impl Tool for JobListFlag {
        fn name(&self) -> &str {
            "job_list"
        }
        fn description(&self) -> &str {
            "poll job list"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn run(
            &self,
            _ctx: ToolCtx,
            _input: serde_json::Value,
            _cancel: CancellationToken,
        ) -> crate::error::Result<ToolOutput> {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(ToolOutput::ok("polled"))
        }
    }
}
