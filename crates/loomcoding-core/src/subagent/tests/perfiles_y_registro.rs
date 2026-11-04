use super::*;

#[tokio::test]
async fn start_with_a_named_agent_without_registry_is_a_validation_error() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "sin-registro-error.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(dir.path(), store.clone(), jobs, proveedor, None, None);

    let error = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("obrero".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("obrero"), "{error}");
    assert!(store.list_subagents(parent).await.unwrap().is_empty());
}

#[tokio::test]
async fn start_without_agent_uses_default_and_inherits_model() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "defecto.db").await;
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
        Some(registro_global(global.path())),
        None,
    );

    let (_child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let (modelo, sistema, esfuerzo) = peticiones.lock().unwrap()[0].clone();
    assert_eq!(modelo, "test");
    assert_eq!(esfuerzo.as_deref(), Some("low"));
    assert_eq!(
        sistema,
        crate::config::context::load_context_files(dir.path()).join("\n\n"),
        "`default` no aporta prompt de sistema propio"
    );
    let hijo = &store.list_subagents(parent).await.unwrap()[0];
    assert_eq!(hijo.agent.as_deref(), Some(AGENTE_POR_DEFECTO));
    assert_eq!(hijo.agent_origin.as_deref(), Some("built-in"));
}

#[tokio::test]
async fn start_with_a_file_agent_applies_persona_model_and_effort() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(global.path(), "frontend", PERFIL_CON_PERSONA);
    let (store, parent) = tienda_de_prueba(dir.path(), "perfil.db").await;
    let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: peticiones.clone(),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store,
        jobs.clone(),
        proveedor,
        Some(registro_global(global.path())),
        None,
    );

    let (_child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("frontend".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let (modelo, sistema, esfuerzo) = peticiones.lock().unwrap()[0].clone();
    assert_eq!(modelo, "modelo-perfil");
    assert_eq!(esfuerzo.as_deref(), Some("high"));
    assert!(sistema.contains("Rol: Ingeniero\n"), "{sistema}");
    assert!(sistema.contains("Cuerpo del agente."), "{sistema}");
}

#[tokio::test]
async fn local_profile_provider_override_does_not_change_the_parent_provider() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(
            global.path(),
            "backend",
            "---\nname: backend\ndescription: Rust\nmodel: recursive/flint-v1.0:free\nprovider:\n  kind: openai_compat\n  base_url: https://openrouter.ai/api/v1\n  api_key_env: OPENROUTER_API_KEY\n---\n\nBackend.\n",
        );
    let (store, parent) = tienda_de_prueba(dir.path(), "proveedor-perfil.db").await;
    let parent_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let child_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let jobs = Arc::new(LocalJobRegistry::default());
    let parent_provider = ProveedorGrabador {
        peticiones: parent_requests.clone(),
        puerta: None,
    };
    let child_requests_for_resolver = child_requests.clone();
    let resolver: Arc<ProfileProviderResolver> = Arc::new(move |config| {
        assert_eq!(config.base_url, "https://openrouter.ai/api/v1");
        Ok(Arc::new(ProveedorGrabador {
            peticiones: child_requests_for_resolver.clone(),
            puerta: None,
        }))
    });
    let lanzador = lanzador_grabador(
        dir.path(),
        store,
        jobs.clone(),
        parent_provider,
        Some(registro_global(global.path())),
        None,
    )
    .with_profile_provider_resolver(resolver);

    let (_, job) = lanzador
        .start(
            parent,
            "backend".into(),
            "implementa una función".into(),
            Some("backend".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    assert!(parent_requests.lock().unwrap().is_empty());
    assert_eq!(
        child_requests.lock().unwrap()[0].0,
        "recursive/flint-v1.0:free"
    );
}

struct ProveedorFallido;

#[async_trait]
impl Provider for ProveedorFallido {
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
        Err(CoreError::Provider("mock backend failure".into()))
    }
}

#[tokio::test]
async fn local_subagent_failure_is_persisted_for_diagnosis() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "fallo-diagnostico.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let provider: Arc<dyn Provider> = Arc::new(ProveedorFallido);
    let runtime = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.path().to_path_buf(),
        jobs.clone(),
        EventBus::new(64),
    );

    let (child, job) = runtime
        .start(
            parent,
            "backend".into(),
            "revisa arquitectura".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let outcome = jobs.wait(parent, &job).await.unwrap();
    assert_eq!(outcome.status, crate::job::JobStatus::Failed);
    espera_estado_del_hijo(&store, parent, child, TaskState::Failed).await;

    let descriptor = store
        .list_subagents(parent)
        .await
        .unwrap()
        .into_iter()
        .find(|descriptor| descriptor.child == child)
        .unwrap();
    assert_eq!(descriptor.status.state, TaskState::Failed);
    assert!(descriptor
        .status
        .message_text()
        .unwrap()
        .contains("mock backend failure"));
}

