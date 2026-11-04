//! Catálogo de subagentes: estados, artefactos, progreso, agente y referencias.

use super::support::*;
use super::*;

#[tokio::test]
async fn persists_subagent_catalog_and_completes_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("subagents.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    let inicial = descriptor(parent, child, TaskState::Working);
    store.save_subagent(&inicial).await.unwrap();
    assert_eq!(store.list_subagents(parent).await.unwrap(), vec![inicial]);
    store
        .complete_subagent(
            parent,
            child,
            &[crate::a2a::Artifact::text_artifact("finished")],
        )
        .await
        .unwrap();
    assert_eq!(
        store.list_subagents(parent).await.unwrap()[0].status.state,
        TaskState::Completed
    );
    let notice = Message::user("[Subagent review completed]\nfinished");
    assert!(store
        .append_subagent_notice(parent, child, &notice)
        .await
        .unwrap());
    assert!(!store
        .append_subagent_notice(parent, child, &notice)
        .await
        .unwrap());
    assert_eq!(store.messages(parent).await.unwrap().len(), 1);
}

#[tokio::test]
async fn reopened_store_marks_non_terminal_children_failed_with_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reopen.db");
    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let mut vivos = Vec::new();
    for state in [
        TaskState::Submitted,
        TaskState::Working,
        TaskState::InputRequired,
        TaskState::AuthRequired,
    ] {
        let child = store.create_session("/proj").await.unwrap();
        store
            .save_subagent(&descriptor(parent, child, state))
            .await
            .unwrap();
        vivos.push(child);
    }
    let terminado = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, terminado, TaskState::Completed))
        .await
        .unwrap();
    // Una fila escrita con la cadena antigua «running» también cuenta como no terminal.
    let antigua = store.create_session("/proj").await.unwrap();
    sqlx::query(
        "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, created_at_ms)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(parent.0)
    .bind(antigua.0)
    .bind("subagent-antigua")
    .bind("review")
    .bind("running")
    .bind(crate::message::now_ms())
    .execute(&store.pool)
    .await
    .unwrap();
    vivos.push(antigua);
    drop(store);

    let reopened = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let listed = reopened.list_subagents(parent).await.unwrap();
    for child in vivos {
        let fila = listed.iter().find(|d| d.child == child).unwrap();
        assert_eq!(fila.status.state, TaskState::Failed, "hijo {child:?}");
        assert_eq!(
            fila.status.message_text().as_deref(),
            Some("interrupted by application shutdown")
        );
    }
    let fila = listed.iter().find(|d| d.child == terminado).unwrap();
    assert_eq!(fila.status.state, TaskState::Completed);
    assert_eq!(fila.status.message, None);
}

#[tokio::test]
async fn state_roundtrips_through_sqlite_with_message_and_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("roundtrip.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    for state in TaskState::ALL {
        let child = store.create_session("/proj").await.unwrap();
        let guardado = SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            status: TaskStatus {
                state,
                message: Some(crate::a2a::Message::agent_text(format!(
                    "mensaje de {state:?}"
                ))),
                timestamp_ms: 1_700_000_000_123,
            },
            ..descriptor(parent, child, state)
        };
        store.save_subagent(&guardado).await.unwrap();
        // Lo que se escribe es el nombre del proto v1.0, no una cadena propia.
        assert_eq!(raw_status(&store, child).await, state.proto_name());
        let leido = store
            .list_subagents(parent)
            .await
            .unwrap()
            .into_iter()
            .find(|d| d.child == child)
            .unwrap();
        assert_eq!(leido, guardado);
    }
}

#[tokio::test]
async fn update_from_terminal_state_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("terminal.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, child, TaskState::Working))
        .await
        .unwrap();
    store
        .update_subagent_status(parent, child, TaskStatus::new(TaskState::Completed))
        .await
        .unwrap();
    for intento in [TaskState::Working, TaskState::Failed, TaskState::Canceled] {
        assert!(
            store
                .update_subagent_status(parent, child, TaskStatus::new(intento))
                .await
                .is_err(),
            "de Completed a {intento:?} debería fallar"
        );
    }
    assert!(store
        .complete_subagent(
            parent,
            child,
            &[crate::a2a::Artifact::text_artifact("otra vez")]
        )
        .await
        .is_err());
    assert_eq!(
        store.list_subagents(parent).await.unwrap()[0].status.state,
        TaskState::Completed
    );
}

