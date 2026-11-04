//! Tests: tasks.

use super::support::*;
use super::*;

#[test]
fn the_served_agent_can_only_view_glob_and_grep() {
    let mut nombres: Vec<String> = served_tools().specs().into_iter().map(|s| s.name).collect();
    nombres.sort();

    assert_eq!(nombres, ["glob", "grep", "view"]);
}

#[tokio::test]
async fn a_task_runs_to_completion_and_returns_its_text() {
    let montaje = montar(
        Arc::new(ConTexto("hola desde el servidor".into())),
        limites(),
    )
    .await;

    let inicial = montaje.host.start("saluda".into()).await.unwrap();
    let fin = montaje
        .host
        .wait_terminal(&id_de(&inicial), Duration::from_secs(10))
        .await
        .unwrap();

    assert!(fin.terminal, "{:?}", fin.json);
    assert_eq!(fin.json["status"]["state"], "TASK_STATE_COMPLETED");
    assert_eq!(
        fin.json["artifacts"][0]["parts"][0]["text"],
        "hola desde el servidor"
    );
}

#[tokio::test]
async fn incoming_send_is_observed_without_copying_its_message() {
    let montaje = montar(Arc::new(ConTexto("ok".into())), limites()).await;
    let mut eventos = montaje.bus.subscribe();
    let tarea = montaje
        .host
        .start("mensaje privado de prueba".into())
        .await
        .unwrap();
    let id = id_de(&tarea);

    loop {
        let evento = tokio::time::timeout(Duration::from_secs(2), eventos.recv())
            .await
            .unwrap()
            .unwrap();
        if let LiveEvent::A2aObserved(observacion) = evento {
            assert_eq!(observacion.direction, A2aDirection::Inbound);
            assert_eq!(observacion.operation, A2aOperation::SendMessage);
            assert_eq!(observacion.task_id.as_deref(), Some(id.as_str()));
            assert_eq!(observacion.context_id.as_deref(), Some("1"));
            assert!(observacion.succeeded);
            assert_eq!(observacion.artifact, None);
            break;
        }
    }
}

#[tokio::test]
async fn every_state_change_of_an_incoming_task_is_announced_on_the_bus() {
    let montaje = montar(Arc::new(ConTexto("ok".into())), limites()).await;
    let mut eventos = montaje.bus.subscribe();

    let inicial = montaje.host.start("saluda".into()).await.unwrap();
    let id = id_de(&inicial);

    let mut estados = Vec::new();
    let limite = Instant::now() + Duration::from_secs(10);
    while Instant::now() < limite {
        let Ok(Ok(evento)) = tokio::time::timeout(Duration::from_millis(500), eventos.recv()).await
        else {
            continue;
        };
        if let LiveEvent::A2aServed { task, agent, state } = evento {
            assert_eq!(task, id);
            assert_eq!(agent, "default");
            let terminal = state == "TASK_STATE_COMPLETED";
            estados.push(state);
            if terminal {
                break;
            }
        }
    }

    assert_eq!(
        estados.last().map(String::as_str),
        Some("TASK_STATE_COMPLETED")
    );
    let mut sin_repetir = estados.clone();
    sin_repetir.dedup();
    assert_eq!(
        estados, sin_repetir,
        "no debe avisar dos veces del mismo estado seguido"
    );
}

#[tokio::test]
async fn the_task_cap_refuses_the_extra_one_and_creates_nothing() {
    let montaje = montar(Arc::new(Colgado), limites()).await;
    montaje.host.start("uno".into()).await.unwrap();
    montaje.host.start("dos".into()).await.unwrap();

    let error = montaje.host.start("tres".into()).await.unwrap_err();

    assert_eq!(error, HostError::Busy);
    let hijos = montaje.launcher.list(montaje.host.parent()).await.unwrap();
    assert_eq!(hijos.len(), 2, "la tercera no debe crear tarea");
}

#[tokio::test]
async fn a_finished_task_frees_its_slot() {
    let montaje = montar(
        Arc::new(ConTexto("ok".into())),
        HostLimits {
            max_concurrent: 1,
            ..limites()
        },
    )
    .await;
    let primera = montaje.host.start("a".into()).await.unwrap();
    montaje
        .host
        .wait_terminal(&id_de(&primera), Duration::from_secs(10))
        .await
        .unwrap();

    assert!(montaje.host.start("b".into()).await.is_ok());
}

#[tokio::test]
async fn a_task_that_overruns_its_time_is_canceled() {
    let montaje = montar(
        Arc::new(Colgado),
        HostLimits {
            max_task: Duration::from_millis(300),
            ..limites()
        },
    )
    .await;

    let inicial = montaje.host.start("eterna".into()).await.unwrap();
    let fin = montaje
        .host
        .wait_terminal(&id_de(&inicial), Duration::from_secs(10))
        .await
        .unwrap();

    assert!(fin.terminal, "{:?}", fin.json);
    assert_eq!(fin.json["status"]["state"], "TASK_STATE_CANCELED");
}

#[tokio::test]
async fn cancel_returns_the_task_canceled_and_a_second_cancel_is_not_cancelable() {
    let montaje = montar(Arc::new(Colgado), limites()).await;
    let inicial = montaje.host.start("larga".into()).await.unwrap();
    let id = id_de(&inicial);

    let cancelada = montaje.host.cancel(&id).await.unwrap();
    assert_eq!(cancelada.json["status"]["state"], "TASK_STATE_CANCELED");

    assert_eq!(
        montaje.host.cancel(&id).await.unwrap_err(),
        HostError::NotCancelable
    );
}

#[tokio::test]
async fn another_sessions_task_and_garbage_ids_are_just_not_found() {
    let montaje = montar(Arc::new(ConTexto("ok".into())), limites()).await;
    // Una tarea de otra sesión padre, en el mismo almacén.
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
        "-1".into(),
        String::new(),
    ] {
        assert_eq!(
            montaje.host.get(&id).await.unwrap_err(),
            HostError::NotFound,
            "{id:?}"
        );
        assert_eq!(
            montaje.host.cancel(&id).await.unwrap_err(),
            HostError::NotFound,
            "{id:?}"
        );
    }
}

#[tokio::test]
async fn waiting_returns_the_current_state_when_time_runs_out() {
    let montaje = montar(Arc::new(Colgado), limites()).await;
    let inicial = montaje.host.start("larga".into()).await.unwrap();

    let vista = montaje
        .host
        .wait_terminal(&id_de(&inicial), Duration::from_millis(200))
        .await
        .unwrap();

    assert!(!vista.terminal);
    assert_eq!(vista.json["status"]["state"], "TASK_STATE_WORKING");
}
