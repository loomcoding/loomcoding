use super::*;

fn decision_para(agente: &str) -> RouteDecision {
    RouteDecision {
        agent: agente.into(),
        reason: "prueba".into(),
        source: crate::registry::router::RouteSource::Rule,
    }
}

#[tokio::test]
async fn route_returns_the_routers_decision_for_an_approved_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "r2.db").await;
    for modo in [ConfirmDelegation::Doubt, ConfirmDelegation::Never] {
        let lanzador = lanzador_con_enrutador(
            dir.path(),
            store.clone(),
            Some((Ok(Some(decision_para("revisor"))), modo)),
        );

        let decision = lanzador.route(parent, "t", "revisa").await.unwrap();

        assert_eq!(decision.map(|d| d.agent), Some("revisor".to_string()));
    }
}

#[tokio::test]
async fn always_mode_still_asks_so_the_router_does_not_decide() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "r3.db").await;
    let lanzador = lanzador_con_enrutador(
        dir.path(),
        store,
        Some((
            Ok(Some(decision_para("revisor"))),
            ConfirmDelegation::Always,
        )),
    );

    assert_eq!(lanzador.route(parent, "t", "revisa").await.unwrap(), None);
}

#[tokio::test]
async fn an_invented_agent_name_or_a_failing_router_never_blocks_delegation() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "r4.db").await;
    let inventado = lanzador_con_enrutador(
        dir.path(),
        store.clone(),
        Some((
            Ok(Some(decision_para("no-existe"))),
            ConfirmDelegation::Doubt,
        )),
    );
    let roto = lanzador_con_enrutador(
        dir.path(),
        store,
        Some((Err(CoreError::Tool("x".into())), ConfirmDelegation::Doubt)),
    );

    assert_eq!(inventado.route(parent, "t", "revisa").await.unwrap(), None);
    assert_eq!(roto.route(parent, "t", "revisa").await.unwrap(), None);
}

#[tokio::test]
async fn cancel_returns_the_task_canceled_and_drops_its_question() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "cancel-ok.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let runtime = lanzador_que_pregunta(dir.path(), store.clone(), jobs.clone(), EventBus::new(64));
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

    // Cancelable en `input-required` (D3): la tarea actualizada vuelve ya asentada.
    let tarea = runtime.cancel(parent, child).await.unwrap();

    assert_eq!(tarea.child, child);
    assert_eq!(tarea.status.state, TaskState::Canceled);
    assert!(runtime.pending_inputs(parent).is_empty());

    // Una tarea ya cancelada no vuelve a ser cancelable (D4).
    match runtime.cancel(parent, child).await {
        Err(CoreError::TaskNotCancelable { child: id, state }) => {
            assert_eq!((id, state), (child.0, TaskState::Canceled));
        }
        otro => panic!("se esperaba TaskNotCancelable, y salió {otro:?}"),
    }
}

#[tokio::test]
async fn cancel_of_a_completed_child_is_not_cancelable() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "cancel-completed.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let runtime = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        None,
        None,
    );
    let (child, job) = runtime
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
    espera_estado_del_hijo(&store, parent, child, TaskState::Completed).await;

    match runtime.cancel(parent, child).await {
        Err(CoreError::TaskNotCancelable { state, .. }) => {
            assert_eq!(state, TaskState::Completed);
        }
        otro => panic!("se esperaba TaskNotCancelable, y salió {otro:?}"),
    }
}

#[tokio::test]
async fn cancel_of_a_child_of_another_parent_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "cancel-ajeno.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let runtime = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        None,
        None,
    );
    let (child, job) = runtime
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
    let otro_padre = store.create_session("/project").await.unwrap();

    match runtime.cancel(otro_padre, child).await {
        Err(CoreError::TaskNotFound(id)) => assert_eq!(id, child.0),
        otro => panic!("se esperaba TaskNotFound, y salió {otro:?}"),
    }
    match runtime.cancel(parent, SessionId(9999)).await {
        Err(CoreError::TaskNotFound(id)) => assert_eq!(id, 9999),
        otro => panic!("se esperaba TaskNotFound, y salió {otro:?}"),
    }
}

#[tokio::test]
async fn cancel_does_not_claim_success_if_the_child_does_not_settle() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "cancel-terco.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let runtime = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        None,
        None,
    );
    let runtime = runtime.with_cancel_wait(Duration::from_millis(150));
    let child = hijo_que_no_se_detiene(&store, &jobs, parent).await;

    let tarea = runtime.cancel(parent, child).await.unwrap();

    assert_eq!(
        tarea.status.state,
        TaskState::Working,
        "no se asentó: no puede decirse que se canceló"
    );
    // El trabajo recibió la orden de parar (D2) pero sigue deteniéndose.
    let job = JobId(tarea.job_id.clone());
    let estado = jobs
        .list(parent)
        .into_iter()
        .find(|snapshot| snapshot.id == job)
        .unwrap()
        .status;
    assert_eq!(estado, JobStatus::Stopping);
}

#[tokio::test]
async fn a_repeated_cancel_while_stopping_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "cancel-repetido.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let runtime = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        None,
        None,
    );
    let runtime = runtime.with_cancel_wait(Duration::from_millis(150));
    let child = hijo_que_no_se_detiene(&store, &jobs, parent).await;

    let primera = runtime.cancel(parent, child).await.unwrap();
    let segunda = runtime
        .cancel(parent, child)
        .await
        .expect("repetir la orden mientras se detiene no es un error (D4)");

    assert_eq!(primera.status.state, TaskState::Working);
    assert_eq!(segunda.status.state, TaskState::Working);
}

#[tokio::test]
async fn start_without_registry_behaves_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "sin-registro.db").await;
    let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: peticiones.clone(),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        None,
        None,
    );

    // Sin agente y con `default`, sin registro: el mismo camino de siempre.
    for agente in [None, Some(AGENTE_POR_DEFECTO.to_string())] {
        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                agente,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();
    }

    let vistas = peticiones.lock().unwrap().clone();
    assert_eq!(vistas.len(), 2);
    for (modelo, sistema, esfuerzo) in vistas {
        assert_eq!(modelo, "test", "el modelo del padre");
        assert_eq!(esfuerzo.as_deref(), Some("low"), "el esfuerzo del padre");
        assert_eq!(
            sistema,
            crate::config::context::load_context_files(dir.path()).join("\n\n"),
            "el sistema es el contexto del proyecto, sin añadidos"
        );
    }
    assert!(store
        .list_subagents(parent)
        .await
        .unwrap()
        .iter()
        .all(|hijo| hijo.agent.is_none() && hijo.agent_origin.is_none()));
}
