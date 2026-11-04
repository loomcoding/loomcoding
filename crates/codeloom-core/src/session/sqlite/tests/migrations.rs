//! Migraciones de bases antiguas.

use super::support::*;
use super::*;

#[tokio::test]
async fn legacy_strings_are_read_as_equivalent_states() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("legacy.db").to_str().unwrap())
        .await
        .unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let pares = [
        ("running", TaskState::Working),
        ("completed", TaskState::Completed),
        ("failed", TaskState::Failed),
        ("killed", TaskState::Canceled),
        ("interrupted", TaskState::Failed),
        ("unfinished", TaskState::Failed),
    ];
    let mut hijos = Vec::new();
    for (cadena, esperado) in pares {
        let child = store.create_session("/proj").await.unwrap();
        // Fila escrita por una versión anterior, con la cadena antigua tal cual.
        sqlx::query(
            "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, created_at_ms)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(parent.0)
        .bind(child.0)
        .bind(format!("subagent-{cadena}"))
        .bind("review")
        .bind(cadena)
        .bind(crate::message::now_ms())
        .execute(&store.pool)
        .await
        .unwrap();
        hijos.push((child, esperado));
    }
    let listed = store.list_subagents(parent).await.unwrap();
    assert_eq!(listed.len(), hijos.len());
    for (child, esperado) in hijos {
        let fila = listed.iter().find(|d| d.child == child).unwrap();
        assert_eq!(fila.status.state, esperado, "hijo {child:?}");
    }
}

#[tokio::test]
async fn store_migration_adds_status_columns_to_an_old_database_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("viejo-a2a.db");
    {
        let pool =
            sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.to_str().unwrap()))
                .await
                .unwrap();
        // Esquema anterior a A2A: sin `status_message` ni `status_timestamp_ms`.
        sqlx::query(
            "CREATE TABLE subagents (
                parent_session_id INTEGER NOT NULL,
                child_session_id INTEGER NOT NULL PRIMARY KEY,
                job_id TEXT NOT NULL,
                label TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                notice_delivered INTEGER NOT NULL DEFAULT 0,
                created_at_ms INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, created_at_ms)
             VALUES (1, 2, 'subagent-old', 'review', 'completed', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let primera = store.list_subagents(SessionId(1)).await.unwrap();
    assert_eq!(primera.len(), 1);
    assert_eq!(primera[0].status.state, TaskState::Completed);
    assert_eq!(primera[0].status.message, None);
    drop(store);

    // Segunda apertura: las columnas ya existen y no vuelve a fallar ni a cambiar nada.
    let again = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    assert_eq!(again.list_subagents(SessionId(1)).await.unwrap(), primera);
    let columnas: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('subagents')")
            .fetch_all(&again.pool)
            .await
            .unwrap();
    assert!(columnas.iter().any(|c| c == "status_message"));
    assert!(columnas.iter().any(|c| c == "status_timestamp_ms"));
}

#[tokio::test]
async fn store_migrates_an_existing_database_without_usage_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("viejo.db");
    {
        // Base anterior al libro de consumo: solo la tabla de sesiones, con una fila.
        let pool =
            sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.to_str().unwrap()))
                .await
                .unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO sessions (project, created_at_ms) VALUES ('/p', 1)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }

    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let old = SessionId(1);
    assert!(store.session_usage(old).await.unwrap().is_empty());
    store
        .record_usage(old, &usage("nuevo", 4, 4, Some((1.0, 1.0))))
        .await
        .unwrap();
    assert_eq!(store.session_usage(old).await.unwrap().len(), 1);
}

