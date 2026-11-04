use super::*;

#[test]
fn a_long_user_message_is_folded_to_ten_lines() {
    let mut app = App::new();
    app.push_user(&largo(25));

    assert_eq!(app.transcript.entry_lines(0).len(), MAX_BLOCK_LINES + 1);
    assert!(app.transcript.is_foldable(0));
}

#[test]
fn a_streaming_answer_stays_whole_and_folds_when_the_turn_ends() {
    let mut app = App::new();
    for i in 1..=30 {
        app.apply(LiveEvent::Delta {
            text: format!("l{i}\n\n"),
        });
    }
    let en_vivo = app.transcript.entry_lines(0).len();
    assert!(en_vivo > 30, "se ve entera mientras llega: {en_vivo}");

    app.apply(LiveEvent::TurnEnded);

    assert_eq!(app.transcript.entry_lines(0).len(), MAX_BLOCK_LINES + 1);
}

#[test]
fn restored_long_messages_come_back_folded_and_toggle_opens_them() {
    let mut app = App::new();
    app.restore_messages(vec![
        Message::user("hola"),
        Message::assistant(vec![ContentPart::Text { text: largo(40) }]),
    ]);

    assert_eq!(app.transcript.entry_lines(1).len(), MAX_BLOCK_LINES + 1);
    app.block = Some(1);
    assert!(app.toggle_block_fold());
    assert_eq!(app.transcript.entry_lines(1).len(), 40);
    app.block = Some(0);
    assert!(!app.toggle_block_fold(), "un bloque corto no se pliega");
}

#[test]
fn copying_a_folded_block_copies_the_hidden_lines_too() {
    let mut app = App::new();
    app.push_user(&largo(25));
    app.block = Some(0);

    assert!(app.copy_block());

    assert_eq!(app.clipboard.as_deref(), Some(largo(25).as_str()));
}
