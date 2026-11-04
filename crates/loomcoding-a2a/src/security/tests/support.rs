//! Piezas comunes de los tests.

use super::*;

pub(super) fn tarjeta(esquemas: Value, requisitos: Value) -> Value {
    json!({ "securitySchemes": esquemas, "securityRequirements": requisitos })
}

pub(super) fn api_key_header() -> Value {
    json!({ "apiKeySecurityScheme": { "location": "header", "name": "X-Api-Key" } })
}

pub(super) fn requisito(nombres: &[&str]) -> Value {
    let mapa: serde_json::Map<String, Value> = nombres
        .iter()
        .map(|n| (n.to_string(), json!({ "list": [] })))
        .collect();
    json!({ "schemes": mapa })
}

pub(super) fn entorno<'a>(pares: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |nombre| {
        pares
            .iter()
            .find(|(k, _)| *k == nombre)
            .map(|(_, v)| v.to_string())
    }
}

pub(super) fn credenciales(pares: &[(&str, &str)]) -> BTreeMap<String, String> {
    pares
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

pub(super) fn construir(plan: &AuthPlan) -> reqwest::Request {
    let cliente = reqwest::Client::new();
    // Las pruebas de este módulo son síncronas: se espera la aplicación del plan en un
    // ejecutor propio (los planes sin token no esperan nada).
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(plan.apply(cliente.post("http://127.0.0.1:1/rpc")))
        .unwrap()
        .build()
        .unwrap()
}

pub(super) fn tarjeta_mtls() -> CardSecurity {
    CardSecurity::from_card_json(&tarjeta(
        json!({ "tls": { "mtlsSecurityScheme": {} } }),
        json!([requisito(&["tls"])]),
    ))
}

pub(super) const IDENTIDAD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/mtls-test-identity.pem"
);

pub(super) fn plan_mtls(ruta: &str) -> Result<AuthPlan, AuthProblem> {
    let ruta = ruta.to_string();
    AuthPlan::choose(
        &tarjeta_mtls(),
        &credenciales(&[("tls", "V")]),
        &move |v: &str| (v == "V").then(|| ruta.clone()),
    )
}
