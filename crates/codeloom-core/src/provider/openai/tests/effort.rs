//! Tests: effort.

use super::support::*;
use super::*;

#[test]
fn openai_body_without_effort_is_unchanged() {
    // Cuerpo exacto que enviaba `build_body` antes de existir el esfuerzo.
    let previo = json!({
        "model": "m",
        "messages": [{ "role": "system", "content": "sys" }],
        "tools": [{
            "type": "function",
            "function": {
                "name": "view",
                "description": "Ve un fichero",
                "parameters": { "type": "object" }
            }
        }],
        "stream": true,
        "stream_options": { "include_usage": true },
        "temperature": 0.5,
        "max_tokens": 16
    });
    let p = OpenAiCompatProvider::new("http://x", "k");
    assert_eq!(p.build_body(&peticion(None)), previo);
}

#[test]
fn openai_body_sends_reasoning_effort() {
    let p = OpenAiCompatProvider::new("http://x", "k");
    let con = p.build_body(&peticion(Some("high")));
    assert_eq!(con["reasoning_effort"], "high");
    assert!(con.get("reasoning").is_none());
    // Salvo el campo nuevo, el cuerpo es idéntico al de «default».
    let mut sin_campo = con.clone();
    sin_campo
        .as_object_mut()
        .unwrap()
        .remove("reasoning_effort");
    assert_eq!(sin_campo, p.build_body(&peticion(None)));
}

#[test]
fn openrouter_body_nests_effort_under_reasoning() {
    let p = OpenAiCompatProvider::new("https://openrouter.ai/api/v1", "k");
    let con = p.build_body(&peticion(Some("high")));
    assert_eq!(con["reasoning"]["effort"], "high");
    assert!(con.get("reasoning_effort").is_none());
}
