//! Tests de Anthropic: empty turn.

use super::*;
use crate::message::Message;

#[test]
fn an_empty_assistant_turn_is_omitted_instead_of_sending_an_empty_text_block() {
    // La Messages API rechaza los bloques de texto vacíos con un 400.
    let history = vec![
        Message::user("hola"),
        Message::assistant(vec![]),
        Message::user("sigues ahí"),
    ];
    let wire = wire_messages(&history, "claude-x", "https://api.anthropic.com");
    let roles: Vec<&str> = wire.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["user", "user"]);
    assert!(
        !wire.iter().any(|m| m["content"]
            .as_array()
            .is_some_and(|blocks| blocks.iter().any(|b| b["text"] == ""))),
        "no debe viajar ningún bloque de texto vacío: {wire:?}"
    );
}

#[test]
fn an_assistant_turn_with_text_is_kept() {
    let history = vec![
        Message::user("hola"),
        Message::assistant(vec![crate::message::ContentPart::Text {
            text: "buenas".into(),
        }]),
    ];
    let wire = wire_messages(&history, "claude-x", "https://api.anthropic.com");
    assert_eq!(wire.len(), 2);
    assert_eq!(wire[1]["content"][0]["text"], "buenas");
}
