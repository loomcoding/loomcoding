//! Observabilidad: cada cambio de fase llega al bus con su actor y su detalle.

use super::support::*;
use super::*;
use crate::event::LiveEvent;

fn recoger(
    rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
) -> Vec<(GauntletPhase, u8, String, String)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let LiveEvent::GauntletChanged {
            summary,
            actor,
            detail,
        } = event
        {
            out.push((summary.phase, summary.round, actor, detail));
        }
    }
    out
}

#[tokio::test]
async fn start_publishes_the_builder_launch_with_its_actor() {
    let env = Entorno::nuevo().await;
    let mut rx = env.bus.subscribe();
    env.start().await;
    let events = recoger(&mut rx);
    let (phase, round, actor, detail) = events.last().unwrap();
    assert_eq!(*phase, GauntletPhase::Building);
    assert_eq!(*round, 1);
    assert_eq!(actor, "coordinator");
    assert_eq!(detail, "builder launched");
    assert!(events.len() >= 2, "alta y lanzamiento: {events:?}");
}

#[tokio::test]
async fn the_critic_verdict_is_published_with_the_gap_text() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let mut rx = env.bus.subscribe();
    let informe = env.gap().await;
    env.finish_and_advance(&informe).await;
    let events = recoger(&mut rx);
    let veredicto = events
        .iter()
        .find(|(_, _, actor, _)| actor == "critic")
        .expect("el veredicto debe publicarse");
    assert_eq!(veredicto.0, GauntletPhase::Rebuilding);
    assert!(veredicto.3.starts_with("gap: "), "{veredicto:?}");
    assert!(veredicto.3.contains("los márgenes están descuadrados"));
    assert_eq!(events.last().unwrap().3, "builder launched");
}

#[tokio::test]
async fn the_persons_decisions_are_published_as_the_user() {
    let env = Entorno::nuevo().await;
    env.start().await;
    let mut rx = env.bus.subscribe();
    env.run(serde_json::json!({"action": "cancel"}))
        .await
        .unwrap();
    let events = recoger(&mut rx);
    assert_eq!(
        events.last().unwrap(),
        &(
            GauntletPhase::Cancelled,
            1,
            "user".to_string(),
            "cancelled".to_string()
        )
    );
}

#[tokio::test]
async fn a_failed_launch_is_published_too() {
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
    let mut rx = env.bus.subscribe();
    let mut ctx = env.ctx(true);
    ctx.subagents = Some(Arc::new(Roto));
    let _ = GauntletTool
        .run(ctx, Entorno::start_input(), CancellationToken::new())
        .await;
    let events = recoger(&mut rx);
    let (phase, _, actor, detail) = events.last().unwrap();
    assert_eq!(*phase, GauntletPhase::Failed);
    assert_eq!(actor, "coordinator");
    assert!(detail.starts_with("launch failed"), "{detail}");
}
