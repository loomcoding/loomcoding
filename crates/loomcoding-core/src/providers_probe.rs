//! Sondeo de proveedores locales: una sola petición `GET <base_url>/models` por proveedor.
//!
//! Nunca se lanza una inferencia: el sondeo solo pregunta qué modelos hay. El tiempo máximo, la
//! cota de respuesta y el número de modelos están acotados (véase la spec de proveedores locales).

use std::time::Duration;

use serde::Deserialize;

use crate::providers_registry::LocalProvider;

/// Tiempo máximo de un sondeo, de extremo a extremo.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
/// Bytes máximos que se leen de la respuesta; lo que exceda se descarta.
pub const MAX_BODY_BYTES: usize = 256 * 1024;
/// Modelos máximos que se conservan de la lista de un proveedor.
pub const MAX_MODELS: usize = 200;

/// Resultado del sondeo de un proveedor local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeStatus {
    /// Respondió con la lista de modelos (ids, como máximo `MAX_MODELS`).
    Up { models: Vec<String> },
    /// Rechazo de conexión, tiempo agotado o respuesta que no es una lista OpenAI.
    Down,
    /// 401 o 403: falta o sobra la clave.
    Auth,
}

/// Forma mínima de `GET /v1/models` en formato OpenAI.
#[derive(Debug, Deserialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

/// Sondea `provider` con `GET <base_url>/models`, con la clave como `Bearer` si la hay.
///
/// Todo fallo se traduce a un estado: nunca devuelve error, de modo que un servidor caído no
/// impide arrancar ni bloquea la interfaz.
pub async fn probe(provider: &LocalProvider, key: Option<&str>) -> ProbeStatus {
    let cliente = match reqwest::Client::builder().timeout(PROBE_TIMEOUT).build() {
        Ok(cliente) => cliente,
        Err(error) => {
            tracing::warn!(id = %provider.id, %error, "no se pudo preparar el sondeo del proveedor local");
            return ProbeStatus::Down;
        }
    };
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let mut peticion = cliente.get(&url);
    if let Some(clave) = key {
        peticion = peticion.bearer_auth(clave);
    }

    let mut respuesta = match peticion.send().await {
        Ok(respuesta) => respuesta,
        Err(error) => {
            tracing::debug!(id = %provider.id, %error, "el proveedor local no responde");
            return ProbeStatus::Down;
        }
    };
    let estado = respuesta.status();
    if estado == reqwest::StatusCode::UNAUTHORIZED || estado == reqwest::StatusCode::FORBIDDEN {
        return ProbeStatus::Auth;
    }
    if !estado.is_success() {
        tracing::debug!(id = %provider.id, %estado, "el proveedor local rechazó el sondeo");
        return ProbeStatus::Down;
    }

    let Some(cuerpo) = leer_acotado(&mut respuesta).await else {
        tracing::warn!(id = %provider.id, "la respuesta del proveedor local supera 256 KiB; se descarta");
        return ProbeStatus::Down;
    };
    match serde_json::from_slice::<ModelList>(&cuerpo) {
        Ok(lista) => ProbeStatus::Up {
            models: lista
                .data
                .into_iter()
                .take(MAX_MODELS)
                .map(|modelo| modelo.id)
                .collect(),
        },
        Err(error) => {
            tracing::warn!(id = %provider.id, %error, "la respuesta del proveedor local no es una lista de modelos");
            ProbeStatus::Down
        }
    }
}

