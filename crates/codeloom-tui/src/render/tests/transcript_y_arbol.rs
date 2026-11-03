use super::*;

/// Estilo de la primera celda del texto `aguja` en la pantalla pintada.
fn style_of_text(app: &mut App, w: u16, h: u16, aguja: &str) -> Style {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    let buffer = term.backend().buffer();
    for y in 0..h {
        let fila: String = (0..w)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect();
        if let Some(columna) = fila.find(aguja) {
            let x = fila[..columna].chars().count() as u16;
            return buffer[(x, y)].style();
        }
    }
    panic!("no aparece `{aguja}`");
}

#[test]
fn agents_overlay_marks_the_selected_row_like_the_other_pickers() {
    let registro = codeloom_core::registry::AgentRegistry::new(vec![Box::new(
        codeloom_core::registry::BuiltInSource,
    )]);
    let mut app = App::new();
    app.set_agent_registry(std::sync::Arc::new(registro));
    app.show_agents();
    let seleccionado = style_of_text(&mut app, 100, 30, "default: Agente genérico integrado; hereda modelo y esfuerzo del agente que delega [usable]");
    let realce = app.theme.styles.heading.add_modifier(Modifier::BOLD);

    // La fila con el foco se realza como en sesiones, ajustes y selectores: más viva que el
    // texto normal, nunca más apagada (el gris del borde se leía como «sin foco»).
    assert_eq!(seleccionado.fg, realce.fg);
    assert!(seleccionado.add_modifier.contains(Modifier::BOLD));
    assert_ne!(seleccionado.fg, app.theme.styles.border.fg);
}

