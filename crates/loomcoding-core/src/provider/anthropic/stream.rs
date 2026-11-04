//! Acumulador de la respuesta en streaming y mapeo de los eventos SSE de Anthropic a `StreamChunk`.

use crate::message::FinishReason;
use crate::provider::StreamChunk;

/// Tope del estado de continuación; el agente rechaza los que superen 64 KiB, así que
/// uno mayor se descarta aquí en lugar de abortar el turno.
pub(super) const MAX_CONTINUATION_STATE_BYTES: usize = 60 * 1024;

#[derive(Default)]
pub struct AnthropicAcc {
    pub(super) blocks: Vec<(String, String, String)>, // (id, name, json) por index
    /// Bloques de contenido tal como los emitió el modelo, por índice.
    pub(super) content: Vec<serde_json::Value>,
    pub(super) has_thinking: bool,
    /// Modelo y `base_url` con los que se firma el estado de continuación.
    pub(super) identity: Option<(String, String)>,
}

impl AnthropicAcc {
    /// Acumulador que, si la respuesta trae razonamiento, emite su estado de continuación.
    pub fn for_provider(model: &str, base_url: &str) -> Self {
        Self {
            identity: Some((model.to_string(), base_url.to_string())),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    fn content_slot(&mut self, idx: usize) -> &mut serde_json::Value {
        while self.content.len() <= idx {
            self.content.push(serde_json::Value::Null);
        }
        &mut self.content[idx]
    }

    pub fn finish(&mut self) -> Vec<StreamChunk> {
        let state = self.continuation_state();
        let mut out: Vec<StreamChunk> = std::mem::take(&mut self.blocks)
            .into_iter()
            .filter(|(_, name, _)| !name.is_empty())
            .map(|(id, name, json)| StreamChunk::ToolCallComplete {
                id,
                name,
                input: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            })
            .collect();
        out.extend(state.map(StreamChunk::ProviderState));
        out
    }

    /// Cabe señalar que con `thinking` activo la API exige reenviar, junto a cada `tool_use`,
    /// el bloque de razonamiento con su firma; sin él responde 400. Se conserva solo si lo hubo.
    fn continuation_state(&mut self) -> Option<crate::message::ProviderState> {
        let (model, base_url) = self.identity.clone().filter(|_| self.has_thinking)?;
        let mut content = Vec::new();
        for (idx, block) in std::mem::take(&mut self.content).into_iter().enumerate() {
            let mut block = block;
            if block["type"] == "tool_use" {
                let json = self
                    .blocks
                    .get(idx)
                    .map_or("", |(_, _, json)| json.as_str());
                block["input"] =
                    serde_json::from_str(json).unwrap_or_else(|_| serde_json::json!({}));
            }
            if !block.is_null() {
                content.push(block);
            }
        }
        let state = crate::message::ProviderState {
            provider: "anthropic".into(),
            data: serde_json::json!({
                "model": model,
                "base_url": base_url,
                "assistant_content": content,
            }),
        };
        let size = serde_json::to_vec(&state).map_or(usize::MAX, |bytes| bytes.len());
        (size <= MAX_CONTINUATION_STATE_BYTES).then_some(state)
    }
}

/// Añade `extra` al campo de texto `key` de un bloque, si el bloque ya existe.
pub(super) fn append_str(block: &mut serde_json::Value, key: &str, extra: &str) {
    if let Some(serde_json::Value::String(current)) = block.get_mut(key) {
        current.push_str(extra);
    }
}

/// Tokens de caché que comunica un objeto `usage` de Anthropic, o `None` si no informa ninguno.
pub(super) fn cache_usage(usage: &serde_json::Value) -> Option<crate::provider::CacheUsage> {
    let read = usage["cache_read_input_tokens"].as_u64();
    let write = usage["cache_creation_input_tokens"].as_u64();
    if read.is_none() && write.is_none() {
        return None;
    }
    Some(crate::provider::CacheUsage {
        read_tokens: read.unwrap_or(0),
        write_tokens: write.unwrap_or(0),
    })
}

pub fn map_anthropic_event(event: &str, data: &str, acc: &mut AnthropicAcc) -> Vec<StreamChunk> {
    let mut out = Vec::new();
    let v: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return out,
    };
    match event {
        "content_block_start" => {
            let idx = v["index"].as_u64().unwrap_or(0) as usize;
            while acc.blocks.len() <= idx {
                acc.blocks
                    .push((String::new(), String::new(), String::new()));
            }
            let cb = &v["content_block"];
            match cb["type"].as_str().unwrap_or("") {
                "text" => *acc.content_slot(idx) = serde_json::json!({"type":"text","text":""}),
                "thinking" => {
                    acc.has_thinking = true;
                    *acc.content_slot(idx) =
                        serde_json::json!({"type":"thinking","thinking":"","signature":""});
                }
                "redacted_thinking" => {
                    acc.has_thinking = true;
                    *acc.content_slot(idx) = cb.clone();
                }
                "tool_use" => {
                    *acc.content_slot(idx) = serde_json::json!({
                        "type":"tool_use","id":cb["id"],"name":cb["name"],"input":{}
                    });
                }
                _ => {}
            }
            if cb["type"] == "tool_use" {
                acc.blocks[idx].0 = cb["id"].as_str().unwrap_or_default().to_string();
                acc.blocks[idx].1 = cb["name"].as_str().unwrap_or_default().to_string();
            }
        }
        "content_block_delta" => {
            let idx = v["index"].as_u64().unwrap_or(0) as usize;
            let d = &v["delta"];
            match d["type"].as_str().unwrap_or("") {
                "text_delta" => {
                    let text = d["text"].as_str().unwrap_or_default();
                    append_str(acc.content_slot(idx), "text", text);
                    out.push(StreamChunk::TextDelta(text.to_string()));
                }
                "thinking_delta" => {
                    let thinking = d["thinking"].as_str().unwrap_or_default();
                    append_str(acc.content_slot(idx), "thinking", thinking);
                    out.push(StreamChunk::ReasoningDelta(thinking.to_string()));
                }
                "signature_delta" => append_str(
                    acc.content_slot(idx),
                    "signature",
                    d["signature"].as_str().unwrap_or_default(),
                ),
                "input_json_delta" => {
                    while acc.blocks.len() <= idx {
                        acc.blocks
                            .push((String::new(), String::new(), String::new()));
                    }
                    acc.blocks[idx]
                        .2
                        .push_str(d["partial_json"].as_str().unwrap_or_default());
                }
                _ => {}
            }
        }
        "message_start" => {
            if let Some(u) = v.pointer("/message/usage") {
                out.push(StreamChunk::UsageUpdate(crate::provider::UsageUpdate {
                    input_tokens: u["input_tokens"].as_u64().map(|value| value as u32),
                    output_tokens: u["output_tokens"].as_u64().map(|value| value as u32),
                    reported_cost_usd: u["cost"]
                        .as_f64()
                        .filter(|cost| cost.is_finite() && *cost >= 0.0),
                    final_update: false,
                }));
                out.extend(cache_usage(u).map(StreamChunk::CacheUsage));
            }
        }
        "message_delta" => {
            if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                out.push(StreamChunk::UsageUpdate(crate::provider::UsageUpdate {
                    input_tokens: None,
                    output_tokens: u["output_tokens"].as_u64().map(|value| value as u32),
                    reported_cost_usd: u["cost"]
                        .as_f64()
                        .filter(|cost| cost.is_finite() && *cost >= 0.0),
                    final_update: true,
                }));
                out.extend(cache_usage(u).map(StreamChunk::CacheUsage));
            }
            out.extend(acc.finish());
            let reason = match v
                .pointer("/delta/stop_reason")
                .and_then(|x| x.as_str())
                .unwrap_or("")
            {
                "tool_use" => FinishReason::ToolCalls,
                "max_tokens" => FinishReason::Length,
                _ => FinishReason::Stop,
            };
            out.push(StreamChunk::Done(reason));
        }
        _ => {}
    }
    out
}
