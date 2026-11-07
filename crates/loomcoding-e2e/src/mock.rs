//! Provider simulado y espacio de trabajo temporal para lanzar Loomcoding aislado.

use std::io;
use std::time::Duration;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Variable de entorno de la que Loomcoding lee la clave en los E2E.
pub const KEY_ENV: &str = "LOOMCODING_E2E_KEY";

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
/// El hijo pide `ls` sin descanso, de modo que agota el tope de rondas de lectura (`4 × max_steps`) sin producir respuesta.
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

/// Padre que delega en un hijo y hijo que, antes de contestar, pregunta a la persona con `ask_user`.
///
/// El padre se reconoce, como en `SubagentScript`, porque su petición ofrece `subagent`. El hijo
/// pregunta en su primer turno y, una vez tiene la respuesta (un mensaje `tool`), cierra con texto.
struct ChildAsksScript;

impl Respond for ChildAsksScript {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = request.body_json().unwrap_or_default();
        let offers_subagent = body["tools"].as_array().is_some_and(|tools| {
            tools
                .iter()
                .any(|entry| entry.pointer("/function/name") == Some(&"subagent".into()))
        });
        let has_tool_result = body["messages"]
            .as_array()
            .is_some_and(|messages| messages.iter().any(|m| m["role"] == "tool"));
        let say = |text: &str| {
            sse(&[
                serde_json::json!({"choices":[{"delta":{"content":text}}]}),
                serde_json::json!({"choices":[{"finish_reason":"stop"}]}),
            ])
        };
        match (offers_subagent, has_tool_result) {
            (true, false) => tool_call(
                "subagent",
                serde_json::json!({"prompt":"pregunta el color","label":"preguntón"}),
            ),
            (true, true) => say("delegado, espero al hijo"),
            (false, false) => tool_call(
                "ask_user",
                serde_json::json!({"questions":[{"question":"¿Qué color prefieres?",
                    "options":[{"label":"verde"},{"label":"rojo"}]}]}),
            ),
            (false, true) => say("anotado, gracias"),
        }
    }
}

/// Guion de un padre que lanza un job vigilado y, cuando le llega el aviso `[Job … watch]`, despierta.
///
/// Primero ofrece `job_start` con un `watch` cuyo patrón casa con la línea que imprime el job. Con el
/// resultado de la herramienta contesta «job lanzado»; si en el historial ya hay un aviso de vigilancia,
/// contesta «aviso recibido», que es la señal de que el turno despertado ocurrió.
struct WatchScript;

impl Respond for WatchScript {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = request.body_json().unwrap_or_default();
        let has_tool_result = body["messages"]
            .as_array()
            .is_some_and(|messages| messages.iter().any(|m| m["role"] == "tool"));
        let has_watch_notice = body["messages"].to_string().contains("watch]");
        let say = |text: &str| {
            sse(&[
                serde_json::json!({"choices":[{"delta":{"content":text}}]}),
                serde_json::json!({"choices":[{"finish_reason":"stop"}]}),
            ])
        };
        match (has_tool_result, has_watch_notice) {
            (false, _) => tool_call(
                "job_start",
                serde_json::json!({"command":"echo FAIL uno","label":"vigilado",
                    "watch":{"pattern":"FAIL","max_events":5}}),
            ),
            (true, true) => say("aviso recibido"),
            (true, false) => say("job lanzado, vigilo la salida"),
        }
    }
}

/// Servidor OpenAI-compatible con el guion de un padre que vigila un job y despierta con su aviso.
pub async fn mock_provider_watch() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(WatchScript)
        .mount(&server)
        .await;
    server
}

/// Texto del primer intento de cierre del guion de ODD (sin herramientas).
pub const ODD_INTENTO_DE_CIERRE: &str = "Documento listo; doy el trabajo por cerrado.";
/// Texto final del guion de ODD, cuando las dos tareas ya están probadas.
pub const ODD_CIERRE_FINAL: &str = "Hecho: las dos tareas están probadas; cierro el turno.";
/// Id de la llamada `bash` del guion de ODD. Las dos marcas de tarea lo reutilizan como evidencia.
pub const ODD_ID_BASH: &str = "call-bash-odd";
/// Respuesta de relleno para las peticiones internas que no ofrecen `odd_track`.
const ODD_NEUTRO: &str = "ok";
/// Pausa del texto final: da margen a la TUI para pintar `(2/2)` antes de que el turno cierre.
const ODD_PAUSA_FINAL: Duration = Duration::from_millis(1500);

