//! El responder del hijo: permisos delegados, preguntas por el buzón y sus eventos.

use super::support::*;
use super::*;

/// Responder del padre que cuenta las decisiones y siempre deniega.
struct CountingDeny(Arc<AtomicUsize>);

#[async_trait]
impl PermissionResponder for CountingDeny {
    async fn decide(&self, _req: &PermissionRequest) -> Decision {
        self.0.fetch_add(1, Ordering::SeqCst);
        Decision::Deny
    }
}

#[tokio::test]
async fn child_responder_delegates_permissions_to_the_parent() {
    let calls = Arc::new(AtomicUsize::new(0));
    let parent_service = Arc::new(PermissionService::with_responder(Box::new(CountingDeny(
        calls.clone(),
    ))));
    let (_dir, tracker) = tracker_vacio(&EventBus::new(8)).await;
    let child_service = PermissionService::with_responder(Box::new(ChildResponder::new(
        parent_service,
        Arc::new(InputBroker::default()),
        PARENT,
        CHILD,
        "obrero".into(),
        EventBus::new(8),
        tracker,
    )));
    let req = PermissionRequest {
        tool: "bash".into(),
        action: "run".into(),
        detail: String::new(),
    };
    assert_eq!(child_service.check(req).await, Decision::Deny);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn child_responder_publishes_requested_after_registration() {
    let broker = Arc::new(InputBroker::default());
    let bus = EventBus::new(8);
    let mut rx = bus.subscribe();
    let (_dir, tracker) = tracker_vacio(&bus).await;
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            Arc::new(PermissionService::allow_all()),
            broker.clone(),
            PARENT,
            CHILD,
            "obrero".into(),
            bus.clone(),
            tracker,
        ),
    )));
    let questions = vec![question("¿Cuál?")];
    let s = service.clone();
    let q = questions.clone();
    let handle = tokio::spawn(async move { s.ask_user(&q).await });

    match next_event(&mut rx).await {
        LiveEvent::SubagentInputRequested { .. } => {}
        other => panic!("evento inesperado: {other:?}"),
    }
    // Al recibir el aviso, la petición ya debe estar registrada en el buzón del padre.
    assert_eq!(broker.pending(PARENT).len(), 1);

    broker.answer(PARENT, CHILD, vec!["a".into()]).unwrap();
    assert!(handle.await.unwrap().is_some());
}

#[tokio::test]
async fn child_responder_ask_goes_through_the_broker_and_emits_events() {
    let broker = Arc::new(InputBroker::default());
    let bus = EventBus::new(8);
    let mut rx = bus.subscribe();
    let (_dir, tracker) = tracker_vacio(&bus).await;
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            Arc::new(PermissionService::allow_all()),
            broker.clone(),
            PARENT,
            CHILD,
            "obrero".into(),
            bus.clone(),
            tracker,
        ),
    )));
    let questions = vec![question("¿Cuál?")];
    let s = service.clone();
    let q = questions.clone();
    let handle = tokio::spawn(async move { s.ask_user(&q).await });

    match next_event(&mut rx).await {
        LiveEvent::SubagentInputRequested {
            parent,
            child,
            label,
        } => {
            assert_eq!(parent, PARENT);
            assert_eq!(child, CHILD);
            assert_eq!(label, "obrero");
        }
        other => panic!("evento inesperado: {other:?}"),
    }
    wait_for_pending(&broker, PARENT).await;

    broker.answer(PARENT, CHILD, vec!["a".into()]).unwrap();
    assert_eq!(
        handle.await.unwrap(),
        Some(vec![Answer {
            question: "¿Cuál?".into(),
            selected: vec!["a".into()],
        }])
    );
    match next_event(&mut rx).await {
        LiveEvent::SubagentInputResolved { parent, child } => {
            assert_eq!(parent, PARENT);
            assert_eq!(child, CHILD);
        }
        other => panic!("evento inesperado: {other:?}"),
    }
}

