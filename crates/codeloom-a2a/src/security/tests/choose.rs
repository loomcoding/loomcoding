//! Tests: choose.

use super::support::*;
use super::*;

#[test]
fn choose_takes_the_first_alternative_whose_schemes_are_all_supported_and_have_a_credential() {
    let valor = tarjeta(
        json!({
            "oa": { "oauth2SecurityScheme": { "flows": {} } },
            "k": api_key_header(),
        }),
        json!([requisito(&["oa"]), requisito(&["k"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    let plan = AuthPlan::choose(
        &seguridad,
        &credenciales(&[("k", "MI_CLAVE")]),
        &entorno(&[("MI_CLAVE", "abc")]),
    )
    .unwrap();

    let peticion = construir(&plan);
    assert_eq!(peticion.headers()["x-api-key"], "abc");
}

#[test]
fn choose_requires_every_scheme_of_an_alternative() {
    let valor = tarjeta(
        json!({ "k": api_key_header(), "b": { "httpAuthSecurityScheme": { "scheme": "Bearer" } } }),
        json!([requisito(&["b", "k"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(
        &seguridad,
        &credenciales(&[("k", "CLAVE"), ("b", "TOKEN")]),
        &entorno(&[("CLAVE", "abc")]),
    )
    .unwrap_err();

    assert_eq!(
        error,
        AuthProblem::MissingCredential {
            scheme: "b".into(),
            env: Some("TOKEN".into())
        }
    );
    assert!(error.to_string().contains("TOKEN"));
}

#[test]
fn an_empty_env_value_counts_as_missing() {
    let valor = tarjeta(json!({ "k": api_key_header() }), json!([requisito(&["k"])]));
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(
        &seguridad,
        &credenciales(&[("k", "VACIA")]),
        &entorno(&[("VACIA", "   ")]),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        AuthProblem::MissingCredential { env: Some(_), .. }
    ));
}

#[test]
fn a_scheme_without_a_declared_credential_says_where_to_declare_it() {
    let valor = tarjeta(json!({ "k": api_key_header() }), json!([requisito(&["k"])]));
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(&seguridad, &credenciales(&[]), &entorno(&[])).unwrap_err();

    assert!(error.to_string().contains("a2a.credentials"), "{error}");
}

#[test]
fn choose_reports_the_first_alternative_when_none_works() {
    let valor = tarjeta(
        json!({
            "oa": { "oauth2SecurityScheme": { "flows": {} } },
            "ck": { "apiKeySecurityScheme": { "location": "cookie", "name": "sid" } },
        }),
        json!([requisito(&["oa"]), requisito(&["ck"])]),
    );
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(&seguridad, &credenciales(&[]), &entorno(&[])).unwrap_err();

    assert_eq!(
        error,
        AuthProblem::Unsupported {
            scheme: "oa".into(),
            kind: "OAuth2 sin el flujo client_credentials".into()
        }
    );
}

#[test]
fn an_open_card_chooses_the_open_plan() {
    let seguridad = CardSecurity::from_card_json(&json!({ "name": "x" }));
    let plan = AuthPlan::choose(&seguridad, &credenciales(&[]), &entorno(&[])).unwrap();

    assert!(plan.is_open());
    assert!(construir(&plan).headers().get("authorization").is_none());
    assert!(seguridad.is_empty());
    assert!(seguridad.to_json().is_none());
}

#[test]
fn an_empty_requirement_means_no_authentication_is_needed() {
    let valor = tarjeta(json!({ "k": api_key_header() }), json!([requisito(&[])]));
    let seguridad = CardSecurity::from_card_json(&valor);

    let plan = AuthPlan::choose(&seguridad, &credenciales(&[]), &entorno(&[])).unwrap();

    assert!(plan.is_open());
}

#[test]
fn a_requirement_naming_an_undefined_scheme_is_unsupported() {
    let valor = tarjeta(json!({}), json!([requisito(&["fantasma"])]));
    let seguridad = CardSecurity::from_card_json(&valor);

    let error = AuthPlan::choose(&seguridad, &credenciales(&[]), &entorno(&[])).unwrap_err();

    assert!(matches!(error, AuthProblem::Unsupported { .. }));
}

#[test]
fn for_card_refuses_a_card_whose_security_could_not_be_stored() {
    let error = AuthPlan::for_card(None, true, &credenciales(&[]), &entorno(&[])).unwrap_err();
    assert!(error.contains("demasiado grande"), "{error}");

    let error = AuthPlan::for_card(Some("no es json"), true, &credenciales(&[]), &entorno(&[]))
        .unwrap_err();
    assert!(error.contains("no se pudo leer"), "{error}");

    assert!(
        AuthPlan::for_card(None, false, &credenciales(&[]), &entorno(&[]))
            .unwrap()
            .is_open()
    );
}
