//! Tecla `?`: abre la ayuda solo con el compositor vacío y sin ningún diálogo o panel abierto.
//! Con texto en el compositor es un carácter más, y `F1` sigue abriendo la ayuda como siempre.

use super::*;
use loomcoding_core::question::{Question, QuestionOption};

fn ask_question() -> Question {
    let option = |label: &str| QuestionOption {
        label: label.into(),
        description: String::new(),
    };
    Question {
        header: "Stack".into(),
        question: "Which?".into(),
        options: vec![option("a"), option("b")],
        multi_select: false,
    }
}

#[test]
fn question_mark_opens_help_with_an_empty_composer() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(app.help_open);
    assert!(app.composer.is_empty());
}

#[test]
fn question_mark_closes_the_help_it_opened() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(app.help_open);
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(app.composer.is_empty());
    assert!(!app.cancel_requested);
}

#[test]
fn question_mark_types_a_character_when_the_composer_has_text() {
    let mut app = App::new();
    app.composer.set_text("hola");
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert_eq!(app.composer.text(), "hola?");
}

#[test]
fn question_mark_does_not_open_help_over_a_permission_dialog_or_question() {
    let mut app = App::new();
    let _answer = open_dialog(&mut app);
    assert!(app.permission.is_some());
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(app.composer.is_empty());

    let mut app = App::new();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question()], tx);
    assert!(app.question.is_some());
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(app.composer.is_empty());
}

#[test]
fn question_mark_does_not_open_help_over_other_panels() {
    let mut app = App::new();
    app.queue_open = true;
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(app.composer.is_empty());

    let mut app = App::new();
    app.tools_open = true;
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(app.composer.is_empty());

    let mut app = App::new();
    app.settings_open = true;
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(app.composer.is_empty());
}
