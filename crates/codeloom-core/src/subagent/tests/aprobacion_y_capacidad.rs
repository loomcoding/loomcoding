use super::*;

async fn delegating_with(
    responder: Option<FixedAnswer>,
    confirm: ConfirmDelegation,
) -> (Result<(SessionId, JobId)>, usize) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("confirm.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let mut runtime = echo_launcher(
        dir.path(),
        store.clone(),
        Arc::new(LocalJobRegistry::default()),
        EventBus::new(64),
        vec![vec![
            StreamChunk::TextDelta("hecho".into()),
            StreamChunk::Done(FinishReason::Stop),
        ]],
    )
    .with_delegation_confirm(confirm);
    runtime.permission = Arc::new(match responder {
        Some(responder) => PermissionService::with_responder(Box::new(responder)),
        None => PermissionService::allow_all(),
    });
    let result = runtime
        .start(
            parent,
            "revisar".into(),
            "revisa el módulo".into(),
            None,
            CancellationToken::new(),
        )
        .await;
    let hijos = store.list_subagents(parent).await.unwrap().len();
    (result, hijos)
}

#[tokio::test]
async fn always_launches_only_after_an_explicit_approval() {
    let (resultado, hijos) = delegating_with(
        Some(FixedAnswer(Some("Approve"))),
        ConfirmDelegation::Always,
    )
    .await;
    assert!(resultado.is_ok(), "{resultado:?}");
    assert_eq!(hijos, 1);
}

#[tokio::test]
async fn always_denies_on_cancel_without_leaving_a_child() {
    let (resultado, hijos) =
        delegating_with(Some(FixedAnswer(Some("Cancel"))), ConfirmDelegation::Always).await;
    let error = resultado.unwrap_err().to_string();
    assert!(error.contains("not approved"), "{error}");
    assert_eq!(hijos, 0, "una denegación no deja sesión hija ni tarea");
}

#[tokio::test]
async fn always_denies_when_nobody_can_answer_as_under_yolo() {
    // `allow_all` (yolo) concede permisos pero no responde preguntas.
    let (sin_responder, hijos) = delegating_with(None, ConfirmDelegation::Always).await;
    assert!(sin_responder.is_err());
    assert_eq!(hijos, 0);
    let (descartada, _) = delegating_with(Some(FixedAnswer(None)), ConfirmDelegation::Always).await;
    assert!(descartada.is_err());
}

#[tokio::test]
async fn other_confirmation_modes_do_not_ask() {
    for modo in [ConfirmDelegation::Doubt, ConfirmDelegation::Never] {
        let (resultado, hijos) = delegating_with(None, modo).await;
        assert!(resultado.is_ok(), "{modo:?}: {resultado:?}");
        assert_eq!(hijos, 1);
    }
}

#[tokio::test]
async fn configured_capacity_replaces_the_default_of_eight() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("capacity.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let runtime = echo_launcher(
        dir.path(),
        store.clone(),
        Arc::new(LocalJobRegistry::default()),
        EventBus::new(64),
        vec![],
    )
    .with_max_active(2);
    let _ocupados: Vec<_> = (0..2)
        .map(|_| runtime.active.clone().try_acquire_owned().unwrap())
        .collect();
    let error = runtime
        .start(
            parent,
            "tercero".into(),
            "hazlo".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("maximum of 2 active subagents reached"),
        "{error}"
    );
}

#[tokio::test]
async fn launcher_rejects_when_capacity_is_exhausted_and_records_a_rejected_task() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("rejected.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let runtime = echo_launcher(
        dir.path(),
        store.clone(),
        Arc::new(LocalJobRegistry::default()),
        bus,
        vec![],
    );
    // Se ocupan los ocho puestos a mano: el noveno hijo debe rechazarse.
    let _ocupados: Vec<_> = (0..8)
        .map(|_| runtime.active.clone().try_acquire_owned().unwrap())
        .collect();

    let error = runtime
        .start(
            parent,
            "noveno".into(),
            "hazlo".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("maximum of 8 active subagents reached"),
        "{error}"
    );

    let hijos = store.list_subagents(parent).await.unwrap();
    assert_eq!(hijos.len(), 1, "queda una tarea rechazada");
    let hijo = &hijos[0];
    assert_eq!(hijo.status.state, TaskState::Rejected);
    assert_eq!(
        hijo.status.message_text().as_deref(),
        Some("maximum of 8 active subagents reached")
    );
    assert!(hijo.job_id.is_empty(), "un rechazado no tiene trabajo");

    let publicado = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(LiveEvent::SubagentChanged(descriptor)) if descriptor.child == hijo.child => {
                    return descriptor.status.state;
                }
                Ok(_) => {}
                Err(error) => panic!("el bus debería seguir abierto: {error}"),
            }
        }
    })
    .await
    .expect("el rechazo debería publicarse");
    assert_eq!(publicado, TaskState::Rejected);
}

