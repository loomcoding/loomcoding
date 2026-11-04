//! `accept`, `cancel` y `status`: las decisiones de la persona y la consulta.

use super::support::*;
use super::*;

async fn hasta_la_aceptacion(env: &Entorno) {
    env.hasta_el_juicio().await;
    let informe = env.no_gap().await;
    env.finish_and_advance(&informe).await;
    let informe = env.no_gap().await;
    env.finish_and_advance(&informe).await;
    assert_eq!(
        env.active().await.phase,
        GauntletPhase::AwaitingFinalAcceptance
    );
}

#[tokio::test]
async fn an_explicit_yes_completes_the_gauntlet() {
    let env = Entorno::nuevo().await;
    hasta_la_aceptacion(&env).await;
    let out = env
        .run_as(true, serde_json::json!({"action": "accept"}))
        .await
        .unwrap();
    assert!(!out.is_error, "{}", out.content);
    assert_eq!(env.latest().await.phase, GauntletPhase::Completed);
}

#[tokio::test]
async fn anything_but_an_explicit_yes_cancels_instead_of_accepting() {
    let env = Entorno::nuevo().await;
    hasta_la_aceptacion(&env).await;
    env.run_as(false, serde_json::json!({"action": "accept"}))
        .await
        .unwrap();
    assert_eq!(env.latest().await.phase, GauntletPhase::Cancelled);
}

#[tokio::test]
async fn silence_does_not_accept() {
    struct Mudo;
    #[async_trait]
    impl PermissionResponder for Mudo {
        async fn decide(&self, _: &PermissionRequest) -> Decision {
            Decision::Allow
        }
        async fn ask(&self, _: &[crate::question::Question]) -> Option<Vec<Answer>> {
            None
        }
    }
    let env = Entorno::nuevo().await;
    hasta_la_aceptacion(&env).await;
    let mut ctx = env.ctx(true);
    ctx.permission = Arc::new(crate::permission::PermissionService::with_responder(
        Box::new(Mudo),
    ));
    GauntletTool
        .run(
            ctx,
            serde_json::json!({"action": "accept"}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(env.latest().await.phase, GauntletPhase::Cancelled);
}

#[tokio::test]
async fn accept_is_refused_if_the_artifact_changed_after_it_was_presented() {
    let env = Entorno::nuevo().await;
    hasta_la_aceptacion(&env).await;
    std::fs::write(
        env.project.path().join("out/portada.md"),
        "cambiado a escondidas",
    )
    .unwrap();
    let out = env
        .run_as(true, serde_json::json!({"action": "accept"}))
        .await;
    assert!(out.is_err() || out.unwrap().is_error);
    assert_eq!(
        env.active().await.phase,
        GauntletPhase::AwaitingFinalAcceptance
    );
}

#[tokio::test]
async fn accept_before_the_verdict_is_refused() {
    let env = Entorno::nuevo().await;
    env.start().await;
    assert!(env
        .run(serde_json::json!({"action": "accept"}))
        .await
        .is_err());
    assert_eq!(env.active().await.phase, GauntletPhase::Building);
}

#[tokio::test]
async fn cancel_ends_the_gauntlet_and_asks_the_running_child_to_stop() {
    let env = Entorno::nuevo().await;
    env.start().await;
    let child = env.active().await.active_child.unwrap();
    let out = env
        .run(serde_json::json!({"action": "cancel"}))
        .await
        .unwrap();
    assert!(!out.is_error, "{}", out.content);
    assert_eq!(env.latest().await.phase, GauntletPhase::Cancelled);
    assert_eq!(env.launcher.cancelled.lock().unwrap().as_slice(), [child]);
}

#[tokio::test]
async fn actions_other_than_start_and_status_need_an_active_gauntlet() {
    let env = Entorno::nuevo().await;
    for action in ["advance", "accept", "cancel"] {
        assert!(
            env.run(serde_json::json!({"action": action}))
                .await
                .is_err(),
            "{action}"
        );
    }
    assert!(env
        .run(serde_json::json!({"action": "bogus"}))
        .await
        .is_err());
}

#[tokio::test]
async fn status_reports_phase_round_and_child_without_changing_anything() {
    let env = Entorno::nuevo().await;
    env.start().await;
    let before = env.active().await;
    let out = env
        .run(serde_json::json!({"action": "status"}))
        .await
        .unwrap();
    assert!(!out.is_error);
    let json: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert_eq!(json["gauntlet"]["phase"], "building");
    assert_eq!(json["gauntlet"]["round"], 1);
    assert!(json["child_state"].is_string());
    assert!(json["next_action"].as_str().unwrap().contains("wait"));
    assert!(
        !out.content.contains("pulir la portada"),
        "el resumen no copia el objetivo"
    );
    assert_eq!(env.active().await, before);
    let child = before.active_child.unwrap();
    env.launcher.complete(child, "hecho");
    let out = env
        .run(serde_json::json!({"action": "status"}))
        .await
        .unwrap();
    assert!(out.content.contains("advance"));
}

#[tokio::test]
async fn status_without_any_gauntlet_is_an_error() {
    let env = Entorno::nuevo().await;
    assert!(env
        .run(serde_json::json!({"action": "status"}))
        .await
        .is_err());
}
