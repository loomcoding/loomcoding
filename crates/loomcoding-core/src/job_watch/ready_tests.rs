//! Pruebas de `ready` (lote listo sin consumirlo) sobre `Watch` y sobre `LocalJobRegistry`.
//!
//! Criterio que se fija aquí: `ready(t)` debe coincidir con `drain(t).is_some()` en el mismo
//! estado, pero sin cambiar nada del estado.

use super::*;
use crate::job::{JobRegistry, JobSpec, LocalJobRegistry};
use crate::session::SessionId;

const OWNER: SessionId = SessionId(1);

fn vigilancia(max_events: usize) -> Watch {
    Watch::new(WatchSpec::parse("FAIL", Some(max_events)).unwrap())
}

#[test]
fn ready_is_false_without_lines() {
    let watch = vigilancia(20);
    assert!(!watch.ready(Instant::now()));
}

#[test]
fn ready_is_true_with_pending_lines_and_does_not_consume_them() {
    let mut watch = vigilancia(20);
    watch.feed("FAIL uno\n");
    let now = Instant::now();
    assert!(watch.ready(now));
    assert!(watch.ready(now), "consultar no drena el lote");
    let batch = watch.drain(now).expect("el lote seguía ahí");
    assert_eq!(batch.lines, vec!["FAIL uno".to_string()]);
    assert!(!watch.ready(now), "drenado, ya no hay nada listo");
}

#[test]
fn ready_respects_the_batch_interval_while_the_job_runs() {
    let mut watch = vigilancia(20);
    let t0 = Instant::now();
    watch.feed("FAIL uno\n");
    assert!(watch.drain(t0).is_some());
    watch.feed("FAIL dos\n");
    assert!(!watch.ready(t0 + Duration::from_secs(1)));
    assert!(watch.ready(t0 + BATCH_INTERVAL));
}

#[test]
fn ready_ignores_the_interval_once_the_job_has_settled() {
    let mut watch = vigilancia(20);
    let t0 = Instant::now();
    watch.feed("FAIL uno\n");
    watch.drain(t0);
    watch.feed("FAIL dos\n");
    watch.finish();
    assert!(watch.ready(t0), "cerrada, el resto sale sin esperar");
}

#[test]
fn ready_agrees_with_drain_along_a_simulated_clock() {
    // Dos vigilancias idénticas: una se consulta con `ready` y la otra se drena en el mismo
    // instante. Deben dar lo mismo en cada paso.
    let t0 = Instant::now();
    let script: [(&str, u64); 6] = [
        ("FAIL a\nFAIL b\n", 0),
        ("", 1),
        ("FAIL c\n", 4),
        ("", 5),
        ("FAIL d\n", 6),
        ("", 12),
    ];
    let mut consultada = vigilancia(20);
    let mut drenada = vigilancia(20);
    for (texto, segundos) in script {
        consultada.feed(texto);
        drenada.feed(texto);
        let now = t0 + Duration::from_secs(segundos);
        let listo = consultada.ready(now);
        let entregado = drenada.drain(now).is_some();
        assert_eq!(listo, entregado, "en el segundo {segundos}");
        // Ambas vigilancias siguen el mismo camino: la consultada drena lo mismo que la otra.
        consultada.drain(now);
    }
}

#[tokio::test]
async fn registry_reports_ready_per_owner_without_consuming() {
    let registry = LocalJobRegistry::default();
    let job = registry
        .start(JobSpec {
            kind: "test".into(),
            label: "vigilado".into(),
            owner: OWNER,
            output_limit: 64 * 1024,
            watch: Some(WatchSpec::parse("FAIL", Some(20)).unwrap()),
            run: Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(None)
                })
            }),
        })
        .unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    let now = Instant::now();
    assert!(registry.watch_ready(OWNER, now));
    assert!(registry.watch_ready(OWNER, now), "no consume");
    assert!(
        !registry.watch_ready(SessionId(2), now),
        "otro dueño no ve nada"
    );
    assert_eq!(registry.take_watch_batches(OWNER, now).len(), 1);
    assert!(!registry.watch_ready(OWNER, now));
}

#[tokio::test]
async fn registry_without_watch_is_never_ready() {
    let registry = LocalJobRegistry::default();
    let job = registry
        .start(JobSpec {
            kind: "test".into(),
            label: "sin vigilancia".into(),
            owner: OWNER,
            output_limit: 64 * 1024,
            watch: None,
            run: Box::new(|handle, _| {
                Box::pin(async move {
                    handle.append("FAIL uno\n");
                    Ok(None)
                })
            }),
        })
        .unwrap();
    registry.wait(OWNER, job.id()).await.unwrap();
    assert!(!registry.watch_ready(OWNER, Instant::now()));
}
