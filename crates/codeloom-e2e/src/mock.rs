//! Provider simulado y espacio de trabajo temporal para lanzar CodeLoom aislado.

use std::io;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

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

/// Contesta como un padre que delega en un `subagent` y como un hijo que jamás da texto final.
///
/// Cabe señalar que el padre se reconoce porque su petición ofrece la herramienta `subagent`
/// (el hijo no la recibe): en su primer turno delega; en los siguientes, responde con `parent_reply`.
/// El hijo pide `ls` sin descanso, de modo que agota `max_steps` sin producir respuesta.
struct SubagentScript {
    parent_reply: String,
}

fn sse(chunks: &[serde_json::Value]) -> ResponseTemplate {
    let body: String = chunks
        .iter()
        .map(|chunk| {
            format!(
                "data: {chunk}

"
            )
        })
        .collect();
    ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
}

fn tool_call(name: &str, arguments: serde_json::Value) -> ResponseTemplate {
    sse(&[
        serde_json::json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":"call-1","function":{"name":name,"arguments":arguments.to_string()}}]}}]}),
        serde_json::json!({"choices":[{"finish_reason":"tool_calls"}]}),
    ])
}

impl Respond for SubagentScript {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = request.body_json().unwrap_or_default();
        let offers = |tool: &str| {
            body["tools"].as_array().is_some_and(|tools| {
                tools
                    .iter()
                    .any(|entry| entry.pointer("/function/name") == Some(&tool.into()))
            })
        };
        let already_delegated = body["messages"]
            .as_array()
            .is_some_and(|messages| messages.iter().any(|m| m["role"] == "tool"));
        if !offers("subagent") {
            // Cada paso varía el argumento para no parecer un bucle repetido.
            let step = body["messages"].as_array().map_or(0, |messages| {
                messages.iter().filter(|m| m["role"] == "tool").count()
            });
            return tool_call("ls", serde_json::json!({"path": ".", "paso": step}));
        }
        if already_delegated {
            return sse(&[
                serde_json::json!({"choices":[{"delta":{"content":self.parent_reply}}]}),
                serde_json::json!({"choices":[{"finish_reason":"stop"}]}),
            ]);
        }
        tool_call(
            "subagent",
            serde_json::json!({"prompt":"inspecciona el proyecto","label":"inspector"}),
        )
    }
}

/// Servidor OpenAI-compatible con el guion de un padre que delega en un hijo sin respuesta final.
pub async fn mock_provider_empty_subagent(parent_reply: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(SubagentScript {
            parent_reply: parent_reply.to_string(),
        })
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
            ("HOME".into(), home.clone()),
            ("CODELOOM_HOME".into(), home),
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
        assert!(env.contains(&("HOME".to_string(), home.clone())));
        assert!(env.contains(&("CODELOOM_HOME".to_string(), home)));
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