#[tokio::test]
async fn usage_migration_preserves_old_rows_and_marks_their_legacy_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage-legacy.db");
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.to_str().unwrap()))
        .await
        .unwrap();
    sqlx::query("CREATE TABLE sessions (id INTEGER PRIMARY KEY, project TEXT NOT NULL, created_at_ms INTEGER NOT NULL)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO sessions VALUES (1, '/p', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE usage (id INTEGER PRIMARY KEY, session_id INTEGER NOT NULL, model TEXT NOT NULL, input_tokens INTEGER NOT NULL, output_tokens INTEGER NOT NULL, input_price REAL, output_price REAL, created_at_ms INTEGER NOT NULL)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO usage VALUES (1, 1, 'old-model', 12, 4, NULL, NULL, 2)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let records = store.session_usage(SessionId(1)).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].source, crate::message::UsageSource::Provider);
    assert!(records[0].is_final);
    assert_eq!((records[0].input_tokens, records[0].output_tokens), (12, 4));
    // Una base anterior a la caché se lee con ceros: «no informado», no una medición.
    assert_eq!(
        (records[0].cache_read_tokens, records[0].cache_write_tokens),
        (0, 0)
    );
}

#[tokio::test]
async fn column_migration_for_artifacts_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("columna-artefactos.db");
    {
        // Base anterior a los artefactos: `result` existe y no hay `artifacts_json`.
        let pool =
            sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.to_str().unwrap()))
                .await
                .unwrap();
        sqlx::query(
            "CREATE TABLE subagents (
                parent_session_id INTEGER NOT NULL,
                child_session_id INTEGER NOT NULL PRIMARY KEY,
                job_id TEXT NOT NULL,
                label TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                notice_delivered INTEGER NOT NULL DEFAULT 0,
                created_at_ms INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, result, created_at_ms)
             VALUES (1, 2, 'subagent-old', 'review', 'completed', 'hecho', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    let primera_apertura = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let primera = primera_apertura.list_subagents(SessionId(1)).await.unwrap();
    assert_eq!(primera[0].result_text().as_deref(), Some("hecho"));
    drop(primera_apertura);

    let again = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    assert_eq!(again.list_subagents(SessionId(1)).await.unwrap(), primera);
    let columnas: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('subagents')")
            .fetch_all(&again.pool)
            .await
            .unwrap();
    assert!(columnas.iter().any(|c| c == "artifacts_json"));
}

/// Base anterior a las columnas de agente: tiene las de A2A y los artefactos, pero no
/// `agent` ni `agent_origin`. Contiene una fila terminada.
async fn base_sin_columnas_de_agente(path: &std::path::Path) {
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.to_str().unwrap()))
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE subagents (
            parent_session_id INTEGER NOT NULL,
            child_session_id INTEGER NOT NULL PRIMARY KEY,
            job_id TEXT NOT NULL,
            label TEXT NOT NULL,
            status TEXT NOT NULL,
            status_message TEXT,
            status_timestamp_ms INTEGER,
            result TEXT,
            artifacts_json TEXT,
            notice_delivered INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO subagents (parent_session_id, child_session_id, job_id, label, status, status_timestamp_ms, artifacts_json, created_at_ms)
         VALUES (1, 2, 'subagent-old', 'review', 'TASK_STATE_COMPLETED', 1, '[]', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
}

#[tokio::test]
async fn old_rows_without_agent_columns_read_as_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("viejo-agente.db");
    base_sin_columnas_de_agente(&path).await;

    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let leido = store.list_subagents(SessionId(1)).await.unwrap();
    assert_eq!(leido.len(), 1);
    assert_eq!(leido[0].agent, None);
    assert_eq!(leido[0].agent_origin, None);
    assert!(leido[0].reference_task_ids.is_empty());
}

#[tokio::test]
async fn agent_columns_migration_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("columnas-agente.db");
    base_sin_columnas_de_agente(&path).await;

    let primera_apertura = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let primera = primera_apertura.list_subagents(SessionId(1)).await.unwrap();
    drop(primera_apertura);

    // Segunda apertura: las columnas ya existen, no vuelven a añadirse ni fallan.
    let again = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    assert_eq!(again.list_subagents(SessionId(1)).await.unwrap(), primera);
    let columnas: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('subagents')")
            .fetch_all(&again.pool)
            .await
            .unwrap();
    assert_eq!(columnas.iter().filter(|c| *c == "agent").count(), 1);
    assert_eq!(columnas.iter().filter(|c| *c == "agent_origin").count(), 1);
    assert_eq!(
        columnas
            .iter()
            .filter(|c| *c == "reference_task_ids")
            .count(),
        1
    );
}
