use super::*;

#[tokio::test]
async fn a_stream_cut_before_the_terminal_state_falls_back_to_polling() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![Ok(tarea("r1", TaskState::Completed, None, &["por sondeo"]))],
    );
    con_flujo(
        &cliente,
        GuionFlujo::Elementos(vec![Ok(tarea("r1", TaskState::Working, None, &[]))]),
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(
        entorno.estado_del_aviso(&mut rx).await,
        TaskState::Completed
    );
    assert_eq!(
        entorno.descriptor(hijo).await.artifacts[0].text(),
        "por sondeo"
    );
    assert!(*entorno.cliente.sondeos.lock().unwrap() >= 1);
}

#[tokio::test]
async fn a_stream_that_fails_to_open_or_midway_falls_back_to_polling() {
    for guion in [
        GuionFlujo::Falla(RemoteTaskError::Other("no abre".into())),
        GuionFlujo::Falla(RemoteTaskError::AuthFailed("HTTP 401".into())),
        GuionFlujo::Elementos(vec![
            Ok(tarea("r1", TaskState::Working, None, &[])),
            Err(RemoteTaskError::Other("se cortó".into())),
        ]),
    ] {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![Ok(tarea("r1", TaskState::Completed, None, &["a salvo"]))],
        );
        con_flujo(&cliente, guion);
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        assert_eq!(
            entorno.descriptor(hijo).await.artifacts[0].text(),
            "a salvo"
        );
    }
}

#[tokio::test]
async fn a_stream_about_another_task_is_dropped_and_polling_takes_over() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![Ok(tarea("r1", TaskState::Failed, Some("de verdad"), &[]))],
    );
    // Un flujo que dice que «otra» tarea se completó no puede dar por buena la nuestra.
    con_flujo(
        &cliente,
        GuionFlujo::Elementos(vec![Ok(tarea(
            "otra",
            TaskState::Completed,
            None,
            &["falso"],
        ))]),
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    assert!(entorno.descriptor(hijo).await.artifacts.is_empty());
}

#[tokio::test]
async fn cancelling_the_parent_while_following_the_stream_cancels_the_remote_task() {
    let cliente = ClienteGuionizado::trabajando();
    con_flujo(&cliente, GuionFlujo::Pendiente);
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let cancel_padre = CancellationToken::new();
    let (hijo, _) = entorno
        .lanzador
        .start(
            entorno.parent,
            "lejano".into(),
            "haz lo tuyo".into(),
            Some("remoto".into()),
            cancel_padre.clone(),
        )
        .await
        .unwrap();
    espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Working).await;

    cancel_padre.cancel();

    espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Canceled).await;
    assert_eq!(
        *entorno.cliente.canceladas.lock().unwrap(),
        vec!["r1".to_string()]
    );
    assert_eq!(*entorno.cliente.sondeos.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_silent_stream_is_given_up_at_the_follow_limit_and_polling_gives_up_too() {
    let cliente = ClienteGuionizado::trabajando();
    con_flujo(&cliente, GuionFlujo::Pendiente);
    let entorno = entorno_remoto(cliente, true, false, Duration::from_millis(200)).await;
    let mut rx = entorno.bus.subscribe();

    entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
}

#[tokio::test]
async fn an_unapproved_remote_creates_no_task() {
    let entorno = entorno_remoto(
        ClienteGuionizado::trabajando(),
        false,
        false,
        Duration::from_secs(60),
    )
    .await;

    let error = entorno
        .lanzador
        .start(
            entorno.parent,
            "lejano".into(),
            "haz lo tuyo".into(),
            Some("remoto".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(
        error.to_string().contains("pendiente de aprobación"),
        "{error}"
    );
    assert!(entorno
        .store
        .list_subagents(entorno.parent)
        .await
        .unwrap()
        .is_empty());
    assert!(entorno.cliente.enviados.lock().unwrap().is_empty());
}
