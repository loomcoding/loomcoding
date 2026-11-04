//! Tests: query.

use super::support::*;
use super::*;

#[tokio::test]
async fn get_and_cancel_map_the_host_answers() {
    let (estado, _) = bearer(Falso::default());
    let con = |metodo: &str, id: &str| json!({ "jsonrpc": "2.0", "id": "x", "method": metodo, "params": { "id": id } });

    let (_, valor) = llamar(&estado, con("GetTask", "t-1")).await;
    assert_eq!(valor["result"]["id"], "t-1");
    assert_eq!(valor["id"], "x");

    let (_, valor) = llamar(&estado, con("CancelTask", "t-1")).await;
    assert_eq!(valor["result"]["status"]["state"], "TASK_STATE_CANCELED");

    let (_, valor) = llamar(&estado, con("CancelTask", "terminada")).await;
    assert_eq!(valor["error"]["code"], TASK_NOT_CANCELABLE);

    let (_, valor) = llamar(&estado, con("GetTask", "inventada")).await;
    assert_eq!(valor["error"]["code"], TASK_NOT_FOUND);
}

#[tokio::test]
async fn an_unknown_and_a_foreign_task_look_exactly_the_same() {
    let (estado, _) = bearer(Falso::default());
    let con = |id: &str| json!({ "jsonrpc": "2.0", "id": 7, "method": "GetTask", "params": { "id": id } });

    let (_, _, a) = enviar_req(&estado, buena(con("no-existe"))).await;
    let (_, _, b) = enviar_req(&estado, buena(con("de-otra-sesion"))).await;

    assert_eq!(a, b);
}

#[tokio::test]
async fn a_missing_or_huge_task_id_is_invalid_params() {
    let (estado, _) = bearer(Falso::default());
    let largo = "a".repeat(MAX_ID_BYTES + 1);
    for params in [
        json!({}),
        json!({ "id": "" }),
        json!({ "id": 3 }),
        json!({ "id": largo }),
    ] {
        let cuerpo = json!({ "jsonrpc": "2.0", "id": 1, "method": "GetTask", "params": params });

        let (_, valor) = llamar(&estado, cuerpo).await;

        assert_eq!(valor["error"]["code"], INVALID_PARAMS, "{params}");
    }
}