#[tokio::test]
async fn complete_subagent_writes_the_proto_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("complete.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, child, TaskState::Working))
        .await
        .unwrap();
    store
        .complete_subagent(
            parent,
            child,
            &[crate::a2a::Artifact::text_artifact("finished")],
        )
        .await
        .unwrap();
    assert_eq!(raw_status(&store, child).await, "TASK_STATE_COMPLETED");
}

#[tokio::test]
async fn status_message_roundtrips_through_sqlite_as_a_message() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("mensaje.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    let mensaje = crate::a2a::Message::agent_text("¿Cuál?");
    let guardado = SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        status: TaskStatus {
            state: TaskState::InputRequired,
            message: Some(mensaje.clone()),
            timestamp_ms: 1_700_000_000_123,
        },
        ..descriptor(parent, child, TaskState::InputRequired)
    };
    store.save_subagent(&guardado).await.unwrap();
    let leido = store.list_subagents(parent).await.unwrap();
    assert_eq!(leido[0].status.message, Some(mensaje));
    assert_eq!(leido, vec![guardado]);
}

#[tokio::test]
async fn legacy_plain_text_status_message_reads_as_agent_message() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("legacy-mensaje.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    fila_antigua(
        &store,
        parent,
        child,
        "TASK_STATE_INPUT_REQUIRED",
        Some("¿Cuál?"),
        None,
    )
    .await;
    let leido = store.list_subagents(parent).await.unwrap();
    let mensaje = leido[0]
        .status
        .message
        .clone()
        .expect("el texto plano se lee como mensaje");
    assert_eq!(mensaje.role, crate::a2a::Role::Agent);
    assert_eq!(mensaje.text(), "¿Cuál?");
    // Dos lecturas dan el mismo mensaje: su identificador se deriva de la fila.
    assert_eq!(store.list_subagents(parent).await.unwrap(), leido);
}

#[tokio::test]
async fn artifacts_roundtrip_through_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("artefactos.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    let artefactos = vec![
        crate::a2a::Artifact::text_artifact("resumen"),
        crate::a2a::Artifact {
            artifact_id: "datos".into(),
            name: Some("datos".into()),
            description: Some("binario y json".into()),
            parts: vec![
                crate::a2a::Part {
                    content: crate::a2a::PartContent::Raw(vec![0, 255]),
                    metadata: None,
                    filename: Some("d.bin".into()),
                    media_type: Some("application/octet-stream".into()),
                },
                crate::a2a::Part {
                    content: crate::a2a::PartContent::Data(serde_json::json!({"ok": true})),
                    metadata: None,
                    filename: None,
                    media_type: None,
                },
            ],
            metadata: None,
            extensions: vec![],
        },
    ];
    let guardado = SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        artifacts: artefactos,
        ..descriptor(parent, child, TaskState::Completed)
    };
    store.save_subagent(&guardado).await.unwrap();
    assert_eq!(store.list_subagents(parent).await.unwrap(), vec![guardado]);
}

#[tokio::test]
async fn legacy_result_column_reads_as_one_text_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("legacy-result.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    fila_antigua(
        &store,
        parent,
        child,
        "TASK_STATE_COMPLETED",
        None,
        Some("hecho"),
    )
    .await;
    let leido = store.list_subagents(parent).await.unwrap();
    assert_eq!(leido[0].artifacts.len(), 1);
    assert_eq!(leido[0].artifacts[0].text(), "hecho");
    assert_eq!(leido[0].result_text().as_deref(), Some("hecho"));
}

