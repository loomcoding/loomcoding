//! El buzón: espera, respuesta, descarte y ámbito por padre.

use super::support::*;
use super::*;

#[tokio::test]
async fn request_waits_until_answered() {
    let broker = Arc::new(InputBroker::default());
    let questions = vec![question("¿Cuál?")];
    let b = broker.clone();
    let q = questions.clone();
    let handle = tokio::spawn(async move { b.request(PARENT, CHILD, "obrero".into(), q).await });

    wait_for_pending(&broker, PARENT).await;
    assert_eq!(broker.pending(PARENT).len(), 1);

    broker.answer(PARENT, CHILD, vec!["a".into()]).unwrap();
    let result = handle.await.unwrap();
    assert_eq!(
        result,
        Some(vec![Answer {
            question: "¿Cuál?".into(),
            selected: vec!["a".into()],
        }])
    );
    assert!(broker.pending(PARENT).is_empty());
}

#[tokio::test]
async fn answer_without_pending_is_an_error() {
    let broker = InputBroker::default();
    let err = broker.answer(PARENT, CHILD, vec!["a".into()]).unwrap_err();
    assert!(
        matches!(err, CoreError::Tool(_)),
        "error inesperado: {err:?}"
    );
}

#[tokio::test]
async fn answer_with_wrong_count_is_an_error_and_keeps_waiting() {
    let broker = Arc::new(InputBroker::default());
    let questions = vec![question("Primera"), question("Segunda")];
    let b = broker.clone();
    let q = questions.clone();
    let handle = tokio::spawn(async move { b.request(PARENT, CHILD, "obrero".into(), q).await });

    wait_for_pending(&broker, PARENT).await;
    let err = broker
        .answer(PARENT, CHILD, vec!["solo una".into()])
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Tool(_)),
        "error inesperado: {err:?}"
    );
    assert_eq!(broker.pending(PARENT).len(), 1, "la petición debe seguir");

    broker
        .answer(PARENT, CHILD, vec!["x".into(), "y".into()])
        .unwrap();
    let result = handle.await.unwrap();
    assert_eq!(
        result,
        Some(vec![
            Answer {
                question: "Primera".into(),
                selected: vec!["x".into()],
            },
            Answer {
                question: "Segunda".into(),
                selected: vec!["y".into()],
            },
        ])
    );
}

#[tokio::test]
async fn discard_unblocks_with_none_and_clears_entry() {
    let broker = Arc::new(InputBroker::default());
    let b = broker.clone();
    let handle = tokio::spawn(async move {
        b.request(PARENT, CHILD, "obrero".into(), vec![question("Q")])
            .await
    });

    wait_for_pending(&broker, PARENT).await;
    broker.discard(CHILD);
    assert_eq!(handle.await.unwrap(), None);
    assert!(broker.pending(PARENT).is_empty());
}

#[tokio::test]
async fn dropping_the_request_future_clears_entry() {
    let broker = Arc::new(InputBroker::default());
    let b = broker.clone();
    let handle = tokio::spawn(async move {
        b.request(PARENT, CHILD, "obrero".into(), vec![question("Q")])
            .await
    });

    wait_for_pending(&broker, PARENT).await;
    handle.abort();
    // Al esperar al JoinError, la tarea ya se ha soltado y su guard ha corrido.
    assert!(handle.await.unwrap_err().is_cancelled());
    assert!(broker.pending(PARENT).is_empty());
}

#[tokio::test]
async fn request_with_calls_the_callback_after_registration() {
    let broker = Arc::new(InputBroker::default());
    let observed = Arc::new(Mutex::new(None));
    let b = broker.clone();
    let o = observed.clone();
    let on_registered = move || {
        *o.lock().unwrap() = Some(b.pending(PARENT).len());
    };
    let b = broker.clone();
    let handle = tokio::spawn(async move {
        b.request_with(
            PARENT,
            CHILD,
            "obrero".into(),
            vec![question("Q")],
            on_registered,
        )
        .await
    });

    wait_for_pending(&broker, PARENT).await;
    broker.answer(PARENT, CHILD, vec!["a".into()]).unwrap();
    assert!(handle.await.unwrap().is_some());
    // El callback vio la petición ya registrada (y sin cerrojo: pending no bloquea).
    assert_eq!(*observed.lock().unwrap(), Some(1));
}

#[tokio::test]
async fn take_unannounced_returns_each_request_once() {
    let broker = Arc::new(InputBroker::default());
    let b = broker.clone();
    let handle = tokio::spawn(async move {
        b.request(PARENT, CHILD, "obrero".into(), vec![question("Q")])
            .await
    });

    wait_for_pending(&broker, PARENT).await;
    let first = broker.take_unannounced(PARENT);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].label, "obrero");
    assert!(broker.take_unannounced(PARENT).is_empty());
    // Anunciar no equivale a responder: sigue pendiente.
    assert_eq!(broker.pending(PARENT).len(), 1);

    broker.discard(CHILD);
    handle.await.unwrap();
}

#[tokio::test]
async fn pending_is_scoped_to_the_parent() {
    let broker = Arc::new(InputBroker::default());
    let other_parent = SessionId(9);
    let other_child = SessionId(3);

    let b1 = broker.clone();
    let h1 = tokio::spawn(async move {
        b1.request(PARENT, CHILD, "uno".into(), vec![question("Q1")])
            .await
    });
    let b2 = broker.clone();
    let h2 = tokio::spawn(async move {
        b2.request(
            other_parent,
            other_child,
            "dos".into(),
            vec![question("Q2")],
        )
        .await
    });

    wait_for_pending(&broker, PARENT).await;
    wait_for_pending(&broker, other_parent).await;

    let mine = broker.pending(PARENT);
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].child, CHILD);
    let theirs = broker.pending(other_parent);
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0].child, other_child);

    broker.discard(CHILD);
    broker.discard(other_child);
    h1.await.unwrap();
    h2.await.unwrap();
}

/// Extra (más allá de las 7 del plan): una petición nueva del mismo hijo sustituye
/// a la pendiente; la anterior recibe `None` y su guard no borra la nueva.
#[tokio::test]
async fn new_request_from_same_child_replaces_the_previous_one() {
    let broker = Arc::new(InputBroker::default());

    let b1 = broker.clone();
    let first = tokio::spawn(async move {
        b1.request(PARENT, CHILD, "primera".into(), vec![question("Vieja")])
            .await
    });
    wait_for_pending(&broker, PARENT).await;

    let b2 = broker.clone();
    let second = tokio::spawn(async move {
        b2.request(PARENT, CHILD, "segunda".into(), vec![question("Nueva")])
            .await
    });

    assert_eq!(first.await.unwrap(), None);
    wait_for_pending(&broker, PARENT).await;
    let pending = broker.pending(PARENT);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].label, "segunda");

    broker.answer(PARENT, CHILD, vec!["ok".into()]).unwrap();
    assert_eq!(
        second.await.unwrap(),
        Some(vec![Answer {
            question: "Nueva".into(),
            selected: vec!["ok".into()],
        }])
    );
}