#[tokio::test]
async fn launcher_validation_errors_create_no_task() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("validation.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let runtime = echo_launcher(
        dir.path(),
        store.clone(),
        Arc::new(LocalJobRegistry::default()),
        EventBus::new(16),
        vec![],
    );

    let casos = [
        ("obrero".to_string(), "   ".to_string()),
        ("obrero".to_string(), "x".repeat(32_769)),
        ("x".repeat(129), "hola".to_string()),
    ];
    for (etiqueta, prompt) in casos {
        assert!(
            runtime
                .start(parent, etiqueta, prompt, None, CancellationToken::new())
                .await
                .is_err(),
            "debería rechazarse la entrada inválida"
        );
    }
    assert!(
        store.list_subagents(parent).await.unwrap().is_empty(),
        "la validación no crea tarea"
    );
}

#[tokio::test]
async fn interrupt_moves_the_task_to_canceled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("interrupt_canceled.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let jobs = Arc::new(LocalJobRegistry::default());
    let runtime = lanzador_que_pregunta(dir.path(), store.clone(), jobs.clone(), bus);
    let (child, _job) = runtime
        .start(
            parent,
            "obrero".into(),
            "decide algo".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.pending_inputs(parent).is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("el hijo debería quedar esperando una respuesta");

    runtime
        .control(parent, child, "interrupt", None)
        .await
        .unwrap();
    espera_estado_del_hijo(&store, parent, child, TaskState::Canceled).await;
    let publicado = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(LiveEvent::SubagentChanged(descriptor))
                    if descriptor.child == child
                        && descriptor.status.state == TaskState::Canceled =>
                {
                    return true;
                }
                Ok(_) => {}
                Err(error) => panic!("el bus debería seguir abierto: {error}"),
            }
        }
    })
    .await
    .expect("la cancelación debería publicarse");
    assert!(publicado);
}

#[tokio::test]
async fn parent_cancel_moves_the_child_to_canceled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("parent_cancel.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let runtime = lanzador_que_pregunta(dir.path(), store.clone(), jobs.clone(), EventBus::new(64));
    let padre = CancellationToken::new();
    let (child, job) = runtime
        .start(
            parent,
            "obrero".into(),
            "decide algo".into(),
            None,
            padre.clone(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.pending_inputs(parent).is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("el hijo debería quedar esperando una respuesta");

    // Esc del padre: cancela su turno, y con él al hijo, que no puede terminar en `completed`.
    padre.cancel();
    let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
        .await
        .expect("el hijo debería terminar al cancelar al padre")
        .unwrap();
    assert_eq!(outcome.status, JobStatus::Killed);
    espera_estado_del_hijo(&store, parent, child, TaskState::Canceled).await;
    assert!(runtime.pending_inputs(parent).is_empty());
}

/// Proveedor que responde `listo` declarando `entrada` tokens de consumo.
struct ProveedorConUso {
    entrada: u32,
}

#[async_trait]
impl Provider for ProveedorConUso {
    fn capabilities(&self) -> crate::provider::Capabilities {
        crate::provider::Capabilities {
            tool_use: false,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        _req: crate::provider::ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<crate::provider::ProviderStream> {
        Ok(Box::pin(futures::stream::iter(vec![
            StreamChunk::TextDelta("listo".into()),
            StreamChunk::Usage(crate::message::Usage {
                input_tokens: self.entrada,
                output_tokens: 0,
                reported_cost_usd: None,
            }),
            StreamChunk::Done(FinishReason::Stop),
        ])))
    }
}

#[tokio::test]
async fn a_child_inherits_the_budget_and_its_warning_reaches_the_parent_bus() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "presupuesto.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let provider: Arc<dyn Provider> = Arc::new(ProveedorConUso { entrada: 900 });
    let runtime = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.path().to_path_buf(),
        jobs.clone(),
        bus,
    )
    .with_budget(BudgetConfig {
        session_tokens: Some(1000),
        session_cost_usd: None,
        hard_stop: false,
    });

    let (_, job) = runtime
        .start(
            parent,
            "obrero".into(),
            "tarea".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let aviso = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(LiveEvent::ChildEvent { event, .. }) => {
                    if let LiveEvent::BudgetWarning {
                        level_percent,
                        used,
                        ..
                    } = *event
                    {
                        return (level_percent, used);
                    }
                }
                Ok(_) => {}
                Err(error) => panic!("el bus debería seguir abierto: {error}"),
            }
        }
    })
    .await
    .expect("el aviso del hijo debería llegar al bus del padre");
    assert_eq!(aviso, (80, 900.0));
}

#[tokio::test]
async fn route_without_a_router_abstains() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "r1.db").await;
    let lanzador = lanzador_con_enrutador(dir.path(), store, None);

    assert_eq!(lanzador.route(parent, "t", "revisa").await.unwrap(), None);
}
