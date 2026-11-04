use super::*;

#[tokio::test]
async fn a_remote_agent_runs_to_completion_and_its_result_is_mirrored() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![
            Ok(tarea("r1", TaskState::Working, None, &[])),
            Ok(tarea(
                "r1",
                TaskState::Completed,
                None,
                &["resultado remoto"],
            )),
        ],
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(
        entorno.estado_del_aviso(&mut rx).await,
        TaskState::Completed
    );
    let descriptor = entorno.descriptor(hijo).await;
    assert_eq!(descriptor.status.state, TaskState::Completed);
    assert_eq!(descriptor.artifacts[0].text(), "resultado remoto");
    assert_eq!(descriptor.agent.as_deref(), Some("remoto"));
    let enviados = entorno.cliente.enviados.lock().unwrap();
    assert_eq!(enviados.len(), 1);
    assert_eq!(enviados[0].text(), "haz lo tuyo");
}

#[tokio::test]
async fn the_mirrored_states_follow_the_remote_in_order() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![
            Ok(tarea(
                "r1",
                TaskState::InputRequired,
                Some("¿qué color?"),
                &[],
            )),
            Ok(tarea("r1", TaskState::Working, None, &[])),
            Ok(tarea("r1", TaskState::Completed, None, &["azul"])),
        ],
    );
    cliente
        .respuestas
        .lock()
        .unwrap()
        .push_back(Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))));
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;
    responder(&entorno, hijo, "azul").await;

    let mut vistos: Vec<(TaskState, String)> = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(LiveEvent::SubagentChanged(d)) => {
                    let texto = d
                        .status
                        .message
                        .as_ref()
                        .map(Message::text)
                        .unwrap_or_default();
                    if vistos.last().map(|(s, _)| *s) != Some(d.status.state) {
                        vistos.push((d.status.state, texto));
                    }
                }
                Ok(LiveEvent::SubagentSettled { .. }) => return,
                _ => {}
            }
        }
    })
    .await
    .expect("debería llegar el aviso de cierre");

    let estados: Vec<TaskState> = vistos.iter().map(|(s, _)| *s).collect();
    assert_eq!(
        estados,
        vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::Working,
            TaskState::Completed
        ],
        "{vistos:?}"
    );
    assert_eq!(vistos[2].1, "¿qué color?");
}

fn pregunta_remota(cliente: &Arc<ClienteGuionizado>, texto: &str) {
    *cliente.envio.lock().unwrap() = Some(Ok(RemoteReply::Task(tarea(
        "r1",
        TaskState::InputRequired,
        Some(texto),
        &[],
    ))));
}

#[tokio::test]
async fn a_remote_input_required_is_asked_and_the_answer_resumes_the_task() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "¿qué color?");
    cliente
        .respuestas
        .lock()
        .unwrap()
        .push_back(Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Completed,
            None,
            &["azul"],
        ))));
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;
    let pregunta = responder(&entorno, hijo, "azul").await;

    assert_eq!(pregunta, "¿qué color?");
    assert_eq!(
        entorno.estado_del_aviso(&mut rx).await,
        TaskState::Completed
    );
    assert_eq!(entorno.descriptor(hijo).await.artifacts[0].text(), "azul");
    let enviados = entorno.cliente.enviados.lock().unwrap();
    assert_eq!(enviados.len(), 2);
    // La respuesta continúa la misma tarea del remoto.
    assert_eq!(enviados[1].text(), "azul");
    assert_eq!(enviados[1].task_id.as_deref(), Some("r1"));
    assert!(entorno.lanzador.pending_inputs(entorno.parent).is_empty());
}

#[tokio::test]
async fn the_remote_question_is_capped_and_cleaned() {
    let cliente = ClienteGuionizado::trabajando();
    let sucia = format!("hola\u{7}\u{1b}[31m{}", "x".repeat(10_000));
    pregunta_remota(&cliente, &sucia);
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;

    let hijo = entorno.lanzar().await;
    let pregunta = esperar_pregunta(&entorno, hijo).await;

    assert!(
        pregunta.len() < REMOTE_MAX_BYTES_PREGUNTA + 200,
        "{}",
        pregunta.len()
    );
    assert!(!pregunta.contains('\u{7}') && !pregunta.contains('\u{1b}'));
    assert!(pregunta.starts_with("hola"));
}

#[tokio::test]
async fn an_empty_remote_question_still_shows_something() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "   ");
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;

    let hijo = entorno.lanzar().await;

    assert!(!esperar_pregunta(&entorno, hijo).await.trim().is_empty());
}