/// Guías del carril de árbol de la fila de pantalla `y`: las celdas a la izquierda del texto.
fn gutter_at(term: &Terminal<TestBackend>, area: Rect, y: u16) -> String {
    (area.x - TREE_GUTTER..area.x)
        .map(|x| term.backend().buffer()[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn the_tree_gutter_is_dropped_when_there_is_no_room_for_it() {
    assert_eq!(tree_gutter(TREE_MIN_WIDTH - 1), 0);
    assert_eq!(tree_gutter(TREE_MIN_WIDTH), TREE_GUTTER);
}

/// Fila de pantalla de la fila `dentro` del bloque `entrada`, según el último fotograma.
fn screen_row(app: &App, area: Rect, entrada: usize, dentro: usize) -> u16 {
    let linea = app.transcript.entry_start(entrada);
    area.y + (app.layout.row_of(linea) + dentro - app.viewport.top()) as u16
}

#[test]
fn every_block_is_its_own_tree_root() {
    let mut app = App::new();
    app.push_user("hola");
    app.push_assistant(
        "uno
dos",
    );
    app.push_user("fin");
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");

    // Cada bloque abre y cierra su propia línea, con el glifo de su rol; ninguno cuelga de otro.
    let glifos = ['»', '≋', '»'];
    for (entrada, filas) in [(0, 2), (1, 3), (2, 2)] {
        assert_eq!(
            gutter_at(&term, area, screen_row(&app, area, entrada, 0)),
            format!("╭{} ", glifos[entrada])
        );
        for dentro in 1..filas - 1 {
            assert_eq!(
                gutter_at(&term, area, screen_row(&app, area, entrada, dentro)),
                "│  "
            );
        }
        assert_eq!(
            gutter_at(&term, area, screen_row(&app, area, entrada, filas - 1)),
            "╰─ "
        );
    }
}

#[test]
fn a_block_of_a_single_row_is_a_dash_with_its_glyph() {
    let mut app = App::new();
    app.push_user("hola");
    app.transcript.push_entry(
        batten::state::LineKind::System,
        batten::state::Format::Plain,
        "aviso",
    );
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");

    assert_eq!(gutter_at(&term, area, screen_row(&app, area, 1, 0)), "─◌ ");
}

#[test]
fn the_tree_guide_spans_every_wrapped_row_of_a_block() {
    let mut app = App::new();
    app.push_assistant(&"palabra ".repeat(60));
    app.push_user("fin");
    let term = terminal_of(&mut app, 60, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (start, end) = block_rows(&app, 0);

    assert!(end - start > 3, "el bloque debería ocupar varias filas");
    for row in start + 1..end - 1 {
        let y = area.y + (row - app.viewport.top()) as u16;
        assert_eq!(gutter_at(&term, area, y), "│  ", "fila {row}");
    }
    let y = area.y + (end - 1 - app.viewport.top()) as u16;
    assert_eq!(gutter_at(&term, area, y), "╰─ ");
}

#[test]
fn the_tree_follows_the_viewport_scroll() {
    let mut app = App::new();
    for i in 0..30 {
        app.push_user(&format!("pregunta {i}"));
    }
    let _ = terminal_of(&mut app, 100, 12);
    app.viewport.scroll_up(5);
    let term = terminal_of(&mut app, 100, 12);
    let area = app.transcript_area.expect("el transcript fija su área");

    for y in area.y..area.bottom() {
        let guia = gutter_at(&term, area, y);
        assert!(
            ["╭» ", "│  ", "╰─ ", "─» "].contains(&guia.as_str()),
            "fila {y}: «{guia}»"
        );
    }
}

/// El bloque seleccionado se ve entero: trazo grueso a lo largo de todas sus filas y con el
/// color de su rol, en cualquier estilo de pantalla (no solo en `Flat`).
fn assert_active_block_has_a_thick_line(mut app: App) {
    app.push_user("pregunta");
    app.push_assistant(
        "uno
dos
tres",
    );
    app.push_user("otra");
    let _ = terminal_of(&mut app, 100, 30);
    app.focus = Focus::Transcript;
    app.block = Some(1);
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (start, end) = block_rows(&app, 1);
    let success = app.theme.styles.success.fg;

    for row in start..end {
        let y = area.y + (row - app.viewport.top()) as u16;
        let esperado = if row == start {
            "┏≋ "
        } else if row + 1 == end {
            "┗━ "
        } else {
            "┃  "
        };
        assert_eq!(gutter_at(&term, area, y), esperado, "fila {row}");
        assert_eq!(
            term.backend().buffer()[(area.x - TREE_GUTTER, y)].fg,
            success.unwrap()
        );
    }
    // Los demás bloques siguen con el trazo fino.
    let y = area.y + (block_rows(&app, 0).0 - app.viewport.top()) as u16;
    assert_eq!(gutter_at(&term, area, y), "╭» ");
}

#[test]
fn the_active_block_gets_a_thick_line_in_the_default_chrome() {
    let app = App::new();
    assert_ne!(app.theme.look.chrome, ChromeStyle::Flat);
    assert_active_block_has_a_thick_line(app);
}

#[test]
fn the_active_block_gets_a_thick_line_in_flat_chrome() {
    assert_active_block_has_a_thick_line(crush_ctx());
}

#[test]
fn a_user_block_is_marked_with_the_accent_and_nothing_is_thick_without_focus() {
    let mut app = App::new();
    app.push_user("pregunta");
    app.push_assistant("respuesta");
    let _ = terminal_of(&mut app, 100, 30);
    app.block = Some(0);
    let sin_foco = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let y = screen_row(&app, area, 0, 0);
    assert_eq!(
        gutter_at(&sin_foco, area, y),
        "╭» ",
        "sin foco no hay trazo grueso"
    );

    app.focus = Focus::Transcript;
    let con_foco = terminal_of(&mut app, 100, 30);
    assert_eq!(gutter_at(&con_foco, area, y), "┏» ");
    assert_eq!(
        con_foco.backend().buffer()[(area.x - TREE_GUTTER, y)].fg,
        app.theme.styles.accent.fg.unwrap()
    );
}

#[test]
fn every_role_glyph_is_one_column_wide_and_distinct() {
    use batten::state::{DiffKind, LineKind, ToolStatus};
    let roles = [
        LineKind::User,
        LineKind::Assistant,
        LineKind::Thinking,
        LineKind::System,
        LineKind::Error,
        LineKind::Todo,
        LineKind::Plan,
        LineKind::Banner,
    ];
    let glifos: Vec<char> = roles
        .iter()
        .map(|k| tree_glyph(*k).expect("lleva glifo"))
        .collect();

    // Un glifo de otro ancho descuadraría el carril; dos iguales harían ilegible el rol.
    for g in &glifos {
        assert_eq!(
            Span::raw(g.to_string()).width(),
            1,
            "«{g}» no mide una columna"
        );
    }
    let mut unicos = glifos.clone();
    unicos.sort();
    unicos.dedup();
    assert_eq!(unicos.len(), glifos.len(), "glifos repetidos: {glifos:?}");
    // Las herramientas y los subagentes traen su propio icono de estado en el texto.
    assert_eq!(tree_glyph(LineKind::ToolHead(ToolStatus::Success)), None);
    assert_eq!(tree_glyph(LineKind::ToolDiff(DiffKind::Insert)), None);
}

fn finished_tool() -> batten::state::ToolItem {
    let mut tool = batten::state::ToolItem::new("call-1", "view");
    tool.set_summary(r#"{"path":"src/main.rs"}"#);
    tool.set_result(false);
    tool.set_output("fn main() {}");
    tool
}

#[test]
fn the_pinned_tool_card_goes_away_once_the_turn_is_over() {
    let mut app = App::new();
    app.tools.push_back(finished_tool());

    // Sin turno en curso y con la herramienta terminada, la tarjeta solo repetiría la
    // conversación: no se pinta ni reserva filas.
    let content = buffer_of(&mut app, 90, 20);
    assert!(!content.contains("OUT"), "{content}");
    assert!(!content.contains("DONE"), "{content}");

    // Mientras dura el turno, sigue mostrando la última herramienta aunque haya acabado.
    app.turn_in_flight = true;
    let content = buffer_of(&mut app, 90, 20);
    assert!(
        content.contains("OUT") && content.contains("DONE"),
        "{content}"
    );
}

#[test]
fn a_live_tool_keeps_its_card_even_without_a_turn() {
    let mut tool = batten::state::ToolItem::new("call-2", "bash");
    tool.start();
    let mut app = App::new();
    app.tools.push_back(tool);

    let content = buffer_of(&mut app, 90, 20);

    assert!(content.contains("RUNNING"), "{content}");
}

#[test]
fn the_composer_stays_put_when_the_tool_card_goes_away() {
    let mut app = App::new();
    app.turn_in_flight = true;
    app.tools.push_back(finished_tool());
    let _ = buffer_of(&mut app, 90, 24);
    let con_tarjeta = app.composer_area;
    let transcript_con = app.transcript_area.unwrap().height;

    app.turn_in_flight = false;
    let _ = buffer_of(&mut app, 90, 24);

    // El composer va anclado abajo: la tarjeta libera su sitio a la conversación.
    assert_eq!(app.composer_area, con_tarjeta);
    assert!(app.transcript_area.unwrap().height > transcript_con);
}

#[test]
fn help_overlay_lists_subagent_navigation() {
    let mut app = App::new();
    app.help_open = true;
    let content = buffer_of(&mut app, 80, 24);
    assert!(content.contains("Alt+←"), "falta Alt+←: {content}");
    assert!(content.contains("/parent"), "falta /parent: {content}");
    assert!(content.contains("↑/↓"), "falta ↑/↓ del rail: {content}");
    assert!(
        content.contains("Press F1 or Esc to close"),
        "la ayuda no cabe con las líneas nuevas: {content}"
    );
}

#[test]
fn live_tool_card_separates_status_arguments_and_output() {
    let mut tool = batten::state::ToolItem::new("call-1", "view");
    tool.set_summary(r#"{"path":"src/main.rs"}"#);
    tool.set_result(false);
    tool.set_output("fn main() {}");
    let mut app = App::new();
    app.turn_in_flight = true;
    app.tools.push_back(tool);
    let content = buffer_of(&mut app, 90, 20);
    assert!(content.contains("DONE"));
    assert!(content.contains("IN"));
    assert!(content.contains("OUT"));
    assert!(content.contains("fn main()"));
    assert!(content.contains("src/main.rs"));
}

#[test]
fn crush_shows_inline_tool_once_without_fixed_card() {
    let mut app = App::new();
    app.theme = batten::widgets::Theme::preset(batten::widgets::ThemePreset::Crush);
    app.apply(codeloom_core::event::LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    // Por debajo de 80 columnas no hay rail, que también lista la tool.
    let content = buffer_of(&mut app, 70, 20);
    assert_eq!(content.matches("● View").count(), 1);
    assert!(!content.contains("RUNNING"));
}

#[test]
fn crush_composer_keeps_cursor_inside_the_screen_when_wrapping() {
    let mut app = App::new();
    app.theme = batten::widgets::Theme::preset(batten::widgets::ThemePreset::Crush);
    app.composer.set_text(&"x".repeat(100));
    let backend = TestBackend::new(40, 20);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, &mut app)).unwrap();
    let cursor = term.backend_mut().get_cursor_position().unwrap();
    assert!(cursor.x < 40);
    assert!(cursor.y > 0 && cursor.y < 20);
}

#[test]
fn composer_grows_for_multiline_input_and_keeps_cursor_visible_when_limited() {
    let mut app = App::new();
    app.composer.set_text("first\nsecond\nthird");
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, &mut app)).unwrap();
    let content = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    let cursor = term.backend_mut().get_cursor_position().unwrap();
    assert!(content.contains("first"));
    assert!(content.contains("second"));
    assert!(content.contains("third"));
    assert!(cursor.y > 0);

    app.composer.set_text("one\ntwo\nthree\nfour\nfive");
    let backend = TestBackend::new(60, 12);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, &mut app)).unwrap();
    let content = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    let cursor = term.backend_mut().get_cursor_position().unwrap();
    assert!(content.contains("three"));
    assert!(content.contains("four"));
    assert!(content.contains("five"));
    assert!(cursor.y > 0);
}

#[test]
fn slash_menu_renders_filters_and_respects_overlays_and_small_sizes() {
    let mut app = App::new();
    app.composer.set_text("/");
    let content = buffer_of(&mut app, 100, 30);
    assert!(content.contains("/help"));
    assert!(content.contains("/sessions"));
    assert!(content.contains("Tab complete"));
    app.composer.set_text("/to");
    let content = buffer_of(&mut app, 100, 30);
    assert!(content.contains("/tools"));
    assert!(!content.contains("/sessions"));
    for (width, height) in [(0, 0), (1, 1), (20, 5), (40, 10), (120, 40)] {
        buffer_of(&mut app, width, height);
    }
    app.composer.set_text("/unknown");
    assert!(buffer_of(&mut app, 100, 30).contains("Unknown command"));
    app.composer.set_text("/new");
    app.turn_in_flight = true;
    crate::run::handle_key(
        &mut app,
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(buffer_of(&mut app, 100, 30).contains("Wait for the active turn"));
    app.help_open = true;
    let content = buffer_of(&mut app, 100, 30);
    assert!(!content.contains("Tab complete"));
    assert!(content.contains("Press F1 or Esc to close"));
}

#[test]
fn tool_inspector_shows_status_arguments_and_scrolls_full_output() {
    let mut app = App::new();
    app.tools_open = true;
    assert!(buffer_of(&mut app, 100, 30).contains("No tool calls yet"));
    let mut tool = batten::state::ToolItem::new("call", "view");
    tool.set_summary("{\"path\":\"sample.rs\"}");
    tool.set_result(true);
    tool.set_output(
        (0..40)
            .map(|index| format!("line-{index:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app.tools.push_back(tool);
    let top = buffer_of(&mut app, 100, 30);
    assert!(top.contains("Error"));
    assert!(top.contains("sample.rs"));
    assert!(top.contains("line-00"));
    assert!(!top.contains("line-39"));
    app.tool_scroll = (u16::MAX, u16::MAX);
    let bottom = buffer_of(&mut app, 100, 30);
    assert!(bottom.contains("line-39"));
    assert_eq!(app.tool_scroll.1, 0);
    for (width, height) in [(0, 0), (1, 1), (30, 8), (60, 20)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        assert!(!terminal.backend().cursor_visible());
    }
    assert!(buffer_of(&mut app, 30, 8).contains("enlarge"));
    app.tools[0].set_output(format!("{}THE-END", "x".repeat(200)));
    app.tool_scroll = (0, u16::MAX);
    assert!(buffer_of(&mut app, 100, 30).contains("THE-END"));
}
