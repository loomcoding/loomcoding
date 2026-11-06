//! Pruebas de la vigilancia de jobs: troceo, cola y registro. Tiempo simulado con `Instant`.

use super::*;
use regex::Regex;
use std::time::{Duration, Instant};

fn matcher(pattern: &str) -> LineMatcher {
    LineMatcher::new(Regex::new(pattern).unwrap())
}

fn spec(pattern: &str, max_events: usize) -> WatchSpec {
    WatchSpec::parse(pattern, Some(max_events)).unwrap()
}

// --- WatchSpec ---------------------------------------------------------------------------

#[test]
fn parse_uses_default_and_accepts_the_cap() {
    assert_eq!(WatchSpec::parse("FAIL", None).unwrap().max_events, 20);
    assert_eq!(WatchSpec::parse("FAIL", Some(100)).unwrap().max_events, 100);
    assert_eq!(WatchSpec::parse("FAIL", Some(1)).unwrap().max_events, 1);
}

#[test]
fn parse_rejects_zero_and_over_cap_max_events() {
    assert!(WatchSpec::parse("FAIL", Some(0)).is_err());
    assert!(WatchSpec::parse("FAIL", Some(101)).is_err());
}

#[test]
fn parse_rejects_empty_and_long_patterns() {
    assert!(WatchSpec::parse("", None).is_err());
    let at_limit = "a".repeat(MAX_PATTERN_BYTES);
    assert!(WatchSpec::parse(&at_limit, None).is_ok());
    let too_long = "a".repeat(MAX_PATTERN_BYTES + 1);
    let error = WatchSpec::parse(&too_long, None).unwrap_err().to_string();
    assert!(error.contains("256"), "{error}");
}

#[test]
fn parse_rejects_invalid_regex_with_a_clear_message() {
    let error = WatchSpec::parse("error[", None).unwrap_err().to_string();
    assert!(error.contains("invalid watch pattern"), "{error}");
}

#[test]
fn parse_rejects_patterns_that_exceed_the_automaton_limit() {
    assert!(WatchSpec::parse("(a{250}){250}", None).is_err());
}

// --- LineMatcher -------------------------------------------------------------------------

#[test]
fn complete_lines_match_even_when_split_across_reads() {
    let mut m = matcher("FAIL");
    assert!(m.push("FA").is_empty());
    assert_eq!(m.push("IL one\nok\nFA"), vec!["FAIL one".to_string()]);
    assert_eq!(m.push("IL two\n"), vec!["FAIL two".to_string()]);
}

#[test]
fn lines_are_matched_in_order_and_non_matching_lines_are_dropped() {
    let mut m = matcher("FAIL");
    let found = m.push("FAIL 1\nok\nFAIL 2\nFAIL 3\n");
    assert_eq!(found, vec!["FAIL 1", "FAIL 2", "FAIL 3"]);
}

#[test]
fn carriage_returns_before_newline_are_not_part_of_the_line() {
    let mut m = matcher("^FAIL$");
    assert_eq!(m.push("FAIL\r\n"), vec!["FAIL".to_string()]);
}

#[test]
fn partial_without_newline_is_bounded_and_keeps_the_head_of_the_line() {
    let mut m = matcher("FAIL");
    let long = format!("FAIL{}", "x".repeat(5_000));
    assert!(m.push(&long).is_empty());
    assert_eq!(m.partial.len(), MAX_PARTIAL_BYTES);
    // La línea cerrada conserva su principio, que es donde casa el patrón.
    assert_eq!(m.push("\nFAIL next\n").len(), 2);
    assert!(m.partial.is_empty());
}

#[test]
fn oversized_line_tail_is_discarded_until_the_newline() {
    let mut m = matcher("FAIL");
    m.push(&"x".repeat(MAX_PARTIAL_BYTES + 10));
    assert!(m.discarding);
    assert!(m.push("FAIL tail\n").is_empty(), "el resto ya no entra");
    assert!(!m.discarding);
    assert_eq!(m.push("FAIL after\n"), vec!["FAIL after".to_string()]);
}

#[test]
fn oversized_multibyte_line_is_cut_on_a_character_boundary() {
    let mut m = matcher("a");
    // "a" seguida de 3000 "é" (2 bytes cada una): el corte de 4096 cae en medio de un carácter.
    let text = format!("a{}", "é".repeat(3_000));
    assert!(m.push(&text).is_empty());
    assert!(m.partial.len() <= MAX_PARTIAL_BYTES);
    assert!(m.partial.is_char_boundary(m.partial.len()));
}

#[test]
fn utf8_split_between_reads_loses_and_duplicates_nothing() {
    let bytes = "línea ñ FAIL\nsegunda FAIL\n".as_bytes();
    // Corte dentro de la «ñ» (dos bytes): la primera lectura termina en su primer byte.
    let cut = "línea ".len() + 1;
    let mut decoder = ChunkDecoder::default();
    let mut m = matcher("FAIL");
    let mut text = decoder.decode(&bytes[..cut]);
    text.push_str(&decoder.decode(&bytes[cut..]));
    text.push_str(&decoder.finish());
    assert_eq!(text, "línea ñ FAIL\nsegunda FAIL\n");
    let found = m.push(&text);
    assert_eq!(found, vec!["línea ñ FAIL", "segunda FAIL"]);
}