/// Llamada de herramienta con un identificador elegido, para que el guion lo pueda leer después.
fn tool_call_with_id(id: &str, name: &str, arguments: serde_json::Value) -> ResponseTemplate {
    sse(&[
        serde_json::json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":id,"function":{"name":name,"arguments":arguments.to_string()}}]}}]}),
        serde_json::json!({"choices":[{"finish_reason":"tool_calls"}]}),
    ])
}

/// Respuesta de texto final en SSE, sin herramientas.
fn say_text(text: &str) -> ResponseTemplate {
    sse(&[
        serde_json::json!({"choices":[{"delta":{"content":text}}]}),
        serde_json::json!({"choices":[{"finish_reason":"stop"}]}),
    ])
}

/// Guion de un padre que planifica con ODD y trata de cerrar el turno con tareas pendientes.
///
/// Decide por el historial que el runtime ya envió, sin contadores, así que no depende de cuántas
/// peticiones haga por dentro:
/// 1. Sin llamadas previas: `odd_track` `create` (feature `demo`, tareas `T1` y `T2`).
/// 2. Tras crear: texto final sin herramientas (intento de cierre). Con ODD encendido, el gate reinyecta.
/// 3. Tras la reinyección: `bash` con `echo ok`, id `ODD_ID_BASH`.
/// 4. Tras ese `bash`: `done` de `T1`, con el `call_id` leído del historial como evidencia.
/// 5. Tras `T1`: `done` de `T2`, con la misma evidencia.
/// 6. Tras `T2`: texto final, con una pausa de `ODD_PAUSA_FINAL`.
///
/// Las peticiones que no ofrecen `odd_track` (las internas del runtime) reciben un texto neutro.
struct OddScript;

impl Respond for OddScript {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = request.body_json().unwrap_or_default();
        let offers_odd = body["tools"].as_array().is_some_and(|tools| {
            tools
                .iter()
                .any(|entry| entry.pointer("/function/name") == Some(&"odd_track".into()))
        });
        if !offers_odd {
            return say_text(ODD_NEUTRO);
        }
        let messages = body["messages"].as_array().cloned().unwrap_or_default();
        // Llamadas del asistente en orden: (id, nombre, argumentos ya decodificados).
        let llamadas: Vec<(String, String, serde_json::Value)> = messages
            .iter()
            .filter(|m| m["role"] == "assistant")
            .flat_map(|m| m["tool_calls"].as_array().cloned().unwrap_or_default())
            .map(|llamada| {
                let nombre = llamada["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let argumentos = llamada["function"]["arguments"].as_str().unwrap_or("{}");
                (
                    llamada["id"].as_str().unwrap_or_default().to_string(),
                    nombre,
                    serde_json::from_str(argumentos).unwrap_or_default(),
                )
            })
            .collect();
        let ultimo_rol = messages
            .last()
            .and_then(|m| m["role"].as_str())
            .unwrap_or_default();
        let ultima = llamadas.last();
        // El id de la llamada `bash` sale del historial: es el que el runtime guardó, no uno fijo.
        let id_bash = llamadas
            .iter()
            .find(|llamada| llamada.1 == "bash")
            .map(|llamada| llamada.0.clone())
            .unwrap_or_default();
        let hechas = llamadas
            .iter()
            .filter(|llamada| llamada.1 == "odd_track" && llamada.2["op"] == "done")
            .count();
        match (ultimo_rol, ultima) {
            ("user", None) => tool_call_with_id(
                "call-odd-create",
                "odd_track",
                serde_json::json!({"op":"create","feature":"demo",
                    "objective":"Demostrar ODD en la TUI",
                    "tasks":["primera tarea de prueba","segunda tarea de prueba"]}),
            ),
            ("user", Some(_)) if id_bash.is_empty() => tool_call_with_id(
                ODD_ID_BASH,
                "bash",
                serde_json::json!({"command":"echo ok"}),
            ),
            ("tool", Some(ultima)) => match (ultima.1.as_str(), ultima.2["op"].as_str()) {
                ("odd_track", Some("create")) => say_text(ODD_INTENTO_DE_CIERRE),
                ("bash", _) => tool_call_with_id(
                    "call-odd-done-1",
                    "odd_track",
                    serde_json::json!({"op":"done","feature":"demo","id":"T1",
                        "evidence":{"text":"echo ok en bash","call_id":id_bash}}),
                ),
                ("odd_track", Some("done")) if hechas == 1 => tool_call_with_id(
                    "call-odd-done-2",
                    "odd_track",
                    serde_json::json!({"op":"done","feature":"demo","id":"T2",
                        "evidence":{"text":"echo ok en bash","call_id":id_bash}}),
                ),
                ("odd_track", Some("done")) => {
                    say_text(ODD_CIERRE_FINAL).set_delay(ODD_PAUSA_FINAL)
                }
                _ => say_text(ODD_NEUTRO),
            },
            _ => say_text(ODD_NEUTRO),
        }
    }
}

