//! Tests: apply.

use super::support::*;
use super::*;

#[test]
fn secret_debug_and_display_never_show_the_value() {
    let secreto = Secret::new("super-secreto".into()).unwrap();

    assert!(!format!("{secreto:?}").contains("super-secreto"));
    assert!(!format!("{secreto}").contains("super-secreto"));
    assert!(Secret::new("  ".into()).is_none());
}

#[test]
fn the_plan_never_shows_its_credential_in_debug() {
    let valor = tarjeta(json!({ "k": api_key_header() }), json!([requisito(&["k"])]));
    let seguridad = CardSecurity::from_card_json(&valor);
    let plan = AuthPlan::choose(
        &seguridad,
        &credenciales(&[("k", "V")]),
        &entorno(&[("V", "super-secreto")]),
    )
    .unwrap();

    assert!(!format!("{plan:?}").contains("super-secreto"));
    assert!(construir(&plan).headers()["x-api-key"].is_sensitive());
}

#[test]
fn apply_sets_header_bearer_basic_and_query() {
    let valor = tarjeta(
        json!({
            "b": { "httpAuthSecurityScheme": { "scheme": "Bearer" } },
            "c": { "httpAuthSecurityScheme": { "scheme": "Basic" } },
            "q": { "apiKeySecurityScheme": { "location": "query", "name": "key" } },
        }),
        json!([requisito(&["b"]), requisito(&["c"]), requisito(&["q"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);
    let con = |esquema: &str, valor: &'static str| {
        let solo = CardSecurity {
            schemes: seguridad.schemes.clone(),
            alternatives: vec![vec![esquema.to_string()]],
        };
        AuthPlan::choose(
            &solo,
            &credenciales(&[(esquema, "V")]),
            &entorno(&[("V", valor)]),
        )
        .unwrap()
    };

    let bearer = construir(&con("b", "tok"));
    assert_eq!(bearer.headers()["authorization"], "Bearer tok");

    let basic = construir(&con("c", "usuario:clave"));
    assert_eq!(
        basic.headers()["authorization"],
        format!("Basic {}", STANDARD.encode("usuario:clave")).as_str()
    );

    let consulta = construir(&con("q", "s3"));
    assert_eq!(consulta.url().query(), Some("key=s3"));
}

#[test]
fn basic_without_a_colon_is_an_invalid_credential() {
    let valor = tarjeta(
        json!({ "c": { "httpAuthSecurityScheme": { "scheme": "Basic" } } }),
        json!([requisito(&["c"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(
        &seguridad,
        &credenciales(&[("c", "V")]),
        &entorno(&[("V", "sin-dos-puntos")]),
    )
    .unwrap_err();

    assert!(matches!(error, AuthProblem::InvalidCredential { .. }));
}

#[test]
fn a_header_value_with_control_characters_is_an_invalid_credential() {
    let valor = tarjeta(json!({ "k": api_key_header() }), json!([requisito(&["k"])]));
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(
        &seguridad,
        &credenciales(&[("k", "V")]),
        &entorno(&[("V", "a\nb")]),
    )
    .unwrap_err();

    assert!(matches!(error, AuthProblem::InvalidCredential { .. }));
}
