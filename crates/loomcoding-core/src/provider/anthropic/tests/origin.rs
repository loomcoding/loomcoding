//! Tests: el origen del mensaje no sale hacia el proveedor.

use crate::message::Message;

#[test]
fn job_origin_never_reaches_the_anthropic_request() {
    let aviso = "[Job 7 watch] FAIL uno";
    let wire = super::wire_messages(
        &[Message::from_job("7", aviso)],
        "model",
        "https://api.example",
    );
    assert_eq!(wire[0]["role"], "user");
    assert_eq!(wire[0]["content"], aviso);
    assert!(!serde_json::to_string(&wire).unwrap().contains("origin"));
}