#[tokio::test]
async fn complete_subagent_stores_a_text_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("completo.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, child, TaskState::Working))
        .await
        .unwrap();
    store
        .complete_subagent(
            parent,
            child,
            &[crate::a2a::Artifact::text_artifact("finished")],
        )
        .await
        .unwrap();
    let leido = store.list_subagents(parent).await.unwrap();
    assert_eq!(leido[0].artifacts.len(), 1);
    assert_eq!(leido[0].artifacts[0].text(), "finished");
    assert_eq!(leido[0].result_text().as_deref(), Some("finished"));
    // `complete_subagent` ya no escribe la columna antigua `result`.
    let result: Option<String> =
        sqlx::query_scalar("SELECT result FROM subagents WHERE child_session_id = ?")
            .bind(child.0)
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn progress_is_published_in_flight_and_survives_completion() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("progreso.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, child, TaskState::Working))
        .await
        .unwrap();
    let d = store
        .publish_subagent_progress(child, "plan", Some("pasos"), "1. leer")
        .await
        .unwrap();
    assert_eq!(d.status.state, TaskState::Working);
    assert_eq!(d.progress_artifacts().count(), 1);
    assert_eq!(d.result_text(), None);
    let id = d.artifacts[0].artifact_id.clone();
    let d = store
        .publish_subagent_progress(child, "plan", None, "1. leer\n2. editar")
        .await
        .unwrap();
    assert_eq!(d.artifacts.len(), 1);
    assert_eq!(d.artifacts[0].artifact_id, id);

    store
        .complete_subagent(
            parent,
            child,
            &[crate::a2a::Artifact::text_artifact("hecho")],
        )
        .await
        .unwrap();
    let leido = store.list_subagents(parent).await.unwrap();
    assert_eq!(leido[0].artifacts.len(), 2);
    assert_eq!(leido[0].progress_artifacts().count(), 1);
    assert_eq!(leido[0].result_text().as_deref(), Some("hecho"));
}

#[tokio::test]
async fn progress_is_rejected_for_settled_or_unknown_tasks_and_over_limits() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("rechazo.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    let ajena = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, child, TaskState::Working))
        .await
        .unwrap();
    let error = store
        .publish_subagent_progress(ajena, "x", None, "y")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not a subagent"), "{error}");
    let error = store
        .publish_subagent_progress(child, "x", None, &"y".repeat(9_000))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("content"), "{error}");
    assert_eq!(
        store.list_subagents(parent).await.unwrap()[0]
            .artifacts
            .len(),
        0
    );
    store
        .complete_subagent(parent, child, &[crate::a2a::Artifact::text_artifact("ok")])
        .await
        .unwrap();
    let error = store
        .publish_subagent_progress(child, "x", None, "y")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("already finished"), "{error}");
}

#[tokio::test]
async fn agent_name_and_origin_roundtrip_through_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("agente.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    let con_agente = SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        agent: Some("reviewer".into()),
        agent_origin: Some("project:/proj/.loomcoding/agents/reviewer.md".into()),
        reference_task_ids: Vec::new(),
        ..descriptor(parent, child, TaskState::Working)
    };
    store.save_subagent(&con_agente).await.unwrap();
    assert_eq!(
        store.list_subagents(parent).await.unwrap(),
        vec![con_agente]
    );
}

#[tokio::test]
async fn references_roundtrip_through_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("referencias.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let anterior = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    let sin = SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        reference_task_ids: Vec::new(),
        ..descriptor(parent, anterior, TaskState::Completed)
    };
    let con = SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        reference_task_ids: vec![anterior, SessionId(99)],
        ..descriptor(parent, child, TaskState::Working)
    };
    store.save_subagent(&sin).await.unwrap();
    store.save_subagent(&con).await.unwrap();

    assert_eq!(store.list_subagents(parent).await.unwrap(), vec![sin, con]);
}

#[test]
fn unreadable_references_are_dropped_not_fatal() {
    assert!(decode_references(Some("no es json".into()), 7).is_empty());
    assert!(decode_references(None, 7).is_empty());
    assert_eq!(
        decode_references(Some("[3,5]".into()), 7),
        vec![SessionId(3), SessionId(5)]
    );
}
