//! Tests: lines.

use super::support::*;
use super::*;

#[test]
fn shows_crush_style_role_headers() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::User, "hola"));
    t.push(Line::new(LineKind::Assistant, "buenas"));
    let out = buffer(&t, 40, 10);
    assert!(out.contains("👤"));
    assert!(out.contains("🤖"));
    assert!(!out.contains("loom:"));
    assert!(out.contains("hola"));
    assert!(out.contains("buenas"));
}

#[test]
fn tool_entries_paint_a_prefix_that_is_not_part_of_the_text() {
    use crate::state::{tool_lines, ToolItem};
    let mut item = ToolItem::new("c1", "View");
    item.set_summary("src/lib.rs");
    item.set_result(false);
    item.set_output("uno\ndos");
    let mut t = Transcript::new();
    t.push_lines(tool_lines(&item));
    let out = buffer(&t, 40, 5);
    assert!(out.contains("● View"), "falta la cabecera de la tool");
    assert!(out.contains("│ uno"), "falta el cuerpo de la tool");
    assert!(!t.line(0).unwrap().plain().contains('●'));
    assert!(!t.line(1).unwrap().plain().contains('│'));
}

#[test]
fn diff_lines_render_with_insert_and_delete_styles() {
    use crate::state::{DiffKind, StyleToken};
    let theme = Theme::test();
    let mut t = Transcript::new();
    let line = |kind: DiffKind, text: &str| {
        let mut l = Line::new(LineKind::ToolDiff(kind), text);
        l.spans.push(crate::state::SpanSpec {
            start: 0,
            len: text.len(),
            token: match kind {
                DiffKind::Insert => StyleToken::DiffInsert,
                DiffKind::Delete => StyleToken::DiffDelete,
                _ => StyleToken::DiffHunk,
            },
        });
        l
    };
    t.push_lines(vec![
        line(DiffKind::Delete, "- uno"),
        line(DiffKind::Insert, "+ dos"),
    ]);
    let mut term = Terminal::new(TestBackend::new(20, 3)).unwrap();
    term.draw(|f| {
        render(
            f,
            f.area(),
            &t,
            &mut Viewport::new(),
            &mut Layout::new(),
            None,
            &theme,
        )
    })
    .unwrap();
    let buf = term.backend().buffer();
    // Con el prefijo «│ » el texto empieza en la columna 2.
    let deleted = theme.style(StyleToken::DiffDelete);
    let inserted = theme.style(StyleToken::DiffInsert);
    assert_ne!(deleted.fg, inserted.fg, "precondición: estilos distintos");
    assert_eq!(Some(buf[(2, 0)].fg), deleted.fg);
    assert_eq!(Some(buf[(2, 1)].fg), inserted.fg);
}

#[test]
fn activity_entries_paint_a_prefix_that_is_not_part_of_the_text() {
    use crate::state::{activity_lines, ActivityView, ToolStatus};
    let lines = activity_lines(&ActivityView {
        kind: "job",
        label: "build",
        status: ToolStatus::Running,
        note: None,
        progress: None,
        body: None,
    });
    let mut t = Transcript::new();
    t.push_lines(lines);
    let out = buffer(&t, 40, 3);
    assert!(out.contains("◆ job"), "falta la cabecera de la actividad");
    assert!(!t.line(0).unwrap().plain().contains('◆'));
}

#[test]
fn node_lines_have_no_glyph_prefix() {
    assert_eq!(prefix(LineKind::NodeHead(ToolStatus::Running)), "");
    assert_eq!(prefix(LineKind::NodeBody), "");
}

#[test]
fn todo_entries_paint_a_prefix_that_is_not_part_of_the_text() {
    use crate::state::{todo_lines, ToolStatus};
    let lines = todo_lines(&[("uno", ToolStatus::Success), ("dos", ToolStatus::Running)]);
    let mut t = Transcript::new();
    t.push_lines(lines);
    let out = buffer(&t, 40, 4);
    assert!(out.contains("☑ uno"), "falta la tarea hecha");
    assert!(out.contains("◐ dos"), "falta la tarea en curso");
    assert!(!t.line(1).unwrap().plain().contains('☑'));
    assert!(!t.line(2).unwrap().plain().contains('◐'));
}

#[test]
fn job_lines_paint_their_prefix_that_is_not_part_of_the_text() {
    assert_eq!(prefix(LineKind::Job), "job: ");
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Job, "[Job 3 watch] listo"));
    let out = buffer(&t, 40, 3);
    assert!(
        out.contains("job: [Job 3 watch] listo"),
        "falta el actor job"
    );
    assert!(!t.line(0).unwrap().plain().contains("job:"));
}

#[test]
fn job_lines_keep_their_prefix_under_every_preset_and_without_color() {
    use crate::state::Format;
    use crate::widgets::{ColorDepth, ThemePreset};
    let mut t = Transcript::new();
    t.push_entry(LineKind::Job, Format::Plain, "[Job 3 completed] hecho");
    let mut temas: Vec<Theme> = ThemePreset::ALL.into_iter().map(Theme::preset).collect();
    let mut sin_color = Theme::test();
    sin_color.depth = ColorDepth::NoColor;
    temas.push(sin_color);
    for (i, theme) in temas.iter().enumerate() {
        let mut term = Terminal::new(TestBackend::new(40, 3)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &t,
                &mut Viewport::new(),
                &mut Layout::new(),
                None,
                theme,
            )
        })
        .unwrap();
        let texto: String = term
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(texto.contains("job: [Job 3 completed]"), "tema {i} roto");
    }
}

#[test]
fn empty_transcript_does_not_panic() {
    let _ = buffer(&Transcript::new(), 40, 10);
}

#[test]
fn zero_and_tiny_sizes_do_not_panic() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Assistant, "x"));
    let _ = buffer(&t, 0, 0);
    let _ = buffer(&t, 1, 1);
}

#[test]
fn wraps_a_long_unbroken_token_without_panicking() {
    let mut t = Transcript::new();
    let url = "https://example.com/".to_string() + &"a".repeat(300);
    t.push(Line::new(LineKind::Assistant, url));
    let out = buffer(&t, 20, 5);
    // No se desborda: no hay panico y el buffer tiene la altura pedida.
    assert_eq!(out.chars().count(), 20 * 5);
}

#[test]
fn truncates_on_display_width_not_bytes() {
    // CJK ocupa 2 columnas cada uno; ASCII 1.
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Assistant, "日本語 テキスト 🚀 hola"));
    let _ = buffer(&t, 12, 4); // no debe entrar en panico por anchura
}
