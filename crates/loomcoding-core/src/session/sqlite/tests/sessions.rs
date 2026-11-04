//! Sesiones y mensajes.

use super::*;

#[tokio::test]
async fn append_and_read_back_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("loomcoding.db");
    let store = SqliteStore::open(db.to_str().unwrap()).await.unwrap();
    let sid = store.create_session("/proj").await.unwrap();
    store
        .append_message(sid, &Message::user("one"))
        .await
        .unwrap();
    store
        .append_message(
            sid,
            &Message::assistant(vec![ContentPart::Text { text: "two".into() }]),
        )
        .await
        .unwrap();
    let msgs = store.messages(sid).await.unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].text(), "one");
    assert_eq!(msgs[1].text(), "two");
}

#[tokio::test]
async fn lists_only_nonempty_sessions_for_project_in_recent_activity_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("list.db").to_str().unwrap())
        .await
        .unwrap();
    let old = store.create_session("/proj").await.unwrap();
    let other_project = store.create_session("/other").await.unwrap();
    let recent = store.create_session("/proj").await.unwrap();
    store
        .append_message(old, &Message::user("older task"))
        .await
        .unwrap();
    store
        .append_message(other_project, &Message::user("other task"))
        .await
        .unwrap();
    store
        .append_message(recent, &Message::user("recent task"))
        .await
        .unwrap();

    let sessions = store.list_sessions("/proj").await.unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, recent);
    assert_eq!(sessions[0].preview, "recent task");
    assert_eq!(sessions[0].message_count, 1);
    assert_eq!(sessions[1].id, old);
}

#[tokio::test]
async fn empty_sessions_have_project_ownership_without_appearing_in_listing() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("ownership.db").to_str().unwrap())
        .await
        .unwrap();
    let empty = store.create_session("/proj").await.unwrap();

    assert!(store
        .session_belongs_to_project(empty, "/proj")
        .await
        .unwrap());
    assert!(!store
        .session_belongs_to_project(empty, "/other")
        .await
        .unwrap());
    assert!(store.list_sessions("/proj").await.unwrap().is_empty());
}