#[tokio::test]
async fn a_cancel_while_waiting_cancels_the_remote_task_and_drops_the_question() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "¿color?");
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();
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
    esperar_pregunta(&entorno, hijo).await;

    cancel_padre.cancel();

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Canceled);
    assert_eq!(
        *entorno.cliente.canceladas.lock().unwrap(),
        vec!["r1".to_string()]
    );
    assert!(entorno.lanzador.pending_inputs(entorno.parent).is_empty());
}

#[tokio::test]
async fn too_many_question_rounds_fail_the_task() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "¿otra vez?");
    for _ in 0..REMOTE_MAX_RONDAS_ENTRADA {
        cliente
            .respuestas
            .lock()
            .unwrap()
            .push_back(Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::InputRequired,
                Some("¿otra vez?"),
                &[],
            ))));
    }
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;
    for _ in 0..REMOTE_MAX_RONDAS_ENTRADA {
        responder(&entorno, hijo, "sí").await;
    }

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    assert_eq!(
        *entorno.cliente.canceladas.lock().unwrap(),
        vec!["r1".to_string()]
    );
}

#[tokio::test]
async fn an_unanswered_question_times_out_and_cancels_the_remote_task() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "¿color?");
    let mut entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    entorno.lanzador = entorno
        .lanzador
        .with_remote_input_wait(Duration::from_millis(80));
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    assert_eq!(
        *entorno.cliente.canceladas.lock().unwrap(),
        vec!["r1".to_string()]
    );
    let razon = entorno
        .descriptor(hijo)
        .await
        .status
        .message
        .map(|m| m.text())
        .unwrap_or_default();
    assert!(razon.contains("in time"), "{razon}");
}

#[tokio::test]
async fn the_wait_for_the_user_does_not_count_against_the_follow_limit() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "¿color?");
    cliente
        .respuestas
        .lock()
        .unwrap()
        .push_back(Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Completed,
            None,
            &["a tiempo"],
        ))));
    // Seguimiento máximo de 300 ms; la persona tarda más que eso en contestar.
    let entorno = entorno_remoto(cliente, true, false, Duration::from_millis(300)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;
    esperar_pregunta(&entorno, hijo).await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    entorno
        .lanzador
        .answer(entorno.parent, hijo, vec!["azul".into()])
        .await
        .unwrap();

    assert_eq!(
        entorno.estado_del_aviso(&mut rx).await,
        TaskState::Completed
    );
}

#[tokio::test]
async fn a_message_reply_to_the_answer_is_the_result() {
    let cliente = ClienteGuionizado::trabajando();
    pregunta_remota(&cliente, "¿color?");
    cliente
        .respuestas
        .lock()
        .unwrap()
        .push_back(Ok(RemoteReply::Message(Message::agent_text("era azul"))));
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;
    responder(&entorno, hijo, "azul").await;

    assert_eq!(
        entorno.estado_del_aviso(&mut rx).await,
        TaskState::Completed
    );
    assert_eq!(
        entorno.descriptor(hijo).await.artifacts[0].text(),
        "era azul"
    );
}

#[tokio::test]
async fn a_failed_remote_task_fails_the_local_one_with_the_remote_message() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![Ok(tarea("r1", TaskState::Failed, Some("no pude"), &[]))],
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    let descriptor = entorno.descriptor(hijo).await;
    assert_eq!(descriptor.status.state, TaskState::Failed);
    assert_eq!(descriptor.status.message.unwrap().text(), "no pude");
}

#[tokio::test]
async fn a_rejected_or_canceled_remote_task_keeps_its_own_state_in_the_settled_notice() {
    for estado in [TaskState::Rejected, TaskState::Canceled] {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![Ok(tarea("r1", estado, Some("porque sí"), &[]))],
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, estado);
        assert_eq!(entorno.descriptor(hijo).await.status.state, estado);
    }
}

#[tokio::test]
async fn a_direct_message_reply_completes_the_task_with_its_text() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Message(Message::agent_text(
            "respuesta directa",
        ))),
        vec![],
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
        "respuesta directa"
    );
}

#[tokio::test]
async fn a_failed_send_fails_the_task_visibly() {
    let cliente = ClienteGuionizado::nuevo(Err(RemoteTaskError::Other("boom".into())), vec![]);
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    let mensaje = entorno
        .descriptor(hijo)
        .await
        .status
        .message
        .unwrap()
        .text();
    assert!(
        mensaje.contains("remote send failed") && mensaje.contains("boom"),
        "{mensaje}"
    );
}

#[tokio::test]
async fn losing_contact_after_three_failed_polls_fails_the_task() {
    let cliente = ClienteGuionizado::trabajando();
    *cliente.consulta_final.lock().unwrap() = Err(RemoteTaskError::Timeout);
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    let mensaje = entorno
        .descriptor(hijo)
        .await
        .status
        .message
        .unwrap()
        .text();
    assert!(mensaje.contains("lost contact"), "{mensaje}");
}

