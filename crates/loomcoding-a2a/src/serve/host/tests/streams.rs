//! Tests: streams.

use super::support::*;
use super::*;

#[tokio::test]
async fn a_stream_starts_with_the_task_and_ends_at_the_terminal_state() {
    let montaje = montar(Arc::new(ConTexto("hola".into())), limites()).await;

    let flujo = montaje.host.start_streaming("saluda".into()).await.unwrap();
    let eventos = recoger(flujo).await;

    assert!(eventos[0].get("task").is_some(), "{eventos:?}");
    let ultimo = eventos.last().unwrap();
    let estado = ultimo["statusUpdate"]["status"]["state"]
        .as_str()
        .or_else(|| ultimo["task"]["status"]["state"].as_str());
    assert_eq!(estado, Some("TASK_STATE_COMPLETED"), "{eventos:?}");
    // El texto del artefacto llega por el flujo, una sola vez.
    let textos = eventos
        .iter()
        .filter_map(|e| e["artifactUpdate"]["artifact"]["parts"][0]["text"].as_str())
        .chain(
            eventos
                .iter()
                .filter_map(|e| e["task"]["artifacts"][0]["parts"][0]["text"].as_str()),
        )
        .collect::<Vec<_>>();
    assert!(textos.contains(&"hola"), "{eventos:?}");
}

#[tokio::test]
async fn subscribing_follows_a_working_task_until_it_is_canceled() {
    let montaje = montar(Arc::new(Colgado), limites()).await;
    let inicial = montaje.host.start("larga".into()).await.unwrap();
    let id = id_de(&inicial);

    let flujo = montaje.host.subscribe(&id).await.unwrap();
    montaje.host.cancel(&id).await.unwrap();
    let eventos = recoger(flujo).await;

    assert_eq!(eventos[0]["task"]["status"]["state"], "TASK_STATE_WORKING");
    assert_eq!(
        eventos.last().unwrap()["statusUpdate"]["status"]["state"],
        "TASK_STATE_CANCELED"
    );
}

#[tokio::test]
async fn subscribing_to_a_terminal_task_is_unsupported() {
    let montaje = montar(Arc::new(ConTexto("ok".into())), limites()).await;
    let inicial = montaje.host.start("a".into()).await.unwrap();
    let id = id_de(&inicial);
    montaje
        .host
        .wait_terminal(&id, Duration::from_secs(10))
        .await
        .unwrap();

    let error = montaje.host.subscribe(&id).await.err().unwrap();

    assert_eq!(error, HostError::Unsupported);
}

#[tokio::test]
async fn another_sessions_task_and_garbage_ids_cannot_be_subscribed() {
    let montaje = montar(Arc::new(Colgado), limites()).await;
    let (ajeno, _) = montaje
        .launcher
        .start(
            montaje.otro_padre,
            "ajena".into(),
            "hola".into(),
            Some("default".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    for id in [
        ajeno.0.to_string(),
        "999999".into(),
        "abc".into(),
        String::new(),
    ] {
        let error = montaje.host.subscribe(&id).await.err().unwrap();
        assert_eq!(error, HostError::NotFound, "{id:?}");
    }
}

#[tokio::test]
async fn the_seventeenth_stream_is_refused_and_a_dropped_one_frees_its_slot() {
    let montaje = montar(Arc::new(Colgado), limites()).await;
    let inicial = montaje.host.start("larga".into()).await.unwrap();
    let id = id_de(&inicial);

    let mut abiertos = Vec::new();
    for _ in 0..MAX_STREAMS {
        abiertos.push(montaje.host.subscribe(&id).await.unwrap());
    }
    assert_eq!(
        montaje.host.subscribe(&id).await.err().unwrap(),
        HostError::Busy
    );
    // Tampoco se crea una tarea nueva que se quedaría sin flujo.
    assert_eq!(
        montaje
            .host
            .start_streaming("otra".into())
            .await
            .err()
            .unwrap(),
        HostError::Busy
    );
    let hijos = montaje.launcher.list(montaje.host.parent()).await.unwrap();
    assert_eq!(hijos.len(), 1);

    abiertos.pop();

    assert!(montaje.host.subscribe(&id).await.is_ok());
}
