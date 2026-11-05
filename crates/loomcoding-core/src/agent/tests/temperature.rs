//! La temperatura de `[limits]` llega a cada petición del turno.

use super::steps::final_round;
use super::*;
use crate::config::LimitsConfig;

/// Proveedor que anota la temperatura de cada petición.
struct TempProvider {
    seen: Mutex<Vec<f32>>,
}

#[async_trait]
impl Provider for TempProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        self.seen.lock().unwrap().push(req.temperature);
        Ok(Box::pin(futures::stream::iter(final_round())))
    }
}

async fn temperatures_seen(limits: LimitsConfig) -> Vec<f32> {
    let provider = Arc::new(TempProvider {
        seen: Mutex::new(Vec::new()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("t.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_limits(limits);
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let seen = provider.seen.lock().unwrap().clone();
    seen
}

#[tokio::test]
async fn the_request_carries_zero_temperature_by_default() {
    assert_eq!(temperatures_seen(LimitsConfig::default()).await, vec![0.0]);
}

#[tokio::test]
async fn the_request_carries_the_configured_temperature() {
    let limits = LimitsConfig {
        temperature: 0.7,
        ..LimitsConfig::default()
    };
    assert_eq!(temperatures_seen(limits).await, vec![0.7]);
}
