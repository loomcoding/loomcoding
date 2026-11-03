use super::*;

/// Lanzador de prueba con un hijo ya terminado (`listo`) y el proveedor grabador.
async fn lanzador_con_un_hijo_terminado(
    dir: &std::path::Path,
    nombre: &str,
) -> (
    InProcessSubagentLauncher,
    Arc<SqliteStore>,
    Arc<LocalJobRegistry>,
    SessionId,
    SessionId,
) {
    let (store, parent) = tienda_de_prueba(dir, nombre).await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let runtime = lanzador_grabador(dir, store.clone(), jobs.clone(), proveedor, None, None);
    let (anterior, job) = runtime
        .start(
            parent,
            "primero".into(),
            "haz la primera parte".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();
    espera_estado_del_hijo(&store, parent, anterior, TaskState::Completed).await;
    (runtime, store, jobs, parent, anterior)
}

#[tokio::test]
async fn a_follow_up_carries_the_previous_result_in_its_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, store, jobs, parent, anterior) =
        lanzador_con_un_hijo_terminado(dir.path(), "seguimiento.db").await;

    let (nuevo, job) = runtime
        .start_follow_up(
            parent,
            "segundo".into(),
            "ahora la segunda parte".into(),
            None,
            anterior,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let primero = store.messages(nuevo).await.unwrap()[0].text();
    assert!(
        primero.starts_with("[Follow-up of subagent"),
        "el prompt debería abrir con el encabezado, y es: {primero}"
    );
    assert!(primero.contains(&format!("subagent {} \"primero\"", anterior.0)));
    assert!(primero.contains("(TASK_STATE_COMPLETED)"));
    assert!(primero.contains("Previous result:\nlisto"));
    assert!(primero.ends_with("\n\nahora la segunda parte"));
    let hijos = store.list_subagents(parent).await.unwrap();
    let descriptor = hijos.iter().find(|d| d.child == nuevo).unwrap();
    assert_eq!(descriptor.reference_task_ids, vec![anterior]);
    let previo = hijos.iter().find(|d| d.child == anterior).unwrap();
    assert!(previo.reference_task_ids.is_empty());
}

#[tokio::test]
async fn a_follow_up_shares_the_context_id() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, store, jobs, parent, anterior) =
        lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-contexto.db").await;

    let (nuevo, job) = runtime
        .start_follow_up(
            parent,
            "segundo".into(),
            "sigue".into(),
            None,
            anterior,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let hijos = store.list_subagents(parent).await.unwrap();
    let contexto = |id: SessionId| hijos.iter().find(|d| d.child == id).unwrap().context_id();
    assert_eq!(contexto(nuevo), contexto(anterior));
}

#[tokio::test]
async fn a_follow_up_of_a_running_child_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, store, jobs, parent, _anterior) =
        lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-vivo.db").await;
    let vivo = hijo_que_no_se_detiene(&store, &jobs, parent).await;
    let antes = store.list_subagents(parent).await.unwrap().len();

    match runtime
        .start_follow_up(
            parent,
            "segundo".into(),
            "sigue".into(),
            None,
            vivo,
            CancellationToken::new(),
        )
        .await
    {
        Err(CoreError::Tool(texto)) => {
            assert!(texto.contains("TASK_STATE_WORKING"), "{texto}");
        }
        otro => panic!("se esperaba un error de herramienta, y salió {otro:?}"),
    }
    assert_eq!(store.list_subagents(parent).await.unwrap().len(), antes);
}

#[tokio::test]
async fn a_follow_up_of_another_parents_child_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, store, _jobs, parent, anterior) =
        lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-ajeno.db").await;
    let otro_padre = store.create_session("/project").await.unwrap();

    for referido in [anterior, SessionId(9999)] {
        match runtime
            .start_follow_up(
                otro_padre,
                "segundo".into(),
                "sigue".into(),
                None,
                referido,
                CancellationToken::new(),
            )
            .await
        {
            Err(CoreError::TaskNotFound(id)) => assert_eq!(id, referido.0),
            otro => panic!("se esperaba TaskNotFound, y salió {otro:?}"),
        }
    }
    assert!(store.list_subagents(otro_padre).await.unwrap().is_empty());
    assert_eq!(store.list_subagents(parent).await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_previous_result_is_capped() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, store, jobs, parent, _anterior) =
        lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-tope.db").await;
    let grande = store.create_session("/project").await.unwrap();
    store
        .save_subagent(&SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child: grande,
            job_id: "subagent-grande".into(),
            label: "grande".into(),
            status: TaskStatus::new(TaskState::Completed),
            artifacts: vec![Artifact::text_artifact("x".repeat(100_000))],
            notice_delivered: true,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();

    let (nuevo, job) = runtime
        .start_follow_up(
            parent,
            "segundo".into(),
            "sigue".into(),
            None,
            grande,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let primero = store.messages(nuevo).await.unwrap()[0].text();
    assert!(
        primero.len() < MAX_NOTICE_BYTES + 400,
        "el prompt de seguimiento mide {} bytes",
        primero.len()
    );
    assert!(primero.ends_with("\n\nsigue"));
}

#[tokio::test]
async fn a_follow_up_of_a_child_without_result_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, store, jobs, parent, _anterior) =
        lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-vacio.db").await;
    let fallido = store.create_session("/project").await.unwrap();
    store
        .save_subagent(&SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child: fallido,
            job_id: "subagent-fallido".into(),
            label: "fallido".into(),
            status: TaskStatus::new(TaskState::Failed),
            artifacts: Vec::new(),
            notice_delivered: true,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();

    let (nuevo, job) = runtime
        .start_follow_up(
            parent,
            "segundo".into(),
            "reintenta".into(),
            None,
            fallido,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let primero = store.messages(nuevo).await.unwrap()[0].text();
    assert!(primero.contains("(TASK_STATE_FAILED)"), "{primero}");
    assert!(
        primero.contains("Previous result:\n(no result)"),
        "{primero}"
    );
}

// ---------------------------------------------------------------- espejo de remotos (A8a, T5a)