/// Almacén SQLite con un padre y un hijo ya lanzado en `Working`, para observar sus estados.
async fn almacen_con_hijo(
    dir: &std::path::Path,
) -> (
    Arc<crate::session::sqlite::SqliteStore>,
    SessionId,
    SessionId,
) {
    let store = Arc::new(
        crate::session::sqlite::SqliteStore::open(dir.join("hijo.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let child = store.create_session("/project").await.unwrap();
    store
        .save_subagent(&crate::session::SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child,
            job_id: "subagent-1".into(),
            label: "obrero".into(),
            status: crate::task::TaskStatus::new(crate::task::TaskState::Working),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();
    (store, parent, child)
}

/// Estado de tarea del hijo tal como lo guarda el almacén.
async fn estado_guardado(
    store: &dyn crate::session::SessionStore,
    parent: SessionId,
    child: SessionId,
) -> crate::task::TaskStatus {
    store
        .list_subagents(parent)
        .await
        .unwrap()
        .into_iter()
        .find(|descriptor| descriptor.child == child)
        .expect("el hijo debe estar guardado")
        .status
}

/// Espera, con cota de tiempo, a que el almacén registre el estado pedido.
async fn espera_estado(
    store: &dyn crate::session::SessionStore,
    parent: SessionId,
    child: SessionId,
    estado: crate::task::TaskState,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while estado_guardado(store, parent, child).await.state != estado {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("el hijo debería quedar en {estado:?}"));
}

/// Lanzador de permisos del padre que no decide hasta que se le avisa; sirve para
/// observar el estado `auth-required` del hijo mientras espera.
struct EsperaDecision(Arc<tokio::sync::Notify>);

#[async_trait]
impl PermissionResponder for EsperaDecision {
    async fn decide(&self, _req: &PermissionRequest) -> Decision {
        self.0.notified().await;
        Decision::Allow
    }

    fn requires_user(&self) -> bool {
        true
    }
}

/// Recoge los estados `auth-required` que llegan al bus tras `rx`, sin esperar.
fn auth_required_publicados(
    rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
) -> Vec<TaskState> {
    let mut vistos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::SubagentChanged(descriptor) = evento {
            if descriptor.status.state == TaskState::AuthRequired {
                vistos.push(descriptor.status.state);
            }
        }
    }
    vistos
}

#[tokio::test]
async fn child_decide_does_not_mark_auth_required_when_the_parent_allows_without_asking() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    let service = PermissionService::with_responder(Box::new(ChildResponder::new(
        Arc::new(PermissionService::allow_all()),
        Arc::new(InputBroker::default()),
        parent,
        child,
        "obrero".into(),
        bus,
        tracker,
    )));

    let decision = service
        .check(PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: "ls".into(),
        })
        .await;
    assert_eq!(decision, Decision::Allow);
    assert!(
        auth_required_publicados(&mut rx).is_empty(),
        "sin aprobación humana no hay auth-required"
    );
    assert_eq!(
        estado_guardado(store.as_ref(), parent, child).await.state,
        TaskState::Working
    );
}

#[tokio::test]
async fn child_decide_does_not_mark_auth_required_when_the_permission_was_granted_for_the_session()
{
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    // El padre pediría al usuario, pero ya concedió `bash:run` en la sesión.
    let parent_service = Arc::new(PermissionService::with_responder(Box::new(EsperaDecision(
        Arc::new(tokio::sync::Notify::new()),
    ))));
    parent_service.grant_persistent("bash", "run").await;
    let service = PermissionService::with_responder(Box::new(ChildResponder::new(
        parent_service,
        Arc::new(InputBroker::default()),
        parent,
        child,
        "obrero".into(),
        bus,
        tracker,
    )));

    let decision = service
        .check(PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: "ls".into(),
        })
        .await;
    assert_eq!(decision, Decision::Allow);
    assert!(auth_required_publicados(&mut rx).is_empty());
}

#[tokio::test]
async fn child_decide_marks_auth_required_when_the_parent_will_prompt() {
    use crate::task::TaskState;
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(8);
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    let liberar = Arc::new(tokio::sync::Notify::new());
    let parent_service = Arc::new(PermissionService::with_responder(Box::new(EsperaDecision(
        liberar.clone(),
    ))));
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            parent_service,
            Arc::new(InputBroker::default()),
            parent,
            child,
            "obrero".into(),
            bus,
            tracker,
        ),
    )));
    let s = service.clone();
    let handle = tokio::spawn(async move {
        s.check(PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: "ls -la".into(),
        })
        .await
    });

    espera_estado(store.as_ref(), parent, child, TaskState::AuthRequired).await;
    liberar.notify_one();
    assert_eq!(handle.await.unwrap(), Decision::Allow);
    espera_estado(store.as_ref(), parent, child, TaskState::Working).await;
}

