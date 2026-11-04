//! Tests: parsing.

use super::support::*;
use super::*;

#[test]
fn parses_the_literal_sample_card() {
    let valor: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/agent-card.sample.json"
    ))
    .unwrap();
    let seguridad = CardSecurity::from_card_json(&valor);

    assert_eq!(
        seguridad.schemes["google"],
        SchemeKind::OpenIdConnect {
            discovery_url: "https://accounts.google.com/.well-known/openid-configuration".into()
        }
    );
    assert_eq!(seguridad.alternatives, vec![vec!["google".to_string()]]);
}

#[test]
fn oauth2_client_credentials_is_read_with_only_its_valid_scopes() {
    let valor = tarjeta(
        json!({ "oa": { "oauth2SecurityScheme": { "flows": { "clientCredentials": {
            "tokenUrl": "https://auth.example/token",
            "scopes": { "leer": "a", "con espacio": "b", "ok2": "", "com\"illa": "c" }
        } } } } }),
        json!([requisito(&["oa"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    assert_eq!(
        seguridad.schemes["oa"],
        SchemeKind::OAuth2ClientCredentials {
            token_url: "https://auth.example/token".into(),
            scopes: vec!["leer".into(), "ok2".into()],
        }
    );
}

#[test]
fn other_oauth2_flows_and_unsafe_token_urls_are_unsupported_with_a_reason() {
    let casos = [
        (
            json!({ "flows": { "authorizationCode": { "tokenUrl": "https://a.example/t" } } }),
            "sin el flujo client_credentials",
        ),
        (
            json!({ "flows": { "clientCredentials": { "tokenUrl": "http://a.example/t" } } }),
            "no es HTTPS",
        ),
        (
            json!({ "flows": { "clientCredentials": { "tokenUrl": "https://u:p@a.example/t" } } }),
            "credenciales incrustadas",
        ),
        (json!({ "flows": { "clientCredentials": {} } }), "tokenUrl"),
        (json!({}), "sin el flujo client_credentials"),
    ];
    for (esquema, esperado) in casos {
        let valor = tarjeta(
            json!({ "oa": { "oauth2SecurityScheme": esquema } }),
            json!([]),
        );
        let seguridad = CardSecurity::from_card_json(&valor);
        match &seguridad.schemes["oa"] {
            SchemeKind::Unsupported { what } => {
                assert!(what.contains(esperado), "{what} no contiene {esperado}")
            }
            otro => panic!("{otro:?}"),
        }
    }
}

#[test]
fn openid_connect_needs_a_safe_discovery_url() {
    let valor = tarjeta(
        json!({
            "ok": { "openIdConnectSecurityScheme": { "openIdConnectUrl": "https://a.example/.well-known/openid-configuration" } },
            "mal": { "openIdConnectSecurityScheme": { "openIdConnectUrl": "http://a.example/x" } },
        }),
        json!([]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    assert!(matches!(
        seguridad.schemes["ok"],
        SchemeKind::OpenIdConnect { .. }
    ));
    assert!(
        matches!(&seguridad.schemes["mal"], SchemeKind::Unsupported { what } if what.starts_with("OpenID Connect ("))
    );
}

#[test]
fn a_token_scheme_needs_an_id_and_secret_credential() {
    let valor = tarjeta(
        json!({ "oa": { "oauth2SecurityScheme": { "flows": { "clientCredentials": {
            "tokenUrl": "https://auth.example/token" } } } } }),
        json!([requisito(&["oa"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);
    let con = |valor: &'static str| {
        AuthPlan::choose(&seguridad, &credenciales(&[("oa", "V")]), &|v: &str| {
            (v == "V").then(|| valor.to_string())
        })
    };

    assert!(matches!(
        con("sin-dos-puntos"),
        Err(AuthProblem::InvalidCredential { .. })
    ));
    let plan = con("id:secreto").unwrap();
    assert!(plan.uses_token());
    // La credencial no se puede mostrar por descuido.
    assert!(!format!("{plan:?}").contains("secreto"));
}

#[test]
fn parses_api_key_header_and_query() {
    let valor = tarjeta(
        json!({
            "h": api_key_header(),
            "q": { "apiKeySecurityScheme": { "location": "QUERY", "name": "key" } },
        }),
        json!([]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    assert_eq!(
        seguridad.schemes["h"],
        SchemeKind::ApiKey {
            location: KeyLocation::Header,
            name: "X-Api-Key".into()
        }
    );
    assert_eq!(
        seguridad.schemes["q"],
        SchemeKind::ApiKey {
            location: KeyLocation::Query,
            name: "key".into()
        }
    );
}

#[test]
fn parses_http_bearer_and_basic_case_insensitively() {
    let valor = tarjeta(
        json!({
            "b": { "httpAuthSecurityScheme": { "scheme": "BEARER" } },
            "c": { "httpAuthSecurityScheme": { "scheme": "basic" } },
        }),
        json!([]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    assert_eq!(seguridad.schemes["b"], SchemeKind::HttpBearer);
    assert_eq!(seguridad.schemes["c"], SchemeKind::HttpBasic);
}

#[test]
fn cookie_api_key_digest_oauth2_and_mtls_are_unsupported_with_a_readable_name() {
    let valor = tarjeta(
        json!({
            "ck": { "apiKeySecurityScheme": { "location": "cookie", "name": "sid" } },
            "dg": { "httpAuthSecurityScheme": { "scheme": "Digest" } },
            "oa": { "oauth2SecurityScheme": { "flows": {} } },
            "dc": { "oauth2SecurityScheme": { "flows": { "deviceCode": { "tokenUrl": "https://a.example/t" } } } },
        }),
        json!([]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);
    let que = |n: &str| match &seguridad.schemes[n] {
        SchemeKind::Unsupported { what } => what.clone(),
        otro => panic!("{otro:?}"),
    };

    assert_eq!(que("ck"), "clave de API en cookie");
    assert_eq!(que("dg"), "HTTP digest");
    assert_eq!(que("oa"), "OAuth2 sin el flujo client_credentials");
    assert_eq!(que("dc"), "OAuth2 sin el flujo client_credentials");
}

#[test]
fn an_unknown_scheme_object_is_unsupported_not_ignored() {
    let valor = tarjeta(json!({ "x": { "magiaSecurityScheme": {} } }), json!([]));
    let seguridad = CardSecurity::from_card_json(&valor);

    assert!(matches!(
        seguridad.schemes["x"],
        SchemeKind::Unsupported { .. }
    ));
}

#[test]
fn json_roundtrip_keeps_the_security_and_bounds_it() {
    let valor = tarjeta(json!({ "k": api_key_header() }), json!([requisito(&["k"])]));
    let seguridad = CardSecurity::from_card_json(&valor);

    let texto = seguridad.to_json().unwrap();
    assert_eq!(CardSecurity::from_json(&texto), Some(seguridad));
    assert!(CardSecurity::from_json(&"x".repeat(MAX_JSON_BYTES + 1)).is_none());
    assert!(CardSecurity::from_json("no es json").is_none());
}

#[test]
fn names_from_a_third_party_are_cleaned_and_bounded() {
    let nombre = format!("a\nb{}", "z".repeat(500));
    let valor = tarjeta(
        json!({ nombre.clone(): api_key_header() }),
        json!([requisito(&[&nombre])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);
    let limpio = seguridad.schemes.keys().next().unwrap();

    assert!(!limpio.contains('\n'));
    assert!(limpio.len() <= MAX_NOMBRE + '…'.len_utf8());
    assert_eq!(&seguridad.alternatives[0][0], limpio);
}

#[test]
fn the_number_of_schemes_and_alternatives_is_capped() {
    let mut esquemas = serde_json::Map::new();
    for i in 0..50 {
        esquemas.insert(format!("e{i:02}"), api_key_header());
    }
    let requisitos: Vec<Value> = (0..50).map(|i| requisito(&[&format!("e{i:02}")])).collect();
    let seguridad =
        CardSecurity::from_card_json(&tarjeta(Value::Object(esquemas), Value::Array(requisitos)));

    assert_eq!(seguridad.schemes.len(), MAX_ESQUEMAS);
    assert_eq!(seguridad.alternatives.len(), MAX_ALTERNATIVAS);
}