/// Servidor OpenAI-compatible con el guion de ODD: crea una feature de dos tareas, intenta cerrar el
/// turno sin probarlas y después marca cada una con la evidencia de un `bash`.
pub async fn mock_provider_odd() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(OddScript)
        .mount(&server)
        .await;
    server
}

/// Texto final del guion de compresión, cuando el modelo ya ha recibido la salida de `bash`.
pub const COMPRESION_FINAL: &str = "Hecho: la salida ruidosa llegó filtrada al modelo.";
/// Id de la llamada `bash` del guion de compresión.
pub const COMPRESION_ID_BASH: &str = "call-bash-ruido";
/// Comando (`cmd`, que es el shell de `bash` en Windows) que imprime 40 líneas idénticas y una final: el filtro
/// general colapsa las repetidas y deja la señal.
pub const COMPRESION_COMANDO: &str = "(for /L %i in (1,1,40) do @echo ruido) & echo FIN";

/// Guion de un padre que ejecuta un `bash` ruidoso y, con su resultado, contesta con texto final.
///
/// Las peticiones que no ofrecen `bash` (las internas del runtime) reciben un texto neutro.
struct RuidoScript;

impl Respond for RuidoScript {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = request.body_json().unwrap_or_default();
        let offers_bash = body["tools"].as_array().is_some_and(|tools| {
            tools
                .iter()
                .any(|entry| entry.pointer("/function/name") == Some(&"bash".into()))
        });
        if !offers_bash {
            return say_text(ODD_NEUTRO);
        }
        let has_tool_result = body["messages"]
            .as_array()
            .is_some_and(|messages| messages.iter().any(|m| m["role"] == "tool"));
        if has_tool_result {
            say_text(COMPRESION_FINAL)
        } else {
            tool_call_with_id(
                COMPRESION_ID_BASH,
                "bash",
                serde_json::json!({"command": COMPRESION_COMANDO}),
            )
        }
    }
}

/// Servidor OpenAI-compatible con el guion de compresión: un `bash` ruidoso y, con su salida, el texto final.
pub async fn mock_provider_compresion() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(RuidoScript)
        .mount(&server)
        .await;
    server
}

/// Servidor OpenAI-compatible con el guion de un hijo que pregunta a la persona y espera su respuesta.
pub async fn mock_provider_child_asks() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ChildAsksScript)
        .mount(&server)
        .await;
    server
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

/// Perfil de usuario y proyecto temporales: nada de esto toca `~/.loomcoding` del usuario.
pub struct Workspace {
    pub home: TempDir,
    pub project: TempDir,
}

impl Workspace {
    pub fn new(base_url: &str) -> io::Result<Workspace> {
        let home = tempfile::tempdir()?;
        let project = tempfile::tempdir()?;
        std::fs::write(
            project.path().join("loomcoding.toml"),
            format!(
                "model = \"mock-model\"\nmax_steps = 3\n\n[provider]\nkind = \"openai_compat\"\n\
                 base_url = \"{base_url}\"\napi_key_env = \"{KEY_ENV}\"\n"
            ),
        )?;
        // Un catálogo reciente evita que el arranque intente descargar models.dev.
        let data = home.path().join(".loomcoding");
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
            ("LOOMCODING_HOME".into(), home),
            (KEY_ENV.into(), "sk-test".into()),
            // El pegado no debe leer el portapapeles real de quien ejecuta las pruebas: una imagen
            // copiada convertiría cada pegado de texto en un adjunto.
            ("LOOMCODING_NO_CLIPBOARD".into(), "1".into()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loomcoding_core::catalog::{is_stale, read_cache};
    use loomcoding_core::config::LoomcodingConfig;
    use std::io::{Read, Write};
    use std::time::SystemTime;

    #[test]
    fn workspace_writes_config_and_catalog() {
        let ws = Workspace::new("http://127.0.0.1:1").unwrap();
        let cfg = LoomcodingConfig::load(&ws.project.path().join("loomcoding.toml")).unwrap();
        assert_eq!(cfg.provider.base_url, "http://127.0.0.1:1");
        assert_eq!(cfg.provider.api_key_env, "LOOMCODING_E2E_KEY");
        assert_eq!(cfg.model, "mock-model");

        let cache = ws.home.path().join(".loomcoding").join("models-dev.json");
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
        assert!(env.contains(&("LOOMCODING_HOME".to_string(), home)));
        assert!(env.contains(&("LOOMCODING_E2E_KEY".to_string(), "sk-test".to_string())));
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
