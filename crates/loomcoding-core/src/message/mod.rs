pub mod codec;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Who authored a user-role message. Model role and author are separate because internal
/// prompts still need the provider-facing `user` role. `Job` lleva el id del job que emite el
/// aviso; solo lo ven la persistencia y la TUI, nunca el proveedor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessageOrigin {
    Human,
    #[default]
    Internal,
    Job(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub reported_cost_usd: Option<f64>,
}

/// Origen del conteo guardado en el ledger. Una estimación nunca equivale a un dato del provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    #[default]
    Provider,
    Estimate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
        signature: Option<String>,
    },
    ImageUrl {
        url: String,
    },
    Image {
        mime_type: String,
        data: String,
        filename: String,
    },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        content: String,
        is_error: bool,
    },
    Finish {
        reason: FinishReason,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: u64,
    pub role: Role,
    /// Defaults to internal so older persisted messages cannot be falsely attributed to a person.
    #[serde(default)]
    pub origin: MessageOrigin,
    pub parts: Vec<ContentPart>,
    #[serde(default)]
    pub usage: Option<Usage>,
    /// Opaque provider continuation data kept only in memory for the active turn.
    #[serde(skip)]
    pub provider_state: Option<ProviderState>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderState {
    pub provider: String,
    pub data: serde_json::Value,
}

impl Message {
    pub fn new(role: Role, parts: Vec<ContentPart>) -> Self {
        Self {
            id: 0,
            role,
            origin: MessageOrigin::Internal,
            parts,
            usage: None,
            provider_state: None,
            created_at_ms: now_ms(),
        }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self::new(Role::User, vec![ContentPart::Text { text: text.into() }])
    }
    pub fn human(text: impl Into<String>) -> Self {
        let mut message = Self::user(text);
        message.origin = MessageOrigin::Human;
        message
    }
    /// Aviso de un job (vigilancia o final), con el id del job como origen.
    pub fn from_job(job_id: impl Into<String>, text: impl Into<String>) -> Self {
        let mut message = Self::user(text);
        message.origin = MessageOrigin::Job(job_id.into());
        message
    }
    pub fn assistant(parts: Vec<ContentPart>) -> Self {
        Self::new(Role::Assistant, parts)
    }
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
    pub fn tool_calls(&self) -> Vec<(&str, &str, &serde_json::Value)> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                ContentPart::ToolCall { id, name, input } => {
                    Some((id.as_str(), name.as_str(), input))
                }
                _ => None,
            })
            .collect()
    }
}

pub(crate) fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_collects_only_text_parts() {
        let m = Message::assistant(vec![
            ContentPart::Reasoning {
                text: "hmm".into(),
                signature: None,
            },
            ContentPart::Text {
                text: "hello ".into(),
            },
            ContentPart::Text {
                text: "world".into(),
            },
        ]);
        assert_eq!(m.text(), "hello world");
    }

    #[test]
    fn tool_calls_exposed() {
        let m = Message::assistant(vec![ContentPart::ToolCall {
            id: "c1".into(),
            name: "view".into(),
            input: json!({"path":"a.rs"}),
        }]);
        let calls = m.tool_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "c1");
        assert_eq!(calls[0].1, "view");
    }

    #[test]
    fn provider_continuation_state_is_not_serialized() {
        let mut message = Message::assistant(vec![]);
        message.provider_state = Some(ProviderState {
            provider: "openai_responses".into(),
            data: json!({ "program": "private generated source" }),
        });
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(!encoded.contains("private generated source"));
        assert!(serde_json::from_str::<Message>(&encoded)
            .unwrap()
            .provider_state
            .is_none());
    }

    #[test]
    fn origin_round_trips_and_legacy_messages_fail_closed() {
        let human = Message::human("prompt enviado");
        let encoded = serde_json::to_string(&human).unwrap();
        let restored: Message = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.origin, MessageOrigin::Human);

        let mut legacy = serde_json::to_value(Message::user("histórico")).unwrap();
        legacy.as_object_mut().unwrap().remove("origin");
        let restored: Message = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored.origin, MessageOrigin::Internal);
    }

    #[test]
    fn job_origin_round_trips_with_its_job_id() {
        let aviso = Message::from_job("3", "[Job 3 watch] FAIL uno");
        let encoded = serde_json::to_string(&aviso).unwrap();
        let restored: Message = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.origin, MessageOrigin::Job("3".into()));
        assert_eq!(restored.role, Role::User);
        assert_eq!(restored.text(), "[Job 3 watch] FAIL uno");
    }

    #[test]
    fn stored_origins_read_back_as_before() {
        for (etiqueta, esperado) in [
            ("internal", MessageOrigin::Internal),
            ("human", MessageOrigin::Human),
        ] {
            let mut valor = serde_json::to_value(Message::user("histórico")).unwrap();
            valor["origin"] = json!(etiqueta);
            let restored: Message = serde_json::from_value(valor).unwrap();
            assert_eq!(restored.origin, esperado, "origen {etiqueta}");
        }
    }
}