#[tokio::test]
async fn start_with_missing_model_in_profile_inherits_from_parent() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(
        global.path(),
        "revisor",
        "---\nname: revisor\ndescription: Revisa\n---\n\nRevisa con cuidado.\n",
    );
    let (store, parent) = tienda_de_prueba(dir.path(), "herencia.db").await;
    let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: peticiones.clone(),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store,
        jobs.clone(),
        proveedor,
        Some(registro_global(global.path())),
        None,
    );

    let (_child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("revisor".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let (modelo, sistema, esfuerzo) = peticiones.lock().unwrap()[0].clone();
    assert_eq!(modelo, "test", "sin modelo en el perfil, el del padre");
    assert_eq!(
        esfuerzo.as_deref(),
        Some("low"),
        "sin esfuerzo, el del padre"
    );
    assert!(sistema.contains("Revisa con cuidado."), "{sistema}");
}

#[tokio::test]
async fn start_with_unknown_agent_creates_no_task() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "inexistente.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs,
        proveedor,
        Some(registro_global(global.path())),
        None,
    );

    let error = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("no-existe".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("no-existe"), "{error}");
    assert!(store.list_subagents(parent).await.unwrap().is_empty());
}

#[tokio::test]
async fn start_with_pending_project_agent_creates_no_task_and_says_so() {
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(
        proyecto.path(),
        "revisor",
        &PERFIL_CON_PERSONA.replace("frontend", "revisor"),
    );
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(
            proyecto.path().to_path_buf(),
            ProfileLevel::Project,
        )),
    ])
    .with_trust(almacen, proyecto.path().to_path_buf(), false);
    let (store, parent) = tienda_de_prueba(dir.path(), "pendiente.db").await;
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store.clone(),
        Arc::new(LocalJobRegistry::default()),
        proveedor,
        Some(Arc::new(registro)),
        None,
    );

    let error = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("revisor".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(
        error.to_string().contains("pendiente de aprobación"),
        "{error}"
    );
    assert!(error.to_string().contains("revisor"), "{error}");
    assert!(store.list_subagents(parent).await.unwrap().is_empty());
}

#[tokio::test]
async fn start_with_remote_agent_creates_no_task() {
    let dir = tempfile::tempdir().unwrap();
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FuenteRemotaDePrueba),
    ]);
    let (store, parent) = tienda_de_prueba(dir.path(), "remoto.db").await;
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store.clone(),
        Arc::new(LocalJobRegistry::default()),
        proveedor,
        Some(Arc::new(registro)),
        None,
    );

    let error = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("remoto".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(
        error.to_string().contains("agente remoto no disponible"),
        "{error}"
    );
    assert!(store.list_subagents(parent).await.unwrap().is_empty());
}

