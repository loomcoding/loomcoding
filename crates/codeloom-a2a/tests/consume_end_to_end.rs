//! Consumo de un agente remoto de extremo a extremo (A8a): perfil `.md` → tarjeta descargada →
//! aprobación → lanzador → espejo → servidor simulado con `wiremock`.

mod support;

use std::time::Duration;

use codeloom_a2a::client::A2aClientFactory;
use codeloom_a2a::config::A2aConfig;
use codeloom_core::registry::Approval;
use codeloom_core::session::SessionStore;
use codeloom_core::subagent::SubagentLauncher;
use codeloom_core::task::TaskState;
use support::*;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn a_remote_agent_is_pending_then_approved_then_runs_to_completion() {
    let montaje = montar(3).await;
    assert_eq!(
        montaje.registro.snapshot().approval("lejano"),
        Some(Approval::Pending)
    );
    // Sin aprobar, no se le manda nada.
    let rechazo = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        rechazo.to_string().contains("pendiente de aprobación"),
        "{rechazo}"
    );
    assert!(metodos_recibidos(&montaje.servidor).await.is_empty());

    montaje.registro.approve("lejano").unwrap();
    let (hijo, _) = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    espera(&montaje, hijo, TaskState::Completed).await;
    let descriptor = montaje
        .store
        .list_subagents(montaje.parent)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.child == hijo)
        .unwrap();
    assert_eq!(descriptor.artifacts[0].text(), "hecho en remoto");
    assert_eq!(descriptor.agent.as_deref(), Some("lejano"));
    let metodos = metodos_recibidos(&montaje.servidor).await;
    assert_eq!(metodos[0], "SendMessage");
    assert_eq!(metodos[1], "SubscribeToTask", "{metodos:?}");
    assert!(metodos[2..].iter().all(|m| m == "GetTask"), "{metodos:?}");
    drop(montaje.bus);
}

#[tokio::test]
async fn cancelling_the_mirror_sends_cancel_task_to_the_remote() {
    let montaje = montar(0).await;
    montaje.registro.approve("lejano").unwrap();
    let (hijo, _) = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    espera(&montaje, hijo, TaskState::Working).await;
    // `Working` se publica antes de recibir el id remoto; `GetTask` confirma que ya está asignado.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !metodos_recibidos(&montaje.servidor)
            .await
            .contains(&"GetTask".to_string())
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("el remoto debe asignar el id antes de cancelar");

    let devuelto = montaje.lanzador.cancel(montaje.parent, hijo).await.unwrap();

    assert_eq!(devuelto.status.state, TaskState::Canceled);
    assert!(metodos_recibidos(&montaje.servidor)
        .await
        .contains(&"CancelTask".to_string()));
}

#[tokio::test]
async fn with_the_role_off_there_is_no_client_factory() {
    assert!(A2aClientFactory::from_config(&A2aConfig::default())
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn a_protected_remote_is_delegated_to_with_its_bearer_token_and_the_secret_leaks_nowhere() {
    let montaje = montar_con(2, true, Some(SECRETO)).await;
    let mut eventos = montaje.bus.subscribe();
    montaje.registro.approve("lejano").unwrap();

    let (hijo, _) = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    espera(&montaje, hijo, TaskState::Completed).await;

    // El remoto solo contesta con la credencial: que haya completado prueba que llegó.
    let metodos = metodos_recibidos(&montaje.servidor).await;
    assert_eq!(metodos[0], "SendMessage");
    // El secreto no aparece en ningún evento ni en lo que quedó guardado de la tarea.
    while let Ok(evento) = eventos.try_recv() {
        assert!(!format!("{evento:?}").contains(SECRETO), "{evento:?}");
    }
    let descriptores = montaje.store.list_subagents(montaje.parent).await.unwrap();
    assert!(!format!("{descriptores:?}").contains(SECRETO));
}

#[tokio::test]
async fn without_the_env_var_a_protected_remote_is_refused_and_nothing_is_sent() {
    let montaje = montar_con(2, true, None).await;
    montaje.registro.approve("lejano").unwrap();

    let error = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("AGENTE_KEY"), "{error}");
    assert!(metodos_recibidos(&montaje.servidor).await.is_empty());
    assert!(montaje
        .store
        .list_subagents(montaje.parent)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_remote_that_rejects_the_credential_fails_the_task_without_echoing_it() {
    // El remoto contesta 401 al token equivocado: el espejo no debe repetirlo en ningún sitio.
    let montaje = montar_con(2, true, Some("otro-token-incorrecto")).await;
    montaje.registro.approve("lejano").unwrap();

    let resultado = montaje
        .lanzador
        .start(
            montaje.parent,
            "tarea".into(),
            "hazlo".into(),
            Some("lejano".into()),
            CancellationToken::new(),
        )
        .await;

    // Con el token equivocado el servidor simulado no tiene ruta (404), así que el envío falla.
    let texto = match resultado {
        Ok((hijo, _)) => {
            espera(&montaje, hijo, TaskState::Failed).await;
            format!(
                "{:?}",
                montaje.store.list_subagents(montaje.parent).await.unwrap()
            )
        }
        Err(error) => error.to_string(),
    };
    assert!(!texto.contains("otro-token-incorrecto"), "{texto}");
}
