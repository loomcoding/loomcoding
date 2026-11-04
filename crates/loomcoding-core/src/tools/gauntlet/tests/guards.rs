//! Guardas de integridad: el crítico debe leer los dos candidatos y la comprobación objetiva manda.

use super::support::*;
use super::*;

fn start_with_check(command: &str) -> serde_json::Value {
    let mut input = Entorno::start_input();
    input["check"] = command.into();
    input
}

// ---- A: prueba de lectura del crítico ------------------------------------------------------

#[tokio::test]
async fn a_verdict_without_reading_both_candidates_is_inconclusive() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let report = env.no_gap().await;
    env.record_views(
        env.active().await.active_child.unwrap(),
        &["out/portada.md", "out/portada.md"],
    )
    .await;
    let out = env.finish_without_reading(&report).await;
    assert!(!out.is_error, "{}", out.content);
    let g = env.latest().await;
    assert_eq!(g.phase, GauntletPhase::Inconclusive);
    assert_eq!(env.launcher.count(), 2, "no se lanza otro crítico");
    let events = env.store.gauntlet_events(g.id).await.unwrap();
    let last = events.last().unwrap();
    assert!(last.detail.contains("ref/portada.md"), "{}", last.detail);
}

#[tokio::test]
async fn a_critic_that_never_called_view_cannot_approve() {
    let env = Entorno::nuevo().await;
    env.hasta_el_juicio().await;
    let report = env.no_gap().await;
    env.finish_without_reading(&report).await;
    assert_eq!(env.latest().await.phase, GauntletPhase::Inconclusive);
}

// ---- B: comprobación objetiva -----------------------------------------------------------------

#[tokio::test]
async fn a_failing_check_sends_its_output_to_the_builder_without_a_critic() {
    let env = Entorno::nuevo().await;
    let out = env
        .run(start_with_check("echo boom-del-check && exit 1"))
        .await
        .unwrap();
    assert!(!out.is_error, "{}", out.content);
    assert!(env.launcher.last().prompt.contains("echo boom-del-check"));
    env.finish_without_reading("hecho").await;
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::Rebuilding);
    assert_eq!(g.round, 2);
    assert_eq!(
        env.launcher.count(),
        2,
        "el segundo hijo es el constructor, no un crítico"
    );
    let launch = env.launcher.last();
    assert_eq!(launch.agent, "constructor");
    assert!(
        launch.prompt.contains("boom-del-check"),
        "{}",
        launch.prompt
    );
}

#[tokio::test]
async fn a_failing_check_in_the_last_round_stops_the_gauntlet() {
    let env = Entorno::nuevo().await;
    let mut input = start_with_check("exit 1");
    input["max_rounds"] = 1.into();
    env.run(input).await.unwrap();
    env.finish_without_reading("hecho").await;
    assert_eq!(env.latest().await.phase, GauntletPhase::Stopped);
    assert_eq!(env.launcher.count(), 1, "sin rondas no hay crítico");
}

#[tokio::test]
async fn a_passing_check_lets_the_critic_judge() {
    let env = Entorno::nuevo().await;
    env.run(start_with_check("exit 0")).await.unwrap();
    env.finish_without_reading("hecho").await;
    let g = env.active().await;
    assert_eq!(g.phase, GauntletPhase::Judging);
    assert_eq!(env.launcher.last().agent, "critico");
}

#[tokio::test]
async fn the_check_is_stored_and_told_to_the_builder() {
    let env = Entorno::nuevo().await;
    env.run(start_with_check("exit 0")).await.unwrap();
    assert_eq!(env.active().await.check.as_deref(), Some("exit 0"));
    assert!(env.launcher.last().prompt.contains("exit 0"));
}

#[tokio::test]
async fn a_denied_check_command_refuses_to_start() {
    let env = Entorno::nuevo().await;
    let error = env
        .run_as(false, start_with_check("exit 0"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("check"), "{error}");
    assert_eq!(env.launcher.count(), 0);
}

#[tokio::test]
async fn the_check_must_be_a_short_string() {
    let env = Entorno::nuevo().await;
    for bad in [serde_json::json!(5), serde_json::json!("x".repeat(2000))] {
        let mut input = Entorno::start_input();
        input["check"] = bad;
        let error = env.run(input).await.unwrap_err();
        assert!(error.to_string().contains("check"), "{error}");
    }
}

#[tokio::test]
async fn the_same_failure_twice_is_called_out_to_the_builder() {
    let env = Entorno::nuevo().await;
    let mut input = start_with_check("echo mismo-error && exit 1");
    input["max_rounds"] = 3.into();
    env.run(input).await.unwrap();
    env.finish_without_reading("hecho").await;
    let first = env.launcher.last().prompt;
    assert!(first.contains("mismo-error"));
    assert!(!first.contains("SAME failure"), "{first}");
    env.finish_without_reading("hecho otra vez").await;
    let second = env.launcher.last().prompt;
    assert!(second.contains("SAME failure"), "{second}");
    assert!(
        second.contains("mismo-error"),
        "la salida sigue viéndose: {second}"
    );
}

#[tokio::test]
async fn a_different_failure_is_not_called_the_same() {
    let env = Entorno::nuevo().await;
    let file = env.project.path().join("salida.txt");
    std::fs::write(&file, "fallo-uno").unwrap();
    // El comando imprime ese fichero: el test lo cambia entre rondas.
    let command = if cfg!(windows) {
        "type salida.txt && exit 1"
    } else {
        "cat salida.txt; exit 1"
    };
    let mut input = start_with_check(command);
    input["max_rounds"] = 3.into();
    env.run(input).await.unwrap();
    env.finish_without_reading("a").await;
    assert!(env.launcher.last().prompt.contains("fallo-uno"));
    std::fs::write(&file, "fallo-dos").unwrap();
    env.finish_without_reading("b").await;
    let second = env.launcher.last().prompt;
    assert!(second.contains("fallo-dos"), "{second}");
    assert!(!second.contains("SAME failure"), "{second}");
}

#[tokio::test]
async fn the_same_failure_is_still_recognised_in_the_third_round() {
    let env = Entorno::nuevo().await;
    let mut input = start_with_check("echo mismo-error-largo && exit 1");
    input["max_rounds"] = 3.into();
    env.run(input).await.unwrap();
    env.finish_without_reading("1").await;
    env.finish_without_reading("2").await;
    env.finish_without_reading("3").await;
    // La tercera ronda agota el tope: la brecha de la segunda ya llevaba la advertencia y se acotó.
    assert_eq!(env.latest().await.phase, GauntletPhase::Stopped);
    let g = env.latest().await;
    let gap = g.last_gap.unwrap();
    assert!(gap.contains("SAME failure"), "{gap}");
}
