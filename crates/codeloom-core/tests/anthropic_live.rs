//! Prueba real contra Anthropic a través de OpenRouter. Ignorada por defecto: gasta unos
//! pocos tokens y exige `OPENROUTER_API_KEY`. Se lanza con:
//! `cargo test -p codeloom-core --test anthropic_live -- --ignored --nocapture`
//!
//! Cabe señalar que reproduce el reenvío del turno con `tool_use` bajo `thinking`: con el
//! estado de continuación debe aceptarse; sin él, la API lo rechaza.

use codeloom_core::message::{ContentPart, Message, Role};
use codeloom_core::provider::anthropic::AnthropicProvider;
use codeloom_core::provider::openai::OpenAiCompatProvider;
use codeloom_core::provider::{Provider, ProviderRequest, StreamChunk, ToolSpec};
use futures::StreamExt;
use tokio_util::sync::CancellationToken;

/// Modelo por defecto; `CODELOOM_LIVE_MODEL` permite probar otro. Cabe señalar que con
/// Haiku 5.5 vía OpenRouter el adaptador no activa `thinking`, así que no ejercita el caso.
fn model() -> String {
    std::env::var("CODELOOM_LIVE_MODEL").unwrap_or_else(|_| "anthropic/claude-haiku-5.5".into())
}

/// `CODELOOM_LIVE_KIND=openai_compat` prueba la ruta que usa un `codeloom.toml` con
/// `kind = "openai_compat"` contra OpenRouter; por defecto, el adaptador de Anthropic.
fn provider(key: String) -> Box<dyn Provider> {
    match std::env::var("CODELOOM_LIVE_KIND").as_deref() {
        Ok("openai_compat") => Box::new(OpenAiCompatProvider::new(
            "https://openrouter.ai/api/v1",
            key,
        )),
        _ => Box::new(AnthropicProvider::new("https://openrouter.ai/api", key)),
    }
}

fn request(messages: Vec<Message>) -> ProviderRequest {
    ProviderRequest {
        model: model(),
        system: "Eres conciso. Usa la herramienta `view` cuando se te pida leer un fichero.".into(),
        messages,
        tools: vec![ToolSpec {
            name: "view".into(),
            description: "Lee un fichero".into(),
            input_schema: serde_json::json!({
                "type":"object",
                "properties":{"path":{"type":"string"}},
                "required":["path"]
            }),
            programmatic_allowed: false,
        }],
        temperature: 0.0,
        max_tokens: 512,
        effort: Some("low".into()),
        programmatic_tool_calling: false,
    }
}

struct Turn {
    assistant: Message,
    call_id: Option<String>,
    error: Option<String>,
}

fn failed(error: String) -> Turn {
    Turn {
        assistant: Message::assistant(vec![]),
        call_id: None,
        error: Some(error),
    }
}

async fn run(provider: &dyn Provider, messages: Vec<Message>) -> Turn {
    let mut parts = Vec::new();
    let mut state = None;
    let mut call_id = None;
    let mut text = String::new();
    let mut stream = match provider
        .stream(request(messages), CancellationToken::new())
        .await
    {
        Ok(stream) => stream,
        Err(error) => return failed(error.to_string()),
    };
    while let Some(chunk) = stream.next().await {
        match chunk {
            StreamChunk::TextDelta(delta) => text.push_str(&delta),
            StreamChunk::ToolCallComplete { id, name, input } => {
                call_id = Some(id.clone());
                parts.push(ContentPart::ToolCall { id, name, input });
            }
            StreamChunk::ProviderState(s) => state = Some(s),
            _ => {}
        }
    }
    if !text.is_empty() {
        parts.insert(0, ContentPart::Text { text });
    }
    let mut assistant = Message::assistant(parts);
    assistant.provider_state = state;
    Turn {
        assistant,
        call_id,
        error: None,
    }
}

fn tool_result(call_id: &str) -> Message {
    Message::new(
        Role::Tool,
        vec![ContentPart::ToolResult {
            call_id: call_id.into(),
            content: "name = \"codeloom\"".into(),
            is_error: false,
        }],
    )
}

#[tokio::test]
#[ignore = "llamada real: exige OPENROUTER_API_KEY y gasta tokens"]
async fn tool_use_turn_can_be_replayed_with_thinking_enabled() {
    let key = std::env::var("OPENROUTER_API_KEY").expect("falta OPENROUTER_API_KEY");
    let provider = provider(key);
    let ask =
        Message::user("Lee el fichero Cargo.toml con la herramienta view y dime su contenido.");

    let first = run(&*provider, vec![ask.clone()]).await;
    assert_eq!(first.error, None, "falló ya el primer turno");
    let call_id = first
        .call_id
        .clone()
        .expect("el modelo no llamó a la herramienta");
    println!(
        "estado de continuación guardado: {}",
        first.assistant.provider_state.is_some()
    );

    // Con el estado (lo que hace ahora el adaptador).
    let with_state = run(
        &*provider,
        vec![ask.clone(), first.assistant.clone(), tool_result(&call_id)],
    )
    .await;
    println!("CON estado -> {:?}", with_state.error);

    // Sin el estado (lo que ocurría antes del arreglo).
    let mut bare = first.assistant.clone();
    bare.provider_state = None;
    let without_state = run(&*provider, vec![ask, bare, tool_result(&call_id)]).await;
    println!("SIN estado -> {:?}", without_state.error);

    assert_eq!(
        with_state.error, None,
        "el reenvío con estado debía aceptarse"
    );
}
