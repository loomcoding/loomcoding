//! Tests: pastes.

use super::*;

#[test]
fn long_pastes_collapse_and_expand_on_submit() {
    let mut c = Composer::new();
    let pasted = "é".repeat(1001);
    c.insert_paste(&pasted);
    assert_eq!(c.text(), "[Pasted Content 1001 chars]");
    assert_eq!(c.submit(), Some(pasted));
}

#[test]
fn pasted_marker_text_is_literal_and_deletion_is_atomic() {
    let mut c = Composer::new();
    let pasted = "x".repeat(1001);
    c.insert_paste(&pasted);
    c.backspace();
    assert_eq!(c.text(), "");
    c.insert_str("[Pasted Content 1001 chars]");
    assert_eq!(c.submit(), Some("[Pasted Content 1001 chars]".to_string()));

    let mut c = Composer::new();
    c.insert_str("before");
    c.insert_paste(&pasted);
    c.insert_str("after");
    c.set_cursor(6);
    c.delete();
    assert_eq!(c.submit(), Some("beforeafter".to_string()));
}

#[test]
fn multiple_pastes_keep_order_and_copy_expanded_content() {
    let mut c = Composer::new();
    let first = "a".repeat(1001);
    let second = "b".repeat(1002);
    c.insert_paste(&first);
    c.insert_str("+");
    c.insert_paste(&second);
    c.select_all();
    let copied = c.selected_text_expanded().unwrap();
    assert_eq!(copied, format!("{first}+{second}"));
    assert_eq!(c.submit(), Some(copied));
}
