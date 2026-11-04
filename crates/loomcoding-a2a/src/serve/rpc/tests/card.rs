//! Tests: card.

use super::support::*;
use super::*;

#[tokio::test]
async fn the_agent_card_is_public_and_cacheable() {
    let (estado, _) = bearer(Falso::default());
    let peticion = Request::builder()
        .uri("/.well-known/agent-card.json")
        .body(Body::empty())
        .unwrap();

    let (http, cabeceras, bytes) = enviar_req(&estado, peticion).await;

    assert_eq!(http, StatusCode::OK);
    assert_eq!(cabeceras[header::CACHE_CONTROL], "max-age=60");
    let etag = cabeceras[header::ETAG].to_str().unwrap().to_string();
    let tarjeta: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(tarjeta["name"], "default");

    let condicional = Request::builder()
        .uri("/.well-known/agent-card.json")
        .header("if-none-match", etag)
        .body(Body::empty())
        .unwrap();
    let (http, _, bytes) = enviar_req(&estado, condicional).await;
    assert_eq!(http, StatusCode::NOT_MODIFIED);
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn a_tenant_card_is_directly_discoverable_without_a_list_operation() {
    let (estado, _) = bearer(Falso::default());
    let tarjeta = crate::serve::card::served_card_for_tenant(
        &AgentCard::local("agent-one", "runtime agent", "1.0.0"),
        "http://127.0.0.1:7077/a2a",
        Some("tenant-one"),
        estado.auth.scheme(),
    );
    estado
        .agent_cards
        .write()
        .unwrap()
        .insert("tenant-one".into(), tarjeta);
    let request = Request::builder()
        .uri("/agents/tenant-one/.well-known/agent-card.json")
        .body(Body::empty())
        .unwrap();

    let (http, _, bytes) = enviar_req(&estado, request).await;

    assert_eq!(http, StatusCode::OK);
    let card: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(card["supportedInterfaces"][0]["tenant"], "tenant-one");
    assert_eq!(
        card["supportedInterfaces"][0]["url"],
        "http://127.0.0.1:7077/a2a"
    );
}
