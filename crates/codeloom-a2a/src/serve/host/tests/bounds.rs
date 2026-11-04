//! Tests: bounds.

use super::support::*;
use super::*;

#[tokio::test]
async fn stream_text_is_bounded() {
    let montaje = montar(
        Arc::new(ConTexto("x".repeat(5_000))),
        HostLimits {
            max_result_bytes: 100,
            ..limites()
        },
    )
    .await;

    let flujo = montaje
        .host
        .start_streaming("habla mucho".into())
        .await
        .unwrap();
    let eventos = recoger(flujo).await;

    let mut vistos = 0;
    for evento in &eventos {
        for texto in [
            evento["artifactUpdate"]["artifact"]["parts"][0]["text"].as_str(),
            evento["task"]["artifacts"][0]["parts"][0]["text"].as_str(),
        ]
        .into_iter()
        .flatten()
        {
            vistos += 1;
            assert!(texto.len() < 140, "{}", texto.len());
        }
    }
    assert!(vistos > 0, "{eventos:?}");
}

#[tokio::test]
async fn the_returned_text_is_bounded() {
    let montaje = montar(
        Arc::new(ConTexto("x".repeat(5_000))),
        HostLimits {
            max_result_bytes: 100,
            ..limites()
        },
    )
    .await;

    let inicial = montaje.host.start("habla mucho".into()).await.unwrap();
    let fin = montaje
        .host
        .wait_terminal(&id_de(&inicial), Duration::from_secs(10))
        .await
        .unwrap();

    let texto = fin.json["artifacts"][0]["parts"][0]["text"]
        .as_str()
        .unwrap();
    assert!(texto.len() < 140, "{}", texto.len());
    assert!(texto.ends_with("[truncated]"));
}
