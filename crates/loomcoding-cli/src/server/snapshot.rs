//! Instantáneas hacia el cliente: lista de sesiones, sesión cargada y recorte de valores del cable.

use super::*;

pub(super) async fn send_session_list(entry: &AgentEntry) {
    match entry.store.list_sessions(&entry.project_key).await {
        Ok(items) => {
            if items.is_empty() {
                entry
                    .ui
                    .send(ServerEvent::SessionListPage { items, done: true })
                    .await;
            } else {
                let count = items.len();
                for (index, page) in items.chunks(20).enumerate() {
                    entry
                        .ui
                        .send(ServerEvent::SessionListPage {
                            items: page.to_vec(),
                            done: (index + 1) * 20 >= count,
                        })
                        .await;
                }
            }
        }
        Err(error) => {
            entry
                .ui
                .send(ServerEvent::Error {
                    message: error.to_string(),
                })
                .await;
        }
    }
}

pub(super) async fn send_loaded_session(entry: &AgentEntry, id: SessionId) -> anyhow::Result<()> {
    if !entry
        .store
        .session_belongs_to_project(id, &entry.project_key)
        .await?
    {
        anyhow::bail!("la sesión no pertenece a este agente");
    }
    let messages = entry.store.messages(id).await?;
    let usage = entry.store.session_usage(id).await?;
    let subagents = entry.store.list_subagents(id).await?;
    let workflow = entry
        .store
        .latest_development_workflow(id)
        .await?
        .map(|w| w.summary());
    let total = messages.len() + usage.len().div_ceil(10) + subagents.len();
    let mut sent = 0usize;
    for message in messages {
        let message = bounded_wire(message);
        sent += 1;
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: vec![message],
                usage: Vec::new(),
                subagents: Vec::new(),
                done: sent == total,
            })
            .await;
    }
    for page in usage.chunks(10) {
        sent += 1;
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: Vec::new(),
                usage: page.to_vec(),
                subagents: Vec::new(),
                done: sent == total,
            })
            .await;
    }
    for item in subagents {
        sent += 1;
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: Vec::new(),
                usage: Vec::new(),
                subagents: vec![bounded_wire(item)],
                done: sent == total,
            })
            .await;
    }
    if total == 0 {
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: Vec::new(),
                usage: Vec::new(),
                subagents: Vec::new(),
                done: true,
            })
            .await;
    }
    entry
        .ui
        .send(ServerEvent::Session(
            loomcoding_tui::run::SessionUpdate::Workflow(id, workflow),
        ))
        .await;
    Ok(())
}

pub(super) fn bounded_wire<T: Serialize + DeserializeOwned>(value: T) -> T {
    let Ok(mut json) = serde_json::to_value(&value) else {
        return value;
    };
    for (max_text, max_items) in [(4096, 16), (2048, 8), (1024, 4), (512, 2), (128, 1)] {
        trim_wire_value(&mut json, 16, max_text, max_items);
        if serde_json::to_vec(&json).is_ok_and(|bytes| bytes.len() < ipc::MAX_FRAME_BYTES - 1024) {
            return serde_json::from_value(json).unwrap_or(value);
        }
    }
    value
}

pub(super) fn trim_wire_value(
    value: &mut serde_json::Value,
    depth: usize,
    max_text: usize,
    max_items: usize,
) {
    if depth == 0 {
        *value = serde_json::Value::Null;
        return;
    }
    match value {
        serde_json::Value::String(text) => {
            if text.len() > max_text {
                *text = loomcoding_core::tools::limit_bytes(text, max_text).into_owned();
            }
        }
        serde_json::Value::Array(items) => {
            items.truncate(max_items);
            for item in items {
                trim_wire_value(item, depth - 1, max_text, max_items);
            }
        }
        serde_json::Value::Object(items) => {
            let old = std::mem::take(items);
            let mut bounded = serde_json::Map::new();
            for (key, mut item) in old.into_iter().take(max_items) {
                let key = loomcoding_core::tools::limit_bytes(&key, 128).into_owned();
                trim_wire_value(&mut item, depth - 1, max_text, max_items);
                bounded.entry(key).or_insert(item);
            }
            *items = bounded;
        }
        _ => {}
    }
}
