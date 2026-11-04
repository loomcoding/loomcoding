//! Tests: auth.

use super::support::*;
use super::*;

#[tokio::test]
async fn without_a_correct_credential_the_answer_is_an_empty_401() {
    let (estado, host) = bearer(Falso::default());
    let cuerpo = envio("hola").to_string();
    for auth in [
        None,
        Some(""),
        Some("Bearer "),
        Some("Bearer s3cr3to-de-pru"),
        Some("Bearer s3cr3to-de-prueba-y-mas"),
        Some("Basic s3cr3to-de-prueba"),
    ] {
        let (http, cabeceras, bytes) =
            enviar_req(&estado, rpc_peticion(&cuerpo, auth, Some("1.0"))).await;

        assert_eq!(http, StatusCode::UNAUTHORIZED, "{auth:?}");
        assert!(bytes.is_empty(), "{auth:?}: el cuerpo no debe informar");
        assert_eq!(cabeceras[header::WWW_AUTHENTICATE], "Bearer");
    }
    assert!(host.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_api_key_server_reads_its_own_header() {
    let (estado, _) = estado(Falso::default(), ServeAuthKind::ApiKey);
    let peticion = Request::builder()
        .method("POST")
        .uri("/a2a")
        .header("x-api-key", SECRETO)
        .header("a2a-version", "1.0")
        .body(Body::from(envio("hola").to_string()))
        .unwrap();

    let (http, _, _) = enviar_req(&estado, peticion).await;
    assert_eq!(http, StatusCode::OK);

    let (http, cabeceras, _) = enviar_req(&estado, buena(envio("hola"))).await;
    assert_eq!(http, StatusCode::UNAUTHORIZED);
    assert!(cabeceras.get(header::WWW_AUTHENTICATE).is_none());
}
