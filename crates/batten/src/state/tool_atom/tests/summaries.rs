//! Tests: summaries.

use super::support::*;
use super::*;

#[test]
fn bash_summary_is_the_command_truncated() {
    let item = ToolItem::new("c1", "bash");
    let lines = tool_lines_with(&item, &primary("cargo test -p batten"));
    assert_eq!(lines[0].plain(), "bash  cargo test -p batten");

    let lines = tool_lines_with(&item, &primary(&"x".repeat(200)));
    let summary = summary_of(&lines, "bash");
    assert_eq!(summary.chars().count(), 80);
    assert!(summary.ends_with('…'));
}

#[test]
fn view_summary_includes_range() {
    let item = ToolItem::new("c1", "view");
    let facts = ToolFacts {
        range: Some((10, 29)),
        ..primary("src/lib.rs")
    };
    let lines = tool_lines_with(&item, &facts);
    assert_eq!(lines[0].plain(), "view  src/lib.rs L10-29");

    let lines = tool_lines_with(&item, &primary("src/lib.rs"));
    assert_eq!(lines[0].plain(), "view  src/lib.rs");
}

#[test]
fn grep_shows_match_count_from_output() {
    let mut item = ToolItem::new("c1", "grep");
    item.set_result(false);
    item.set_output("a\nb\n\nc");
    let lines = tool_lines_with(&item, &primary("TODO"));
    let last = lines.last().unwrap();
    assert_eq!(last.plain(), "3 matches");
    assert_eq!(last.kind, LineKind::ToolBody);

    // Un recuento explícito de la pieza prevalece sobre las líneas de salida.
    let facts = ToolFacts {
        count: Some(7),
        ..primary("TODO")
    };
    let lines = tool_lines_with(&item, &facts);
    assert_eq!(lines.last().unwrap().plain(), "7 matches");
}

#[test]
fn grep_summary_includes_glob_filter() {
    let item = ToolItem::new("c1", "grep");
    let facts = ToolFacts {
        secondary: Some("*.rs".to_string()),
        ..primary("TODO")
    };
    assert_eq!(
        tool_lines_with(&item, &facts)[0].plain(),
        "grep  TODO in *.rs"
    );
}

#[test]
fn glob_shows_file_count() {
    let mut item = ToolItem::new("c1", "glob");
    item.set_result(false);
    item.set_output("a.rs\nb.rs\nc.rs");
    let lines = tool_lines_with(&item, &primary("crates/**/*.rs"));
    assert_eq!(lines[0].plain(), "glob  crates/**/*.rs");
    assert_eq!(lines.last().unwrap().plain(), "3 files");
}

#[test]
fn running_tool_without_output_has_no_count() {
    for name in ["grep", "glob"] {
        let mut item = ToolItem::new("c1", name);
        item.start();
        let lines = tool_lines_with(&item, &primary("TODO"));
        assert_eq!(lines.len(), 1, "{name} en curso no debe mostrar recuento");
        assert_eq!(lines[0].kind, LineKind::ToolHead(ToolStatus::Running));
    }
}

#[test]
fn long_command_with_newlines_and_wide_chars_is_truncated_by_graphemes() {
    let item = ToolItem::new("c1", "bash");
    // Primera línea de caracteres anchos (dos columnas cada uno); el resto se oculta.
    let wide = format!("{}\nsegunda línea", "日".repeat(120));
    let lines = tool_lines_with(&item, &primary(&wide));
    let summary = summary_of(&lines, "bash");
    assert_eq!(summary.graphemes(true).count(), 80);
    assert!(summary.ends_with('…'));
    assert!(!summary.contains("segunda"));

    // Un grafema compuesto (e con acento combinado) cuenta como uno solo.
    let combining = "e\u{301}".repeat(100);
    let lines = tool_lines_with(&item, &primary(&combining));
    assert_eq!(
        summary_of(&lines, "bash"),
        format!("{}…", "e\u{301}".repeat(79))
    );
}

#[test]
fn error_status_keeps_the_output_body() {
    let mut item = ToolItem::new("c1", "bash");
    item.set_result(true);
    item.set_output("x\ny\nz");
    let lines = tool_lines_with(&item, &primary("false"));
    assert_eq!(lines[0].kind, LineKind::ToolHead(ToolStatus::Error));
    let body = lines
        .iter()
        .filter(|l| l.kind == LineKind::ToolBody)
        .count();
    assert_eq!(body, 2);
    assert_eq!(lines.last().unwrap().kind, LineKind::ToolMore);
}

