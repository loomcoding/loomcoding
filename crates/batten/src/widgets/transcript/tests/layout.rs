//! Tests: layout.

use super::support::*;
use super::*;

#[test]
fn only_changed_entries_are_measured() {
    let (mut t, last) = long_transcript(1000);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 10);
    draw(&t, &mut vp, &mut layout, 40, 10);
    let before = layout.measured();
    t.append(last, " y más");
    let out = draw(&t, &mut vp, &mut layout, 40, 10);
    assert_eq!(layout.measured(), before + 1);
    assert!(out.contains("fin y más"));
}

#[test]
fn width_change_remeasures_everything() {
    let (t, _) = long_transcript(100);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 10);
    let before = layout.measured();
    draw(&t, &mut vp, &mut layout, 60, 10);
    assert!(layout.measured() >= before + 101);
}

#[test]
fn line_at_row_handles_multi_row_lines() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Banner, "corta"));
    t.push(Line::new(LineKind::Banner, "palabra ".repeat(10)));
    t.push(Line::new(LineKind::Banner, "otra"));
    let mut layout = Layout::new();
    layout.sync(&t, 20, &Theme::test());
    let h1 = layout.height_of(1);
    assert!(h1 > 1);
    assert_eq!(layout.line_at_row(0), Some((0, 0)));
    assert_eq!(layout.line_at_row(1), Some((1, 0)));
    assert_eq!(layout.line_at_row(h1), Some((1, h1 - 1)));
    assert_eq!(layout.line_at_row(h1 + 1), Some((2, 0)));
    assert_eq!(layout.line_at_row(10_000), Some((2, 0)));
}

#[test]
fn rows_beyond_u16_are_reachable() {
    let mut t = Transcript::new();
    for i in 0..70_000 {
        t.push(Line::new(LineKind::Banner, format!("l{i:05}")));
    }
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    let out = draw(&t, &mut vp, &mut layout, 20, 5);
    assert!(out.contains("l69999"));
    vp.to_top();
    let out = draw(&t, &mut vp, &mut layout, 20, 5);
    assert!(out.contains("l00000"));
}

#[test]
fn width_change_keeps_the_anchor_line_on_top() {
    let mut t = Transcript::new();
    for i in 0..200 {
        t.push(Line::new(
            LineKind::Assistant,
            format!("a{i:03} {}", "xx ".repeat(15)),
        ));
    }
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 10);
    vp.scroll_up(100);
    draw(&t, &mut vp, &mut layout, 40, 10);
    let (line, _) = vp.anchor().unwrap();
    draw(&t, &mut vp, &mut layout, 25, 10);
    assert_eq!(vp.anchor().unwrap().0, line);
}
