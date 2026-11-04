//! Registro del uso de tokens.

use super::support::*;
use super::*;

#[tokio::test]
async fn usage_cache_tokens_survive_a_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("c.db").to_str().unwrap())
        .await
        .unwrap();
    let sid = store.create_session("/p").await.unwrap();
    let mut record = usage("m", 100, 5, None);
    record.session = sid;
    record.cache_read_tokens = 900;
    record.cache_write_tokens = 40;
    store.record_usage(sid, &record).await.unwrap();
    let rows = store.session_usage(sid).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].cache_read_tokens, rows[0].cache_write_tokens),
        (900, 40)
    );
}

#[tokio::test]
async fn usage_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("u.db").to_str().unwrap())
        .await
        .unwrap();
    let sid = store.create_session("/p").await.unwrap();
    let priced = UsageRecord {
        session: sid,
        model: "mistral-nemo".into(),
        input_tokens: 1200,
        output_tokens: 300,
        input_price: Some(0.02),
        output_price: Some(0.04),
        reported_cost_usd: Some(0.0001),
        source: crate::message::UsageSource::Provider,
        is_final: true,
        components: vec![crate::session::UsageComponent {
            kind: crate::session::UsageComponentKind::Mcp,
            id: "filesystem".into(),
            input_tokens: 73,
        }],
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: 1_000,
    };
    let unpriced = UsageRecord {
        session: sid,
        model: "sin-precio".into(),
        input_tokens: 5,
        output_tokens: 7,
        input_price: None,
        output_price: None,
        reported_cost_usd: None,
        source: crate::message::UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: 2_000,
    };
    store.record_usage(sid, &priced).await.unwrap();
    store.record_usage(sid, &unpriced).await.unwrap();
    assert_eq!(
        store.session_usage(sid).await.unwrap(),
        vec![priced, unpriced]
    );
}

#[tokio::test]
async fn usage_record_carries_its_session_after_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("sesion.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/p").await.unwrap();
    let child = store.create_session("/p").await.unwrap();
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
            label: "review".into(),
            status: TaskStatus::new(TaskState::Completed),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();
    store
        .record_usage(parent, &usage("padre", 1, 1, None))
        .await
        .unwrap();
    store
        .record_usage(child, &usage("hijo", 2, 2, None))
        .await
        .unwrap();

    let rows = store.session_usage(parent).await.unwrap();
    let apuntes: Vec<(SessionId, &str)> = rows
        .iter()
        .map(|record| (record.session, record.model.as_str()))
        .collect();
    assert_eq!(
        apuntes,
        vec![(parent, "padre"), (child, "hijo")],
        "cada apunte vuelve con la sesión que lo originó"
    );
}

#[tokio::test]
async fn usage_is_isolated_per_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("iso.db").to_str().unwrap())
        .await
        .unwrap();
    let first = store.create_session("/p").await.unwrap();
    let second = store.create_session("/p").await.unwrap();
    store
        .record_usage(first, &usage("a", 10, 1, Some((1.0, 2.0))))
        .await
        .unwrap();
    store
        .record_usage(second, &usage("b", 20, 2, None))
        .await
        .unwrap();

    let of_first = store.session_usage(first).await.unwrap();
    assert_eq!(of_first.len(), 1);
    assert_eq!(of_first[0].model, "a");
    let of_second = store.session_usage(second).await.unwrap();
    assert_eq!(of_second.len(), 1);
    assert_eq!(of_second[0].model, "b");
}

#[tokio::test]
async fn session_usage_includes_children() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("hijos.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/p").await.unwrap();
    let child = store.create_session("/p").await.unwrap();
    let stranger = store.create_session("/p").await.unwrap();
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
            label: "review".into(),
            status: TaskStatus::new(TaskState::Completed),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();

    store
        .record_usage(parent, &usage("p1", 1, 1, None))
        .await
        .unwrap();
    store
        .record_usage(child, &usage("c1", 2, 2, None))
        .await
        .unwrap();
    store
        .record_usage(parent, &usage("p2", 3, 3, None))
        .await
        .unwrap();
    store
        .record_usage(stranger, &usage("otra", 9, 9, None))
        .await
        .unwrap();

    let models: Vec<String> = store
        .session_usage(parent)
        .await
        .unwrap()
        .into_iter()
        .map(|record| record.model)
        .collect();
    assert_eq!(
        models,
        vec!["p1", "c1", "p2"],
        "por id, sin la sesión ajena"
    );
}

#[tokio::test]
async fn session_usage_includes_the_whole_subtree_not_only_direct_children() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("arbol.db").to_str().unwrap())
        .await
        .unwrap();
    let raiz = store.create_session("/p").await.unwrap();
    let hijo = store.create_session("/p").await.unwrap();
    let nieto = store.create_session("/p").await.unwrap();
    let bisnieto = store.create_session("/p").await.unwrap();
    let ajena = store.create_session("/p").await.unwrap();
    for (padre, hijo_de) in [(raiz, hijo), (hijo, nieto), (nieto, bisnieto)] {
        store
            .save_subagent(&SubagentDescriptor {
                phase: SubagentPhase::Queued,
                last_activity_at_ms: 0,
                steps: 0,
                tool_calls: 0,
                tokens_in: 0,
                tokens_out: 0,
                parent: padre,
                child: hijo_de,
                job_id: format!("subagent-{}", hijo_de.0),
                label: "nivel".into(),
                status: TaskStatus::new(TaskState::Completed),
                artifacts: Vec::new(),
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            })
            .await
            .unwrap();
    }
    for (sesion, modelo) in [
        (raiz, "raiz"),
        (hijo, "hijo"),
        (nieto, "nieto"),
        (bisnieto, "bisnieto"),
        (ajena, "ajena"),
    ] {
        store
            .record_usage(sesion, &usage(modelo, 1, 1, None))
            .await
            .unwrap();
    }
    let modelos = |registros: Vec<UsageRecord>| {
        registros
            .into_iter()
            .map(|registro| registro.model)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        modelos(store.session_usage(raiz).await.unwrap()),
        vec!["raiz", "hijo", "nieto", "bisnieto"]
    );
    // El subárbol de un hijo no incluye a su padre.
    assert_eq!(
        modelos(store.session_usage(hijo).await.unwrap()),
        vec!["hijo", "nieto", "bisnieto"]
    );
}
