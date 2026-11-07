//! Tests: el origen del mensaje no sale hacia el proveedor.

use super::support::*;
use super::*;

#[test]
fn job_origin_never_reaches_the_openai_request() {
    let aviso = "[Job 7 watch] FAIL uno";
    let mut req = peticion(None);
    req.messages = vec![Message::from_job("7", aviso)];
    let body = OpenAiCompatProvider::new("http://x", "k").build_body(&req);
    let enviado = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["content"] == aviso)
        .expect("el aviso debe llegar al proveedor");
    assert_eq!(enviado["role"], "user");
    assert_eq!(
        enviado.as_object().unwrap().len(),
        2,
        "solo rol y contenido"
    );
    assert!(!body.to_string().contains("origin"));
}
