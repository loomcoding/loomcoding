//! Pruebas del menú izquierdo del layout `Workspace` (tarea 2 del plan): bloques sin título en
//! mayúsculas, sin los que valen cero y con una fila en blanco entre ellos.
//!
//! Cabe señalar que el menú se prueba con `App::sidebar()` y, para el pintado, con el mismo
//! `sidebar::render` que usa la TUI sobre un `TestBackend`.

use super::support::child_descriptor;
use super::*;
use loomcoding_cloth::state::ToolItem;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Títulos de las secciones del menú, en orden.
fn titulos(app: &App) -> Vec<String> {
    app.sidebar()
        .sections
        .into_iter()
        .map(|s| s.title)
        .collect()
}

/// Líneas de la sección `titulo`, o vacío si el menú no la tiene.
fn lineas(app: &App, titulo: &str) -> Vec<String> {
    app.sidebar()
        .sections
        .into_iter()
        .find(|s| s.title == titulo)
        .map(|s| s.lines)
        .unwrap_or_default()
}

/// Líneas del primer bloque, el que no tiene título.
fn primer_bloque(app: &App) -> Vec<String> {
    app.sidebar()
        .sections
        .into_iter()
        .next()
        .map(|s| s.lines)
        .unwrap_or_default()
}

/// Filas del menú dibujado en un terminal de `w` por `h`, sin el borde izquierdo.
fn filas_del_menu(app: &App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| {
        loomcoding_cloth::widgets::sidebar::render(f, f.area(), &app.sidebar(), &app.theme)
    })
    .unwrap();
    let buf = term.backend().buffer();
    (0..h)
        .map(|y| {
            (1..w)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Añade una herramienta en marcha a la sesión.
fn herramienta_en_marcha(app: &mut App, nombre: &str) {
    let mut tool = ToolItem::new("call", nombre);
    tool.start();
    app.tools.push_back(tool);
}

#[test]
fn idle_menu_has_no_activity_or_session_blocks() {
    let app = App::with_context("/proj", "m");
    let titles = titulos(&app);
    assert!(
        !titles.iter().any(|t| t == "Actividad" || t == "Sesión"),
        "{titles:?}"
    );
    let filas = filas_del_menu(&app, 23, 40).join("\n");
    for palabra in ["AGENT", "FLOCK", "SESSION", "PROJECT"] {
        assert!(!filas.contains(palabra), "{palabra} en el menú:\n{filas}");
    }
}

#[test]
fn no_section_title_is_uppercase() {
    let mut app = App::with_context("/proj", "m");
    app.session_id = Some(SessionId(1));
    herramienta_en_marcha(&mut app, "bash");
    app.input_tokens = 12_300;
    let titles = titulos(&app);
    assert!(titles.iter().any(|t| t == "Actividad"), "{titles:?}");
    assert!(titles.iter().any(|t| t == "Sesión"), "{titles:?}");
    for titulo in titles.iter().filter(|t| !t.is_empty()) {
        assert_ne!(
            titulo,
            &titulo.to_uppercase(),
            "el título «{titulo}» va en mayúsculas"
        );
    }
}

#[test]
fn activity_lists_only_what_is_not_zero() {
    let mut app = App::with_context("/proj", "m");
    app.session_id = Some(SessionId(1));
    herramienta_en_marcha(&mut app, "bash");
    assert_eq!(
        lineas(&app, "Actividad"),
        vec!["◌ 1 herramienta".to_string()]
    );
}

#[test]
fn a_running_agent_names_its_a2a_state_in_parentheses() {
    let mut app = App::with_context("/proj", "m");
    app.session_id = Some(SessionId(1));
    app.subagents
        .insert(10, child_descriptor(10, TaskState::Working, SessionId(1)));
    let actividad = lineas(&app, "Actividad");
    assert!(
        actividad.contains(&"◐ 1 agente".to_string()),
        "{actividad:?}"
    );
    assert!(
        actividad.iter().any(|l| l == "(working 1)"),
        "{actividad:?}"
    );
}

/// Los tokens salen con `format_tokens`: de 10k en adelante, sin decimal (`12k`).
#[test]
fn session_shows_abbreviated_tokens() {
    let mut app = App::with_context("/proj", "m");
    app.input_tokens = 12_300;
    app.output_tokens = 1_100;
    assert_eq!(lineas(&app, "Sesión"), vec!["↑12k ↓1.1k".to_string()]);
}

#[test]
fn the_short_path_keeps_only_the_last_component() {
    let app = App::with_context("/home/andres/proyectos/loom-dashboard", "m");
    let bloque = primer_bloque(&app);
    assert!(
        bloque.contains(&"…/loom-dashboard".to_string()),
        "{bloque:?}"
    );
    assert!(
        !bloque.iter().any(|l| l.contains("proyectos")),
        "{bloque:?}"
    );

    let windows = App::with_context("G:\\temp\\loom-dashboard", "m");
    assert!(
        primer_bloque(&windows).contains(&"…/loom-dashboard".to_string()),
        "{:?}",
        primer_bloque(&windows)
    );
}

#[test]
fn blocks_are_separated_by_a_blank_row() {
    let mut app = App::with_context("/proj", "m");
    app.status = "working".into();
    app.session_id = Some(SessionId(1));
    herramienta_en_marcha(&mut app, "bash");
    app.input_tokens = 12_300;
    let filas = filas_del_menu(&app, 23, 40);
    let fila = |texto: &str| {
        filas
            .iter()
            .position(|f| f.contains(texto))
            .unwrap_or_else(|| panic!("falta «{texto}» en {filas:?}"))
    };
    assert_eq!(filas[fila("Actividad") - 1], "", "{filas:?}");
    assert_eq!(filas[fila("◌ 1 herramienta") + 1], "", "{filas:?}");
    assert_eq!(filas[fila("Sesión") - 1], "", "{filas:?}");
}

#[test]
fn the_first_block_has_glyph_status_model_and_effort_without_title() {
    let mut app = App::with_context("/proj", "Ornith-1.5-9B");
    app.status = "working".into();
    app.provider_settings = Some(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "Ornith-1.5-9B".into(),
        base_url: String::new(),
        api_key_env: String::new(),
        effort: Some("low".into()),
    });
    let sb = app.sidebar();
    assert_eq!(sb.sections[0].title, "");
    assert_eq!(
        sb.sections[0].lines,
        vec!["◐ working", "Ornith-1.5-9B · low", "…/proj"]
    );
}

#[test]
fn an_error_status_is_marked_as_an_error_line() {
    let mut app = App::with_context("/proj", "m");
    app.status = "error: tiempo agotado".into();
    let sb = app.sidebar();
    assert_eq!(sb.sections[0].lines[0], "◌ error: tiempo agotado");
    assert_eq!(sb.sections[0].error_lines, vec![0]);
}
