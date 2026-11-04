//! Persistencia de la fase de ejecución y del latido de los subagentes.

use super::support::*;
use super::*;
use crate::task::{TaskState, TaskStatus};

#[tokio::test]
async fn runtime_phase_and_heartbeat_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("runtime.db");
    let store = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let parent = store.create_session("/proj").await.unwrap();
    let child = store.create_session("/proj").await.unwrap();
    store
        .save_subagent(&descriptor(parent, child, TaskState::Working))
        .await
        .unwrap();
    store
        .update_subagent_runtime(parent, child, SubagentPhase::Streaming, 1234)
        .await
        .unwrap();
    drop(store);
    let again = SqliteStore::open(path.to_str().unwrap()).await.unwrap();
    let hijos = again.list_subagents(parent).await.unwrap();
    assert_eq!(hijos[0].phase, SubagentPhase::Streaming);
    assert_eq!(hijos[0].last_activity_at_ms, 1234);
}

#[tokio::test]
async fn runtime_update_does_not_touch_a_settled_child() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("runtime2.db").to_str().unwrap())
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
    store
        .update_subagent_runtime(parent, child, SubagentPhase::Stalled, 99)
        .await
        .unwrap();
    let hijos = store.list_subagents(parent).await.unwrap();
    assert_ne!(hijos[0].phase, SubagentPhase::Stalled);
    assert_ne!(hijos[0].last_activity_at_ms, 99);
}
