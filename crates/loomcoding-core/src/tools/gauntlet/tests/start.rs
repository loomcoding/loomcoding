//! `start`: validaciones y lanzamiento del constructor.

use super::support::*;
use super::*;

#[tokio::test]
async fn start_launches_the_builder_with_write_access_and_records_building() {
    let env = Entorno::nuevo().await;
    env.start().await;
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::Building);
    assert_eq!(g.round, 1);
    assert_eq!(g.max_rounds, 2);
    let launch = env.launcher.last();
    assert_eq!(launch.agent, "constructor");
    assert!(!launch.read_only);
    assert_eq!(g.active_child, Some(launch.child));
    assert_eq!(g.builder_children, [launch.child]);
    assert!(launch.prompt.contains("pulir la portada"));
    assert!(launch.prompt.contains("out/portada.md"));
    assert!(launch.prompt.contains("ref/portada.md"));
}

#[tokio::test]
async fn start_rejects_the_same_agent_twice() {
    let env = Entorno::nuevo().await;
    let mut input = Entorno::start_input();
    input["critic"] = "constructor".into();
    let error = env.run(input).await.unwrap_err();
    assert!(error.to_string().contains("distintos"), "{error}");
    assert_eq!(env.launcher.count(), 0);
    assert!(env
        .store
        .latest_gauntlet(env.session)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn start_rejects_missing_outside_and_non_text_paths() {
    let env = Entorno::nuevo().await;
    std::fs::write(
        env.project.path().join("ref/foto.bin"),
        [0xff, 0xfe, 0x00, 0x9f],
    )
    .unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("fuera.md"), "x").unwrap();
    for (campo, valor) in [
        ("bar", serde_json::json!(["ref/no-existe.md"])),
        ("bar", serde_json::json!(["ref/foto.bin"])),
        ("bar", serde_json::json!([outside.path().join("fuera.md")])),
        ("bar", serde_json::json!(["../fuera.md"])),
        ("bar", serde_json::json!([])),
        ("artifact", serde_json::json!("out/no-existe.md")),
    ] {
        let mut input = Entorno::start_input();
        input[campo] = valor.clone();
        assert!(
            env.run(input).await.is_err(),
            "{campo} = {valor} debía rechazarse"
        );
    }
    assert_eq!(env.launcher.count(), 0);
}

#[tokio::test]
async fn start_rejects_rounds_out_of_range_and_accepts_an_explicit_one() {
    let env = Entorno::nuevo().await;
    for fuera in [0, 6] {
        let mut input = Entorno::start_input();
        input["max_rounds"] = fuera.into();
        assert!(env.run(input).await.is_err(), "max_rounds {fuera}");
    }
    let mut input = Entorno::start_input();
    input["max_rounds"] = 3.into();
    assert!(!env.run(input).await.unwrap().is_error);
    assert_eq!(env.active().await.max_rounds, 3);
}

#[tokio::test]
async fn a_second_start_is_rejected_while_one_is_active() {
    let env = Entorno::nuevo().await;
    env.start().await;
    assert!(env.run(Entorno::start_input()).await.is_err());
    assert_eq!(env.launcher.count(), 1);
}

#[tokio::test]
async fn a_failed_launch_leaves_the_gauntlet_failed_not_building() {
    struct Roto;
    #[async_trait]
    impl SubagentLauncher for Roto {
        async fn start(
            &self,
            _: SessionId,
            _: String,
            _: String,
            _: Option<String>,
            _: CancellationToken,
        ) -> Result<(SessionId, JobId)> {
            Err(CoreError::Tool("sin cupo".into()))
        }
        async fn start_local_workflow_agent(
            &self,
            _: SessionId,
            _: String,
            _: String,
            _: String,
            _: CancellationToken,
        ) -> Result<(SessionId, JobId)> {
            Err(CoreError::Tool("sin cupo".into()))
        }
        async fn control(
            &self,
            _: SessionId,
            _: SessionId,
            _: &str,
            _: Option<String>,
        ) -> Result<()> {
            Ok(())
        }
        async fn list(&self, _: SessionId) -> Result<Vec<SubagentDescriptor>> {
            Ok(Vec::new())
        }
        async fn deliver_notices(&self, _: SessionId) -> Result<Vec<crate::message::Message>> {
            Ok(Vec::new())
        }
    }
    let env = Entorno::nuevo().await;
    let mut ctx = env.ctx(true);
    ctx.subagents = Some(Arc::new(Roto));
    let result = GauntletTool
        .run(ctx, Entorno::start_input(), CancellationToken::new())
        .await;
    assert!(result.is_err());
    assert_eq!(env.latest().await.phase, GauntletPhase::Failed);
    assert!(env
        .store
        .active_gauntlet(env.session)
        .await
        .unwrap()
        .is_none());
}