#[test]
fn ask_user_shows_first_question_and_option_count() {
    let item = ToolItem::new("c1", "ask_user");
    let lines = tool_lines_with(&item, &ask_facts("¿Qué base de datos usamos?", 3));
    assert_eq!(
        lines[0].plain(),
        "ask_user  ¿Qué base de datos usamos?  3 options"
    );

    // La pregunta se trunca a 80 grafemas; el recuento va detrás.
    let lines = tool_lines_with(&item, &ask_facts(&"q".repeat(200), 2));
    assert_eq!(
        summary_of(&lines, "ask_user"),
        format!("{}…  2 options", "q".repeat(79))
    );

    // Sin opciones conocidas no hay recuento.
    let lines = tool_lines_with(&item, &primary("¿Seguimos?"));
    assert_eq!(lines[0].plain(), "ask_user  ¿Seguimos?");
}

#[test]
fn subagent_summary_is_the_label() {
    let mut item = ToolItem::new("c1", "subagent");
    let facts = primary("revisar los tests");
    assert_eq!(
        tool_lines_with(&item, &facts)[0].plain(),
        "subagent  revisar los tests"
    );

    // En curso y sin salida: solo la cabecera.
    item.start();
    assert_eq!(tool_lines_with(&item, &facts).len(), 1);

    // Con salida, el cuerpo va plegado como en las demás piezas.
    item.set_result(false);
    item.set_output("hecho");
    let lines = tool_lines_with(&item, &facts);
    assert_eq!(lines.last().unwrap().plain(), "hecho");

    let long = tool_lines_with(&ToolItem::new("c2", "subagent"), &primary(&"x".repeat(200)));
    let summary = summary_of(&long, "subagent");
    assert_eq!(summary.graphemes(true).count(), 80);
    assert!(summary.ends_with('…'));
}

#[test]
fn subagent_control_summary_has_action_and_target() {
    let item = ToolItem::new("c1", "subagent_control");
    let facts = ToolFacts {
        secondary: Some("2".to_string()),
        ..primary("interrupt")
    };
    assert_eq!(
        tool_lines_with(&item, &facts)[0].plain(),
        "subagent_control  interrupt 2"
    );
    assert_eq!(
        tool_lines_with(&item, &primary("interrupt"))[0].plain(),
        "subagent_control  interrupt"
    );
}

#[test]
fn list_tools_show_item_counts() {
    for name in ["subagent_list", "job_list"] {
        let mut item = ToolItem::new("c1", name);
        // En curso y sin salida no hay recuento.
        item.start();
        let lines = tool_lines_with(&item, &ToolFacts::default());
        assert_eq!(lines.len(), 1, "{name} en curso no debe mostrar recuento");
        assert_eq!(lines[0].plain(), name);

        // Con salida, las líneas no vacías cuentan como elementos.
        item.set_result(false);
        item.set_output("a\nb\n\nc");
        let lines = tool_lines_with(&item, &ToolFacts::default());
        assert_eq!(lines[0].plain(), format!("{name}  3 items"));

        // Un recuento explícito de la pieza prevalece sobre la salida.
        let facts = ToolFacts {
            count: Some(7),
            ..ToolFacts::default()
        };
        assert_eq!(
            tool_lines_with(&item, &facts)[0].plain(),
            format!("{name}  7 items")
        );
    }
}

#[test]
fn list_tools_do_not_guess_a_count_from_a_single_line_output() {
    for name in ["subagent_list", "job_list"] {
        let mut item = ToolItem::new("c1", name);
        item.start();
        item.set_result(false);
        // El núcleo devuelve un JSON compacto en una línea: no se infiere nada.
        item.set_output(r#"[{"id":1},{"id":2}]"#);
        let lines = tool_lines_with(&item, &ToolFacts::default());
        assert_eq!(lines[0].plain(), name, "{name} sin recuento inventado");
    }
}

#[test]
fn job_tools_show_command_or_id() {
    let start = ToolItem::new("c1", "job_start");
    let lines = tool_lines_with(&start, &primary("cargo test -p batten\nsegunda"));
    assert_eq!(lines[0].plain(), "job_start  cargo test -p batten");

    for name in ["job_output", "job_kill"] {
        let item = ToolItem::new("c1", name);
        assert_eq!(
            tool_lines_with(&item, &primary("job-7"))[0].plain(),
            format!("{name}  job-7")
        );
    }
}
