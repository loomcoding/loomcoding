//! Tests: factory.

use super::support::*;
use super::*;

#[tokio::test]
async fn the_factory_names_the_missing_variable_and_builds_no_client_without_it() {
    let servidor = MockServer::start().await;
    let spec = remoto_con_seguridad(&servidor);
    let sin = fabrica(None);

    let motivo = sin.unavailable_reason("lejano", &spec).unwrap();

    assert!(motivo.contains("AGENTE_KEY"), "{motivo}");
    assert!(sin.client_for("lejano", &spec).is_none());
}

#[tokio::test]
async fn the_factory_builds_an_authenticated_client_when_the_variable_is_set() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("x-api-key", SECRETO))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;
    let spec = remoto_con_seguridad(&servidor);
    let con = fabrica(Some(SECRETO));

    assert!(con.unavailable_reason("lejano", &spec).is_none());
    let cliente = con.client_for("lejano", &spec).unwrap();

    assert!(cliente.send(Message::user_text("hola")).await.is_ok());
}

#[tokio::test]
async fn the_factory_builds_a_client_with_its_own_identity_for_an_mtls_remote() {
    let servidor = MockServer::start().await;
    let spec = remoto_mtls(&servidor);
    let ruta = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/mtls-test-identity.pem"
    );

    let con = fabrica_con_ruta(ruta);

    assert!(con.unavailable_reason("lejano", &spec).is_none());
    assert!(con.client_for("lejano", &spec).is_some());
}

#[tokio::test]
async fn an_mtls_remote_without_a_usable_identity_has_no_client_and_says_why() {
    let servidor = MockServer::start().await;
    let spec = remoto_mtls(&servidor);

    let sin = fabrica_con_ruta("/no/existe.pem");

    let motivo = sin.unavailable_reason("lejano", &spec).unwrap();
    assert!(
        motivo.contains("identidad TLS") && motivo.contains("AGENTE_TLS"),
        "{motivo}"
    );
    assert!(sin.client_for("lejano", &spec).is_none());
}

#[tokio::test]
async fn the_factory_refuses_a_remote_whose_security_it_cannot_read() {
    let servidor = MockServer::start().await;
    let mut spec = remoto_con_seguridad(&servidor);
    spec.fetched.as_mut().unwrap().security = None;

    let motivo = fabrica(Some(SECRETO))
        .unavailable_reason("lejano", &spec)
        .unwrap();

    assert!(motivo.contains("demasiado grande"), "{motivo}");
}