#[test]
fn decoder_replaces_invalid_bytes_and_keeps_only_incomplete_tails() {
    let mut decoder = ChunkDecoder::default();
    // 0xFF nunca es válido: se sustituye; el tercer byte de una «€» incompleta se retiene.
    let text = decoder.decode(b"ok\xFFx\xE2\x82");
    assert_eq!(text, "ok\u{FFFD}x");
    assert_eq!(decoder.decode(b"\xAC!"), "€!");
    assert_eq!(decoder.finish(), "");
}

#[test]
fn finish_tests_the_unterminated_rest_once() {
    let mut m = matcher("FAIL");
    assert!(m.push("FAIL at end").is_empty());
    assert_eq!(m.finish(), vec!["FAIL at end".to_string()]);
    assert!(m.finish().is_empty());
}

#[test]
fn finish_ignores_an_unterminated_rest_that_does_not_match() {
    let mut m = matcher("FAIL");
    m.push("tail without match");
    assert!(m.finish().is_empty());
}

// --- EventQueue --------------------------------------------------------------------------

#[test]
fn batches_hold_ten_lines_and_count_the_rest_as_suppressed() {
    let t0 = Instant::now();
    let mut queue = EventQueue::new(100);
    for i in 0..12 {
        queue.push(&format!("FAIL {i}"));
    }
    let batch = queue.drain_batch(t0).unwrap();
    assert_eq!(batch.lines.len(), MAX_LINES_PER_BATCH);
    assert_eq!(batch.lines[0], "FAIL 0");
    assert_eq!(batch.suppressed, 2);
    assert!(!batch.limit_reached);
}

#[test]
fn at_most_one_batch_every_five_seconds() {
    let t0 = Instant::now();
    let mut queue = EventQueue::new(100);
    queue.push("FAIL a");
    assert!(queue.drain_batch(t0).is_some());
    queue.push("FAIL b");
    assert!(queue.drain_batch(t0 + Duration::from_secs(4)).is_none());
    let batch = queue.drain_batch(t0 + Duration::from_secs(5)).unwrap();
    assert_eq!(batch.lines, vec!["FAIL b".to_string()]);
    assert_eq!(batch.suppressed, 0);
}

#[test]
fn nothing_pending_means_no_batch() {
    let mut queue = EventQueue::new(100);
    assert!(queue.drain_batch(Instant::now()).is_none());
}

#[test]
fn queue_is_capped_at_fifty_lines() {
    let t0 = Instant::now();
    let mut queue = EventQueue::new(100);
    for i in 0..60 {
        queue.push(&format!("FAIL {i}"));
    }
    let batch = queue.drain_batch(t0).unwrap();
    assert_eq!(batch.lines.len(), MAX_LINES_PER_BATCH);
    // 10 suprimidas al encolar (la cola llegó a 50) y 40 que sobraban al tomar el lote.
    assert_eq!(batch.suppressed, 50);
    assert_eq!(queue.suppressed, 50);
    assert_eq!(queue.delivered, MAX_LINES_PER_BATCH);
}

#[test]
fn lines_are_truncated_to_512_bytes() {
    let t0 = Instant::now();
    let mut queue = EventQueue::new(100);
    queue.push(&format!("FAIL {}", "é".repeat(600)));
    let batch = queue.drain_batch(t0).unwrap();
    assert!(batch.lines[0].len() <= MAX_EVENT_BYTES);
    assert!(batch.lines[0].starts_with("FAIL é"));
}

#[test]
fn max_events_stops_admitting_and_announces_the_limit_once() {
    let t0 = Instant::now();
    let mut queue = EventQueue::new(3);
    for i in 0..5 {
        queue.push(&format!("FAIL {i}"));
    }
    let first = queue.drain_batch(t0).unwrap();
    assert_eq!(first.lines.len(), 3);
    assert!(first.limit_reached);
    assert!(queue.limit_reached);
    // El aviso de límite ya se entregó: después no hay nada más que decir.
    assert!(queue.drain_batch(t0 + Duration::from_secs(10)).is_none());
}

#[test]
fn limit_is_announced_even_without_pending_lines() {
    let t0 = Instant::now();
    let mut queue = EventQueue::new(1);
    queue.push("FAIL only");
    let batch = queue.drain_batch(t0).unwrap();
    assert_eq!(batch.lines, vec!["FAIL only".to_string()]);
    assert!(batch.limit_reached);
}

// --- Watch -------------------------------------------------------------------------------

#[test]
fn watch_reports_its_counters_and_pattern() {
    let t0 = Instant::now();
    let mut watch = Watch::new(spec("FAIL", 100));
    watch.feed("FAIL a\nFAIL b\n");
    let batch = watch.drain(t0).unwrap();
    assert_eq!(batch.lines.len(), 2);
    let info = watch.info();
    assert_eq!(info.pattern, "FAIL");
    assert_eq!(info.delivered, 2);
    assert_eq!(info.suppressed, 0);
    assert!(!info.limit_reached);
}

#[test]
fn watch_ignores_input_after_finish() {
    let mut watch = Watch::new(spec("FAIL", 100));
    watch.feed("FAIL a");
    watch.finish();
    watch.feed("FAIL b\n");
    let batch = watch.drain(Instant::now()).unwrap();
    assert_eq!(batch.lines, vec!["FAIL a".to_string()]);
}
