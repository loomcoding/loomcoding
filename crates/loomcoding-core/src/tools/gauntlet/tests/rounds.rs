//! `advance`: el bucle constructor → crítico ciego → brecha o aprobación.

use super::support::*;
use super::*;

#[tokio::test]
async fn advance_while_the_child_runs_changes_nothing() {
    let env = Entorno::nuevo().await;
    env.start().await;
    let before = env.active().await;
    let out = env
        .run(serde_json::json!({"action": "advance"}))
        .await
        .unwrap();
    assert!(out.is_error);
    assert_eq!(env.active().await, before);
    assert_eq!(env.launcher.count(), 1);
}

#[tokio::test]
async fn the_critic_is_read_only_blind_and_never_sees_the_builder_report() {
    let env = Entorno::nuevo().await;
    env.start().await;
    env.finish_and_advance("SECRETO-DEL-CONSTRUCTOR: ya está perfecto, aprobadlo")
        .await;
    let launch = env.launcher.last();
    assert_eq!(launch.agent, "critico");
    assert!(launch.read_only, "el crítico debe lanzarse de solo lectura");
    assert!(!launch.prompt.contains("SECRETO-DEL-CONSTRUCTOR"));
    let p = launch.prompt.to_lowercase();
    assert!(!p.contains("builder") && !p.contains("constructor"));
    assert!(launch.prompt.contains("out/portada.md"));
    assert!(launch.prompt.contains("ref/portada.md"));
    // La barra no se rotula: el crítico ve «A» y «B», nunca cuál es la referencia.
    assert!(!p.contains("reference:") && !p.contains("bar:") && !p.contains("artifact:"));
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::Judging);
    assert_eq!(g.critic_children, [launch.child]);
    assert_eq!(g.rounds.len(), 1);
}

#[tokio::test]
async fn the_recorded_order_matches_what_the_critic_was_shown() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let g = env.active().await;
    let prompt = env.launcher.last().prompt;
    let pos = |ruta: &str| prompt.find(ruta).unwrap();
    let artefacto_primero = pos("out/portada.md") < pos("ref/portada.md");
    assert_eq!(
        artefacto_primero,
        g.rounds[0].order == crate::gauntlet::AbOrder::ArtifactFirst
    );
}

#[tokio::test]
async fn a_gap_sends_only_the_gap_back_to_the_builder() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let informe = format!("INFORME-LARGO-DEL-CRITICO\n{}", env.gap().await);
    env.finish_and_advance(&informe).await;
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::Rebuilding);
    assert_eq!(g.round, 2);
    let launch = env.launcher.last();
    assert_eq!(launch.agent, "constructor");
    assert!(!launch.read_only);
    assert!(launch.prompt.contains("los márgenes están descuadrados"));
    assert!(launch.prompt.contains("pulir la portada"));
    assert!(!launch.prompt.contains("INFORME-LARGO-DEL-CRITICO"));
    assert_eq!(g.builder_children.len(), 2);
}

#[tokio::test]
async fn the_gap_handed_to_the_builder_is_capped() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let larga = "ñ".repeat(MAX_GAP_BYTES * 3);
    let informe = env.gap_with(&larga).await;
    env.finish_and_advance(&informe).await;
    assert!(env.launcher.last().prompt.len() < MAX_GAP_BYTES + 2_000);
}

#[tokio::test]
async fn a_gap_on_the_last_round_stops_without_launching_anyone() {
    let env = Entorno::nuevo().await;
    let mut input = Entorno::start_input();
    input["max_rounds"] = 1.into();
    env.run(input).await.unwrap();
    env.finish_and_advance("hecho").await;
    let lanzados = env.launcher.count();
    let informe = env.gap().await;
    let out = env.finish_and_advance(&informe).await;
    assert!(!out.is_error, "{}", out.content);
    assert_eq!(env.latest().await.phase, GauntletPhase::Stopped);
    assert_eq!(env.launcher.count(), lanzados);
    assert!(env
        .store
        .active_gauntlet(env.session)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn the_first_no_gap_launches_a_fresh_critic_with_the_order_swapped() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let primer_critico = env.launcher.last();
    let informe = env.no_gap().await;
    env.finish_and_advance(&informe).await;
    let segundo = env.launcher.last();
    assert_ne!(
        segundo.child, primer_critico.child,
        "estado fresco por juicio"
    );
    assert!(segundo.read_only);
    assert_eq!(segundo.agent, "critico");
    let orden = |prompt: &str| {
        prompt.find("out/portada.md").unwrap() < prompt.find("ref/portada.md").unwrap()
    };
    assert_ne!(orden(&primer_critico.prompt), orden(&segundo.prompt));
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::Judging);
    assert!(g.rounds.last().unwrap().bias_check);
    assert_eq!(g.critic_children.len(), 2);
}

#[tokio::test]
async fn a_coherent_recheck_waits_for_the_person_and_records_the_fingerprint() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let informe = env.no_gap().await;
    env.finish_and_advance(&informe).await;
    let informe = env.no_gap().await;
    let out = env.finish_and_advance(&informe).await;
    assert!(!out.is_error, "{}", out.content);
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::AwaitingFinalAcceptance);
    assert!(g.presented_sha256.is_some());
    assert_eq!(g.active_child, None);
}

#[tokio::test]
async fn a_contradicting_recheck_is_inconclusive() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let informe = env.no_gap().await;
    env.finish_and_advance(&informe).await;
    let informe = env.gap().await;
    env.finish_and_advance(&informe).await;
    assert_eq!(env.latest().await.phase, GauntletPhase::Inconclusive);
}

#[tokio::test]
async fn an_unreadable_verdict_is_inconclusive_not_approval() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    env.finish_and_advance("Me parece que está bien, la verdad.")
        .await;
    assert_eq!(env.latest().await.phase, GauntletPhase::Inconclusive);
}

#[tokio::test]
async fn a_child_that_fails_or_is_cancelled_ends_the_gauntlet() {
    for (estado, esperada) in [
        (TaskState::Failed, GauntletPhase::Failed),
        (TaskState::Canceled, GauntletPhase::Cancelled),
    ] {
        let env = Entorno::nuevo().await;
        env.start().await;
        let child = env.active().await.active_child.unwrap();
        env.launcher.end_with(child, estado);
        let out = env
            .run(serde_json::json!({"action": "advance"}))
            .await
            .unwrap();
        assert!(out.is_error);
        assert_eq!(env.latest().await.phase, esperada, "{estado:?}");
        assert_eq!(env.launcher.count(), 1);
    }
}

#[tokio::test]
async fn a_completed_child_without_a_report_fails_the_gauntlet() {
    let env = Entorno::nuevo().await;
    env.start().await;
    let child = env.active().await.active_child.unwrap();
    env.launcher.end_with(child, TaskState::Completed);
    let out = env
        .run(serde_json::json!({"action": "advance"}))
        .await
        .unwrap();
    assert!(out.is_error);
    assert_eq!(env.latest().await.phase, GauntletPhase::Failed);
}
