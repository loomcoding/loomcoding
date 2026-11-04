use super::reasoning::run_script_with_steps;
use super::*;

fn read_round() -> Vec<StreamChunk> {
    vec![
        StreamChunk::ToolCallComplete {
            id: "c".into(),
            name: "view".into(),
            input: serde_json::json!({}),
        },
        StreamChunk::Done(FinishReason::ToolCalls),
    ]
}

fn final_round() -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta("listo".into()),
        StreamChunk::Done(FinishReason::Stop),
    ]
}

fn warnings(messages: &[Message]) -> usize {
    messages
        .iter()
        .filter(|m| m.role == crate::message::Role::User && m.text().contains("without writing"))
        .count()
}

#[tokio::test]
async fn a_long_streak_of_reads_gets_one_warning() {
    let mut scripts: Vec<_> = (0..11).map(|_| read_round()).collect();
    scripts.push(final_round());
    let (outcome, messages, notices) = run_script_with_steps(scripts, 30).await;
    assert_eq!(outcome.final_text, "listo");
    assert_eq!(warnings(&messages), 1);
    assert!(notices.iter().any(|n| n.contains("exploración")));
}

#[tokio::test]
async fn a_short_streak_of_reads_gets_no_warning() {
    let mut scripts: Vec<_> = (0..9).map(|_| read_round()).collect();
    scripts.push(final_round());
    let (_, messages, _) = run_script_with_steps(scripts, 30).await;
    assert_eq!(warnings(&messages), 0);
}
