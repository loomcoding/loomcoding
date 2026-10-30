//! Provider simulado y espacio de trabajo temporal para lanzar CodeLoom aislado.

use std::io;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Variable de entorno de la que CodeLoom lee la clave en los E2E.
pub const KEY_ENV: &str = "CODELOOM_E2E_KEY";

/// Servidor compatible con OpenAI que contesta `POST /chat/completions` en SSE con `reply`.
pub async fn mock_provider(reply: &str) -> MockServer {
    let server = MockServer::start().await;
    let content = serde_json::to_string(reply).expect("un &str siempre es serializable");
    let body = format!(
        "data: {{\"choices\":[{{\"delta\":{{\"content\":{content}}}}}]}}\n\n\
         data: {{\"choices\":[{{\"finish_reason\":\"stop\"}}]}}\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;
    server
}

/// Perfil de usuario y proyecto temporales: nada de esto toca `~/.codeloom` del usuario.
pub struct Workspace {
    pub home: TempDir,
    pub project: TempDir,
}

impl Workspace {
    pub fn new(base_url: &str) -> io::Result<Workspace> {
        let home = tempfile::tempdir()?;
        let project = tempfile::tempdir()?;
        std::fs::write(
            project.path().join("codeloom.toml"),
            format!(
                "model = \"mock-model\"\nmax_steps = 3\n\n[provider]\nkind = \"openai_compat\"\n\
                 base_url = \"{base_url}\"\napi_key_env = \"{KEY_ENV}\"\n"
            ),
        )?;
        // Un catálogo reciente evita que el arranque intente descargar models.dev.
        let data = home.path().join(".codeloom");
        std::fs::create_dir_all(&data)?;
        std::fs::write(data.join("models-dev.json"), "{}")?;
        Ok(Workspace { home, project })
    }

    /// Variables de entorno que aíslan el perfil y entregan la clave de prueba.
    pub fn env(&self) -> Vec<(String, String)> {
        let home = self.home.path().to_string_lossy().to_string();
        vec![
            ("USERPROFILE".into(), home.clone()),
            ("HOME".into(), home),
            (KEY_ENV.into(), "sk-test".into()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeloom_core::catalog::{is_stale, read_cache};
    use codeloom_core::config::CodeLoomConfig;
    use std::io::{Read, Write};
    use std::time::SystemTime;

    #[test]
    fn workspace_writes_config_and_catalog() {
        let ws = Workspace::new("http://127.0.0.1:1").unwrap();
        let cfg = CodeLoomConfig::load(&ws.project.path().join("codeloom.toml")).unwrap();
        assert_eq!(cfg.provider.base_url, "http://127.0.0.1:1");
        assert_eq!(cfg.provider.api_key_env, "CODELOOM_E2E_KEY");
        assert_eq!(cfg.model, "mock-model");

        let cache = ws.home.path().join(".codeloom").join("models-dev.json");
        let (_, modified) = read_cache(&cache).expect("la caché sembrada debe ser válida");
        assert!(!is_stale(modified, SystemTime::now()));
    }

    #[test]
    fn workspace_env_points_the_profile_at_the_temp_home() {
        let ws = Workspace::new("http://127.0.0.1:1").unwrap();
        let env = ws.env();
        let home = ws.home.path().to_string_lossy().to_string();
        assert!(env.contains(&("USERPROFILE".to_string(), home.clone())));
        assert!(env.contains(&("HOME".to_string(), home)));
        assert!(env.contains(&("CODELOOM_E2E_KEY".to_string(), "sk-test".to_string())));
    }

    #[tokio::test]
    async fn mock_provider_serves_sse() {
        let server = mock_provider("hola mundo").await;
        let addr = *server.address();
        let reply = tokio::task::spawn_blocking(move || {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            stream
                .write_all(
                    b"POST /chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                )
                .unwrap();
            let mut out = String::new();
            stream.read_to_string(&mut out).unwrap();
            out
        })
        .await
        .unwrap();
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert!(
            reply.to_lowercase().contains("text/event-stream"),
            "{reply}"
        );
        assert!(reply.contains("hola mundo"), "{reply}");
        assert!(
            reply.contains("[DONE]") || reply.contains("\"stop\""),
            "{reply}"
        );
    }
}