#[tokio::test]
async fn a_remote_task_that_never_finishes_is_given_up_and_the_remote_is_told() {
    let cliente = ClienteGuionizado::trabajando();
    let entorno = entorno_remoto(cliente, true, false, Duration::from_millis(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    let mensaje = entorno
        .descriptor(hijo)
        .await
        .status
        .message
        .unwrap()
        .text();
    assert!(mensaje.contains("gave up following"), "{mensaje}");
    assert_eq!(
        *entorno.cliente.canceladas.lock().unwrap(),
        vec!["r1".to_string()]
    );
}

#[tokio::test]
async fn the_remote_forgetting_the_task_fails_it() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![Err(RemoteTaskError::NotFound)],
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    let mensaje = entorno
        .descriptor(hijo)
        .await
        .status
        .message
        .unwrap()
        .text();
    assert!(mensaje.contains("no longer knows"), "{mensaje}");
}

#[tokio::test]
async fn cancel_asks_the_remote_to_cancel_and_says_only_what_it_answered() {
    let casos: Vec<(std::result::Result<RemoteTask, RemoteTaskError>, &str)> = vec![
        (
            Ok(tarea("r1", TaskState::Canceled, None, &[])),
            "cancellation confirmed by the remote",
        ),
        (
            Ok(tarea("r1", TaskState::Working, None, &[])),
            "the remote reports TASK_STATE_WORKING",
        ),
        (
            Err(RemoteTaskError::NotCancelable),
            "the remote says the task is not cancelable",
        ),
        (
            Err(RemoteTaskError::Other("caído".into())),
            "the remote did not confirm it",
        ),
    ];
    for (respuesta, esperado) in casos {
        let cliente = ClienteGuionizado::trabajando();
        *cliente.cancelacion.lock().unwrap() = respuesta;
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let hijo = entorno.lanzar().await;
        espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Working).await;

        let devuelto = entorno.lanzador.cancel(entorno.parent, hijo).await.unwrap();

        assert_eq!(devuelto.status.state, TaskState::Canceled);
        let mensaje = devuelto.status.message.unwrap().text();
        assert!(mensaje.contains(esperado), "{esperado}: {mensaje}");
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
    }
}

#[tokio::test]
async fn cancelling_the_parent_cancels_the_remote_task_too() {
    let cliente = ClienteGuionizado::trabajando();
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
}

#[tokio::test]
async fn a_streaming_remote_is_followed_by_the_stream_without_polling() {
    let cliente = ClienteGuionizado::trabajando();
    con_flujo(
        &cliente,
        GuionFlujo::Elementos(vec![
            Ok(tarea("r1", TaskState::Working, None, &[])),
            Ok(tarea("r1", TaskState::Completed, None, &["por el flujo"])),
        ]),
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();

    let hijo = entorno.lanzar().await;

    assert_eq!(
        entorno.estado_del_aviso(&mut rx).await,
        TaskState::Completed
    );
    let descriptor = entorno.descriptor(hijo).await;
    assert_eq!(descriptor.artifacts[0].text(), "por el flujo");
    assert_eq!(
        *entorno.cliente.sondeos.lock().unwrap(),
        0,
        "no debía sondear"
    );
}

#[tokio::test]
async fn a_remote_is_refused_with_the_reason_the_factory_gives() {
    let entorno = entorno_remoto(
        ClienteGuionizado::trabajando(),
        true,
        true,
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

    assert!(error.to_string().contains("AGENTE_KEY"), "{error}");
    assert!(error.to_string().contains("no se puede usar"), "{error}");
    assert!(entorno
        .store
        .list_subagents(entorno.parent)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn every_good_poll_of_a_remote_is_a_heartbeat_for_the_supervisor() {
    let cliente = ClienteGuionizado::nuevo(
        Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::Working,
            None,
            &[],
        ))),
        vec![
            Ok(tarea("r1", TaskState::Working, None, &[])),
            Ok(tarea("r1", TaskState::Working, None, &[])),
            Ok(tarea("r1", TaskState::Completed, None, &["listo"])),
        ],
    );
    let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
    let mut rx = entorno.bus.subscribe();
    let hijo = entorno.lanzar().await;
    let mut latidos = 0;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(LiveEvent::SubagentPhaseChanged { child, phase, .. }) if child == hijo => {
                    assert_eq!(phase, crate::subagent_phase::SubagentPhase::Working);
                    latidos += 1;
                }
                Ok(LiveEvent::SubagentSettled { .. }) => return,
                _ => {}
            }
        }
    })
    .await
    .expect("debería llegar el aviso de cierre");
    assert!(latidos >= 2, "solo {latidos} latidos");
}
