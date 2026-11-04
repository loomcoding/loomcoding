//! Gauntlets: estado y bitácora persisten al reabrir, y las reglas de versión y fase se imponen.

use super::*;
use crate::gauntlet::{Gauntlet, GauntletId, GauntletPhase, NewGauntlet, Verdict};
use tempfile::tempdir;

fn gauntlet(parent: SessionId) -> Gauntlet {
    Gauntlet::new(NewGauntlet {
        id: GauntletId(0),
        parent,
        goal: "pulir la portada".into(),
        artifact: "out/portada.md".into(),
        bar: vec!["ref/portada.md".into()],
        builder: "constructor".into(),
        critic: "critico".into(),
        max_rounds: None,
        now_ms: 0,
    })
    .unwrap()
}

async fn abrir(path: &std::path::Path) -> SqliteStore {
    SqliteStore::open(path.to_str().unwrap()).await.unwrap()
}

#[tokio::test]
async fn gauntlet_state_and_append_only_events_survive_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("sessions.db");
    let store = abrir(&path).await;
    let parent = store.create_session("project").await.unwrap();
    let created = store
        .create_gauntlet(gauntlet(parent), "builder started")
        .await
        .unwrap();
    assert_eq!(created.version, 1);
    assert_ne!(created.id, GauntletId(0));

    let mut judging = created.clone();
    judging.begin_judging(1).unwrap();
    let judging = store
        .update_gauntlet(judging, "coordinator", "critic launched")
        .await
        .unwrap();
    assert_eq!(judging.version, 2);

    let mut stale = created;
    stale.cancel().unwrap();
    assert!(store
        .update_gauntlet(stale, "parent", "must not overwrite")
        .await
        .is_err());

    let events = store.gauntlet_events(judging.id).await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].from, None);
    assert_eq!(events[0].to, GauntletPhase::Building);
    assert_eq!(events[1].from, Some(GauntletPhase::Building));
    assert_eq!(events[1].to, GauntletPhase::Judging);
    assert_eq!(events[1].actor, "coordinator");
    drop(store);

    let reopened = abrir(&path).await;
    let loaded = reopened.gauntlet(judging.id).await.unwrap().unwrap();
    assert_eq!(loaded, judging);
    assert_eq!(loaded.rounds.len(), 1);
    assert_eq!(
        reopened.active_gauntlet(parent).await.unwrap(),
        Some(loaded)
    );
}

#[tokio::test]
async fn an_invalid_phase_transition_is_rejected_by_the_store() {
    let directory = tempdir().unwrap();
    let store = abrir(&directory.path().join("sessions.db")).await;
    let parent = store.create_session("project").await.unwrap();
    let created = store
        .create_gauntlet(gauntlet(parent), "start")
        .await
        .unwrap();
    let mut bad = created.clone();
    bad.phase = GauntletPhase::Completed;
    assert!(store
        .update_gauntlet(bad, "parent", "skips the whole loop")
        .await
        .is_err());
    assert_eq!(
        store.gauntlet(created.id).await.unwrap().unwrap().phase,
        GauntletPhase::Building
    );
}

#[tokio::test]
async fn a_new_gauntlet_must_start_blank_and_building() {
    let directory = tempdir().unwrap();
    let store = abrir(&directory.path().join("sessions.db")).await;
    let parent = store.create_session("project").await.unwrap();
    let mut con_id = gauntlet(parent);
    con_id.id = GauntletId(9);
    assert!(store.create_gauntlet(con_id, "x").await.is_err());
    let mut con_version = gauntlet(parent);
    con_version.version = 3;
    assert!(store.create_gauntlet(con_version, "x").await.is_err());
    let mut ya_juzgando = gauntlet(parent);
    ya_juzgando.begin_judging(0).unwrap();
    assert!(store.create_gauntlet(ya_juzgando, "x").await.is_err());
    let largo = "x".repeat(2_001);
    assert!(store
        .create_gauntlet(gauntlet(parent), &largo)
        .await
        .is_err());
}

#[tokio::test]
async fn only_one_active_gauntlet_per_session_until_it_ends() {
    let directory = tempdir().unwrap();
    let store = abrir(&directory.path().join("sessions.db")).await;
    let parent = store.create_session("project").await.unwrap();
    let first = store
        .create_gauntlet(gauntlet(parent), "one")
        .await
        .unwrap();
    assert!(store
        .create_gauntlet(gauntlet(parent), "two")
        .await
        .is_err());

    let other = store.create_session("other").await.unwrap();
    store
        .create_gauntlet(gauntlet(other), "independent session")
        .await
        .unwrap();

    let mut ended = first.clone();
    ended.cancel().unwrap();
    store
        .update_gauntlet(ended, "parent", "cancelled")
        .await
        .unwrap();
    assert_eq!(store.active_gauntlet(parent).await.unwrap(), None);
    let second = store
        .create_gauntlet(gauntlet(parent), "again")
        .await
        .unwrap();
    assert_ne!(second.id, first.id);
    assert_eq!(
        store.latest_gauntlet(parent).await.unwrap().unwrap().id,
        second.id
    );
}

#[tokio::test]
async fn the_rounds_and_verdicts_are_kept_in_the_stored_payload() {
    let directory = tempdir().unwrap();
    let store = abrir(&directory.path().join("sessions.db")).await;
    let parent = store.create_session("project").await.unwrap();
    let mut g = store
        .create_gauntlet(gauntlet(parent), "start")
        .await
        .unwrap();
    g.begin_judging(1).unwrap();
    g = store
        .update_gauntlet(g, "coordinator", "judge")
        .await
        .unwrap();
    g.apply_verdict(Verdict::Gap {
        biggest_gap: "los márgenes".into(),
    })
    .unwrap();
    g = store.update_gauntlet(g, "critic", "gap").await.unwrap();
    let loaded = store.gauntlet(g.id).await.unwrap().unwrap();
    assert_eq!(loaded.phase, GauntletPhase::Rebuilding);
    assert_eq!(loaded.round, 2);
    assert_eq!(loaded.last_gap.as_deref(), Some("los márgenes"));
    assert!(matches!(
        loaded.rounds[0].verdict,
        Some(Verdict::Gap { .. })
    ));
}