/// Lee la respuesta completa si cabe en `MAX_BODY_BYTES`; si no cabe, devuelve `None` y deja de leer.
async fn leer_acotado(respuesta: &mut reqwest::Response) -> Option<Vec<u8>> {
    let mut cuerpo = Vec::new();
    loop {
        match respuesta.chunk().await {
            Ok(Some(trozo)) => {
                if cuerpo.len() + trozo.len() > MAX_BODY_BYTES {
                    return None;
                }
                cuerpo.extend_from_slice(&trozo);
            }
            Ok(None) => return Some(cuerpo),
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn provider(base_url: &str) -> LocalProvider {
        LocalProvider {
            id: "prueba".into(),
            name: "prueba".into(),
            kind: "openai_compat".into(),
            base_url: base_url.into(),
            api_key_env: None,
            model: "reserva".into(),
            context_window: None,
        }
    }

    /// Base URL de un puerto que acaba de quedar libre: la conexión será rechazada.
    fn puerto_cerrado() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        format!("http://127.0.0.1:{port}/v1")
    }

    #[tokio::test]
    async fn responde_con_los_ids_de_los_modelos() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"id": "uno", "object": "model"}, {"id": "dos", "object": "model"}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let status = probe(&provider(&format!("{}/v1", server.uri())), None).await;

        assert_eq!(
            status,
            ProbeStatus::Up {
                models: vec!["uno".into(), "dos".into()]
            }
        );
    }

    #[tokio::test]
    async fn solo_hace_get_a_models_y_envia_la_clave_como_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer secreto"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .expect(1)
            .mount(&server)
            .await;

        let status = probe(&provider(&format!("{}/v1", server.uri())), Some("secreto")).await;

        assert_eq!(status, ProbeStatus::Up { models: vec![] });
        let peticiones = server.received_requests().await.unwrap();
        assert_eq!(peticiones.len(), 1, "una sola petición por sondeo");
        assert_eq!(peticiones[0].method.as_str(), "GET");
        assert_eq!(peticiones[0].url.path(), "/v1/models");
    }

    #[tokio::test]
    async fn sin_clave_no_envia_cabecera_de_autorizacion() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .mount(&server)
            .await;

        probe(&provider(&format!("{}/v1", server.uri())), None).await;

        let peticiones = server.received_requests().await.unwrap();
        assert!(peticiones[0].headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn respuesta_401_o_403_es_auth() {
        for codigo in [401u16, 403] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/v1/models"))
                .respond_with(ResponseTemplate::new(codigo))
                .mount(&server)
                .await;

            let status = probe(&provider(&format!("{}/v1", server.uri())), None).await;

            assert_eq!(status, ProbeStatus::Auth, "código {codigo}");
        }
    }

    #[tokio::test]
    async fn conexion_rechazada_es_down() {
        let status = probe(&provider(&puerto_cerrado()), None).await;

        assert_eq!(status, ProbeStatus::Down);
    }

    #[tokio::test]
    async fn tiempo_agotado_es_down_en_menos_de_tres_segundos() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"data": []}))
                    .set_delay(Duration::from_secs(5)),
            )
            .mount(&server)
            .await;

        let inicio = std::time::Instant::now();
        let status = probe(&provider(&format!("{}/v1", server.uri())), None).await;

        assert_eq!(status, ProbeStatus::Down);
        assert!(
            inicio.elapsed() < Duration::from_secs(3),
            "tardó {:?}",
            inicio.elapsed()
        );
    }

    #[tokio::test]
    async fn cuerpo_sin_data_es_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"models": []})),
            )
            .mount(&server)
            .await;

        let status = probe(&provider(&format!("{}/v1", server.uri())), None).await;

        assert_eq!(status, ProbeStatus::Down);
    }

    #[tokio::test]
    async fn respuesta_mayor_de_256_kib_se_corta_y_es_down() {
        let server = MockServer::start().await;
        let relleno = "x".repeat(MAX_BODY_BYTES + 1024);
        let cuerpo = format!("{{\"data\":[{{\"id\":\"a\"}}],\"relleno\":\"{relleno}\"}}");
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string(cuerpo))
            .mount(&server)
            .await;

        let status = probe(&provider(&format!("{}/v1", server.uri())), None).await;

        assert_eq!(status, ProbeStatus::Down);
    }

    #[tokio::test]
    async fn se_conservan_como_mucho_200_modelos() {
        let server = MockServer::start().await;
        let data: Vec<serde_json::Value> = (0..250)
            .map(|i| serde_json::json!({"id": format!("m{i}")}))
            .collect();
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": data})),
            )
            .mount(&server)
            .await;

        let status = probe(&provider(&format!("{}/v1", server.uri())), None).await;

        match status {
            ProbeStatus::Up { models } => {
                assert_eq!(models.len(), MAX_MODELS);
                assert_eq!(models[0], "m0");
            }
            otro => panic!("se esperaba Up, llegó {otro:?}"),
        }
    }
}
