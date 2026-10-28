use codeloom_core::agent::AgentLoop;
use codeloom_core::config::{CodeLoomConfig, ProviderConfig, ProviderKind};
use codeloom_core::event::EventBus;
use codeloom_core::permission::PermissionService;
use codeloom_core::provider::openai::OpenAiCompatProvider;
use codeloom_core::registry::{AgentRegistry, BuiltInSource, FileSource, ProfileLevel};
use codeloom_core::session::sqlite::SqliteStore;
use codeloom_core::session::SessionStore;
use codeloom_core::tools::subagent::SubagentTool;
use codeloom_core::tools::view::ViewTool;
use codeloom_core::tools::ToolRegistry;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn end_to_end_single_turn() {
    let server = MockServer::start().await;
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\n\
                data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n\
                data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n";
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/proj").await.unwrap();

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ViewTool));

    let cfg = CodeLoomConfig {
        provider: ProviderConfig {
            kind: ProviderKind::OpenAiCompat,
            base_url: server.uri(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 3,
        effort: None,
    };
    let provider = Arc::new(OpenAiCompatProvider::new(server.uri(), "sk-test"));
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );

    let outcome = agent
        .run(sid, "say hi".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(outcome.final_text, "hello world");
    assert_eq!(outcome.steps, 1);
    let msgs = store.messages(sid).await.unwrap();
    assert!(msgs.iter().any(|m| m.text().contains("hello world")));
}

#[tokio::test]
async fn end_to_end_parent_delegates_to_registered_agent() {
    let server = MockServer::start().await;
    let child_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let child_seen_by_mock = child_seen.clone();
    let profile_listed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let profile_listed_by_mock = profile_listed.clone();
    let profile_marker = "E2E_REVIEWER_PROFILE_MARKER";
    let subagent_args = serde_json::json!({
        "prompt": "Review the sample change",
        "label": "review",
        "agent": "reviewer"
    })
    .to_string();
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(move |request: &wiremock::Request| {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let messages = body["messages"].as_array().unwrap();
            let child = messages[0]["content"]
                .as_str()
                .unwrap_or_default()
                .contains(profile_marker);
            if !child
                && messages[0]["content"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("reviewer")
            {
                profile_listed_by_mock.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            let response = if child {
                child_seen_by_mock.store(true, std::sync::atomic::Ordering::SeqCst);
                "data: {\"choices\":[{\"delta\":{\"content\":\"reviewer completed\"}}]}\n\n\
                 data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n"
                    .to_string()
            } else if messages.iter().any(|message| message["role"] == "tool") {
                "data: {\"choices\":[{\"delta\":{\"content\":\"parent completed\"}}]}\n\n\
                 data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n"
                    .to_string()
            } else {
                let call = serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-review","type":"function","function":{"name":"subagent","arguments":subagent_args}}]}}]});
                let done = serde_json::json!({"choices":[{"finish_reason":"tool_calls"}]});
                format!("data: {call}\n\ndata: {done}\n\n")
            };
            ResponseTemplate::new(200).set_body_raw(response, "text/event-stream")
        })
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    std::fs::create_dir(&agents_dir).unwrap();
    std::fs::write(
        agents_dir.join("reviewer.md"),
        format!("---\nname: reviewer\ndescription: E2E reviewer\n---\n{profile_marker}\n"),
    )
    .unwrap();
    let registry = Arc::new(AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(agents_dir, ProfileLevel::Global)),
    ]));
    let store = Arc::new(
        SqliteStore::open(dir.path().join("sessions.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store
        .create_session(dir.path().to_str().unwrap())
        .await
        .unwrap();
    let provider = Arc::new(OpenAiCompatProvider::new(server.uri(), "sk-test"));
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(SubagentTool));
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        EventBus::new(128),
        Arc::new(PermissionService::allow_all()),
        CodeLoomConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: server.uri(),
                api_key_env: "K".into(),
            },
            model: "mock".into(),
            max_steps: 3,
            effort: None,
        },
        dir.path().to_path_buf(),
    )
    .with_agent_registry(registry);

    let outcome = agent
        .run(
            parent,
            "Delegate a review to reviewer".into(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.final_text, "parent completed");

    let child = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if let Some(child) = store
                .list_subagents(parent)
                .await
                .unwrap()
                .into_iter()
                .next()
            {
                if child.status.state.is_terminal() {
                    break child;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("subagent should finish");
    assert_eq!(child.agent.as_deref(), Some("reviewer"));
    assert_eq!(
        child.status.state,
        codeloom_core::task::TaskState::Completed
    );
    assert!(child_seen.load(std::sync::atomic::Ordering::SeqCst));
    assert!(profile_listed.load(std::sync::atomic::Ordering::SeqCst));
    assert!(store
        .messages(child.child)
        .await
        .unwrap()
        .iter()
        .any(|message| message.text().contains("reviewer completed")));
}