#[tokio::test]
async fn ask_moves_the_child_to_input_required_and_back() {
    use crate::task::TaskState;
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(8);
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    let broker = Arc::new(InputBroker::default());
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            Arc::new(PermissionService::allow_all()),
            broker.clone(),
            parent,
            child,
            "obrero".into(),
            bus,
            tracker,
        ),
    )));
    let questions = vec![question("¿Cuál?")];
    let s = service.clone();
    let q = questions.clone();
    let handle = tokio::spawn(async move { s.ask_user(&q).await });

    espera_estado(store.as_ref(), parent, child, TaskState::InputRequired).await;
    assert_eq!(
        estado_guardado(store.as_ref(), parent, child)
            .await
            .message_text(),
        Some("¿Cuál?".into())
    );
    wait_for_pending(&broker, parent).await;

    broker.answer(parent, child, vec!["a".into()]).unwrap();
    assert!(handle.await.unwrap().is_some());
    assert_eq!(
        estado_guardado(store.as_ref(), parent, child).await.state,
        TaskState::Working
    );
}

#[tokio::test]
async fn ask_resolves_input_required_even_if_cancelled() {
    use crate::task::TaskState;
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(8);
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    let broker = Arc::new(InputBroker::default());
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            Arc::new(PermissionService::allow_all()),
            broker.clone(),
            parent,
            child,
            "obrero".into(),
            bus,
            tracker,
        ),
    )));
    let questions = vec![question("¿Cuál?")];
    let s = service.clone();
    let q = questions.clone();
    let handle = tokio::spawn(async move { s.ask_user(&q).await });

    espera_estado(store.as_ref(), parent, child, TaskState::InputRequired).await;
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
    espera_estado(store.as_ref(), parent, child, TaskState::Working).await;
    assert!(broker.pending(parent).is_empty());
}

#[tokio::test]
async fn decide_moves_the_child_to_auth_required_with_a_summary_and_back() {
    use crate::task::TaskState;
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(8);
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    let liberar = Arc::new(tokio::sync::Notify::new());
    let parent_service = Arc::new(PermissionService::with_responder(Box::new(EsperaDecision(
        liberar.clone(),
    ))));
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            parent_service,
            Arc::new(InputBroker::default()),
            parent,
            child,
            "obrero".into(),
            bus,
            tracker,
        ),
    )));
    let s = service.clone();
    let handle = tokio::spawn(async move {
        s.check(PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: "ls -la".into(),
        })
        .await
    });

    espera_estado(store.as_ref(), parent, child, TaskState::AuthRequired).await;
    let resumen = estado_guardado(store.as_ref(), parent, child).await;
    let mensaje = resumen
        .message_text()
        .expect("auth-required lleva un resumen");
    assert!(mensaje.contains("bash"), "{mensaje}");

    liberar.notify_one();
    assert_eq!(handle.await.unwrap(), Decision::Allow);
    assert_eq!(
        estado_guardado(store.as_ref(), parent, child).await.state,
        TaskState::Working
    );
}

#[tokio::test]
async fn decide_resolves_auth_required_even_if_cancelled() {
    use crate::task::TaskState;
    let dir = tempfile::tempdir().unwrap();
    let (store, parent, child) = almacen_con_hijo(dir.path()).await;
    let bus = EventBus::new(8);
    let tracker = Arc::new(crate::subagent::TaskTracker::new(
        store.clone(),
        bus.clone(),
    ));
    let liberar = Arc::new(tokio::sync::Notify::new());
    let parent_service = Arc::new(PermissionService::with_responder(Box::new(EsperaDecision(
        liberar,
    ))));
    let service = Arc::new(PermissionService::with_responder(Box::new(
        ChildResponder::new(
            parent_service,
            Arc::new(InputBroker::default()),
            parent,
            child,
            "obrero".into(),
            bus,
            tracker,
        ),
    )));
    let s = service.clone();
    let handle = tokio::spawn(async move {
        s.check(PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: "ls".into(),
        })
        .await
    });

    espera_estado(store.as_ref(), parent, child, TaskState::AuthRequired).await;
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
    espera_estado(store.as_ref(), parent, child, TaskState::Working).await;
}

#[test]
fn auth_summary_names_the_tool_and_trims_the_arguments() {
    let corto = resumen_de_permiso("bash", "run", "ls");
    assert!(corto.contains("bash") && corto.contains("run"), "{corto}");

    let largo = resumen_de_permiso("bash", "run", &"x".repeat(2_000));
    assert!(largo.starts_with("bash"), "{largo}");
    assert!(largo.chars().count() <= 200, "no se recorta: {largo}");
}
