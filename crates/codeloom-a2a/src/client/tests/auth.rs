//! Tests: auth.

use super::support::*;
use super::*;

#[tokio::test]
async fn sends_the_api_key_header() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("x-api-key", SECRETO))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;
    let auth = plan(
        "claveApi",
        json!({ "apiKeySecurityScheme": { "location": "header", "name": "X-Api-Key" } }),
        SECRETO,
    );

    let respuesta = enviar_con(&servidor, auth).await.unwrap();

    assert!(matches!(respuesta, RemoteReply::Task(_)));
}

#[tokio::test]
async fn sends_the_api_key_in_the_query() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(query_param("key", SECRETO))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;
    let auth = plan(
        "claveApi",
        json!({ "apiKeySecurityScheme": { "location": "query", "name": "key" } }),
        SECRETO,
    );

    assert!(enviar_con(&servidor, auth).await.is_ok());
}

#[tokio::test]
async fn an_oauth2_remote_is_called_with_a_token_obtained_from_the_card_token_url() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token("tok-1"))
        .expect(1)
        .mount(&servidor)
        .await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(EnvioConId)
        .expect(2)
        .mount(&servidor)
        .await;
    let auth = plan("oa", esquema_oauth(&servidor), "cliente:secreto");
    let c = cliente(&servidor, None).with_auth(auth);

    // Dos llamadas, un solo token: el segundo sale de la caché.
    c.send(Message::user_text("a")).await.unwrap();
    c.send(Message::user_text("b")).await.unwrap();

    servidor.verify().await;
}

#[tokio::test]
async fn a_rejected_token_is_renewed_once_and_the_call_retried() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token("tok-1"))
        .up_to_n_times(1)
        .mount(&servidor)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token("tok-2"))
        .mount(&servidor)
        .await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&servidor)
        .await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("authorization", "Bearer tok-2"))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;
    let auth = plan("oa", esquema_oauth(&servidor), "cliente:secreto");

    assert!(enviar_con(&servidor, auth).await.is_ok());
}

#[tokio::test]
async fn a_remote_that_keeps_rejecting_the_token_fails_after_one_retry_only() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token("tok"))
        .mount(&servidor)
        .await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .respond_with(ResponseTemplate::new(401).set_body_string("tok cliente:secreto"))
        .expect(2)
        .mount(&servidor)
        .await;
    let auth = plan("oa", esquema_oauth(&servidor), "cliente:secreto");

    let error = enviar_con(&servidor, auth).await.unwrap_err();

    assert!(matches!(error, RemoteTaskError::AuthFailed(_)), "{error:?}");
    assert!(!error.to_string().contains("secreto"));
    servidor.verify().await;
}

#[tokio::test]
async fn when_no_token_can_be_obtained_the_call_is_never_sent_without_credentials() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&servidor)
        .await;
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .respond_with(respuesta_de_envio())
        .expect(0)
        .mount(&servidor)
        .await;
    let auth = plan("oa", esquema_oauth(&servidor), "cliente:secreto");

    let error = enviar_con(&servidor, auth).await.unwrap_err();

    assert!(matches!(error, RemoteTaskError::AuthFailed(_)), "{error:?}");
    servidor.verify().await;
}

#[tokio::test]
async fn sends_bearer_and_basic() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header(
            "authorization",
            format!("Bearer {SECRETO}").as_str(),
        ))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;
    let bearer = plan(
        "t",
        json!({ "httpAuthSecurityScheme": { "scheme": "Bearer" } }),
        SECRETO,
    );
    assert!(enviar_con(&servidor, bearer).await.is_ok());

    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("authorization", "Basic dXN1YXJpbzpjbGF2ZQ=="))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;
    let basic = plan(
        "b",
        json!({ "httpAuthSecurityScheme": { "scheme": "Basic" } }),
        "usuario:clave",
    );
    assert!(enviar_con(&servidor, basic).await.is_ok());
}

#[tokio::test]
async fn an_open_remote_gets_no_credential_at_all() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(respuesta_de_envio())
        .mount(&servidor)
        .await;

    cliente(&servidor, None)
        .send(Message::user_text("hola"))
        .await
        .unwrap();

    let recibidas = servidor.received_requests().await.unwrap();
    assert_eq!(recibidas.len(), 1);
    assert!(recibidas[0].headers.get("authorization").is_none());
    assert!(recibidas[0].url.query().is_none());
}

#[tokio::test]
async fn a_401_or_403_is_an_auth_failure_that_never_repeats_the_credential() {
    for estado in [401, 403] {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(estado).set_body_string(format!("token inválido: {SECRETO}")),
            )
            .mount(&servidor)
            .await;
        let auth = plan(
            "t",
            json!({ "httpAuthSecurityScheme": { "scheme": "Bearer" } }),
            SECRETO,
        );

        let error = enviar_con(&servidor, auth).await.unwrap_err();

        assert!(
            matches!(error, RemoteTaskError::AuthFailed(_)),
            "{estado}: {error:?}"
        );
        assert!(!error.to_string().contains(SECRETO), "{error}");
        assert!(!format!("{error:?}").contains(SECRETO), "{error:?}");
    }
}