#[tokio::test]
async fn start_with_model_unknown_to_catalog_is_a_validation_error() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(
        global.path(),
        "inventado",
        "---\nname: inventado\ndescription: Modelo falso\nmodel: m-inventado\n---\n\nCuerpo.\n",
    );
    escribir_perfil(
        global.path(),
        "real",
        "---\nname: real\ndescription: Modelo conocido\nmodel: m-real\n---\n\nCuerpo.\n",
    );
    // El catálogo tiene un proveedor con la URL base del padre (`http://localhost`).
    let catalogo = Catalog::from_json(
        r#"{"p": {"id": "p", "npm": "@ai-sdk/openai-compatible", "api": "http://localhost",
                "models": {"m-real": {"id": "m-real", "tool_call": true}}}}"#,
    )
    .unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "catalogo.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        Some(registro_global(global.path())),
        Some(catalogo),
    );

    let error = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("inventado".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("m-inventado"), "{error}");
    assert!(store.list_subagents(parent).await.unwrap().is_empty());

    let (_child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("real".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();
}

#[tokio::test]
async fn reload_after_start_does_not_change_the_running_task() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(
        global.path(),
        "frontend",
        "---\nname: frontend\ndescription: UI\nmodel: modelo-v1\n---\n\nv1.\n",
    );
    let (store, parent) = tienda_de_prueba(dir.path(), "recarga.db").await;
    let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
    let puerta = Arc::new(tokio::sync::Notify::new());
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: peticiones.clone(),
        puerta: Some(puerta.clone()),
    };
    let registro = registro_global(global.path());
    let lanzador = lanzador_grabador(
        dir.path(),
        store,
        jobs.clone(),
        proveedor,
        Some(registro.clone()),
        None,
    );

    let (_child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("frontend".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // El hijo queda retenido en la puerta, con su petición ya registrada.
    tokio::time::timeout(Duration::from_secs(5), async {
        while peticiones.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("el hijo debería haber hecho su petición");

    escribir_perfil(
        global.path(),
        "frontend",
        "---\nname: frontend\ndescription: UI\nmodel: modelo-v2\n---\n\nv2.\n",
    );
    registro.reload();
    let recargado = match &registro.snapshot().get("frontend").unwrap().execution {
        Execution::Local(spec) => spec.model.clone(),
        Execution::Remote(_) => None,
    };
    assert_eq!(
        recargado.as_deref(),
        Some("modelo-v2"),
        "la recarga sí cambió"
    );

    puerta.notify_one();
    let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
        .await
        .expect("el hijo debería terminar")
        .unwrap();
    assert_eq!(outcome.status, JobStatus::Completed);
    assert_eq!(
        peticiones.lock().unwrap()[0].0,
        "modelo-v1",
        "la tarea en curso no cambia"
    );
}

#[tokio::test]
async fn descriptor_records_agent_name_and_origin() {
    let global = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    escribir_perfil(global.path(), "frontend", PERFIL_CON_PERSONA);
    let (store, parent) = tienda_de_prueba(dir.path(), "descriptor.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir.path(),
        store.clone(),
        jobs.clone(),
        proveedor,
        Some(registro_global(global.path())),
        None,
    );

    let (_child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "hazlo".into(),
            Some("frontend".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();

    let hijo = &store.list_subagents(parent).await.unwrap()[0];
    assert_eq!(hijo.agent.as_deref(), Some("frontend"));
    let origen = AgentOrigin::File {
        path: global.path().join("frontend.md"),
        level: ProfileLevel::Global,
    };
    assert_eq!(
        hijo.agent_origin.as_deref(),
        Some(origen.as_text().as_str())
    );
}

#[tokio::test]
async fn tracker_refuses_to_leave_a_terminal_state() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("tracker.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let child = store.create_session("/project").await.unwrap();
    store
        .save_subagent(&SubagentDescriptor {
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
            status: TaskStatus::new(TaskState::Completed),
            artifacts: vec![Artifact::text_artifact("hecho")],
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();
    let tracker = TaskTracker::new(store.clone(), EventBus::new(8));

    let error = tracker
        .transition(parent, child, TaskStatus::new(TaskState::Working))
        .await
        .unwrap_err();
    assert!(matches!(error, CoreError::Store(_)), "{error:?}");
    assert_eq!(
        store.list_subagents(parent).await.unwrap()[0].status.state,
        TaskState::Completed
    );
}
