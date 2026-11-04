//! Piezas comunes de los tests del buzón.

use super::*;

pub(super) const PARENT: SessionId = SessionId(1);

pub(super) const CHILD: SessionId = SessionId(2);

pub(super) fn question(text: &str) -> Question {
    Question {
        header: "Cabecera".into(),
        question: text.into(),
        options: vec![
            QuestionOption {
                label: "a".into(),
                description: String::new(),
            },
            QuestionOption {
                label: "b".into(),
                description: String::new(),
            },
        ],
        multi_select: false,
    }
}

/// Espera, con cota de tiempo, a que el padre vea al menos una petición pendiente.
pub(super) async fn wait_for_pending(broker: &InputBroker, parent: SessionId) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while broker.pending(parent).is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("la petición debería aparecer en pending");
}

/// Tracker sobre un almacén SQLite temporal y vacío, para las pruebas que no miran el
/// estado del hijo. El directorio se devuelve para que siga vivo durante la prueba.
pub(super) async fn tracker_vacio(bus: &EventBus) -> (tempfile::TempDir, Arc<TaskTracker>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        crate::session::sqlite::SqliteStore::open(dir.path().join("vacio.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    (dir, Arc::new(TaskTracker::new(store, bus.clone())))
}

/// Espera, con cota de tiempo, al siguiente evento del bus que no sea un cambio de estado de
/// la tarea (esos llegan por `SubagentChanged` y tienen sus propias pruebas).
pub(super) async fn next_event(rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>) -> LiveEvent {
    loop {
        let evento = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("debería llegar un evento")
            .expect("el bus debería seguir abierto");
        if !matches!(evento, LiveEvent::SubagentChanged(_)) {
            return evento;
        }
    }
}
