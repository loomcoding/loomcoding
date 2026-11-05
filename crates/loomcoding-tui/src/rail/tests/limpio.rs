//! Tests: rail limpio (marco sin borde, títulos en negrita y bloque `Session`).

use super::support::*;
use super::*;
use loomcoding_core::event::LiveEvent;
use loomcoding_core::todo::{TodoItem, TodoStatus};
use ratatui::style::Modifier;

/// Rail con ficheros, herramienta, job, subagente y tareas de la sesión 1.
fn rail_con_todo() -> App {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::FileDiff {
        path: "src/main.rs".into(),
        added: 3,
        removed: 1,
    });
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![TodoItem {
            content: "Pintar el rail".into(),
            status: TodoStatus::Pending,
        }],
    });
    app.subagents.insert(9, running(9, "review"));
    app
}

#[test]
fn rail_has_no_left_border() {
    let app = rail_con_todo();
    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter().any(|row| row.contains("review")),
        "el rail no pinta su contenido: {rows:?}"
    );
    assert!(
        rows.iter().all(|row| !row.starts_with('│')),
        "el rail sigue dibujando un borde izquierdo: {rows:?}"
    );
}

#[test]
fn titles_are_bold_and_not_uppercase() {
    let app = rail_con_todo();
    let rows = rows_of(&app, 40, 24);
    for mayusculas in [
        "FILES",
        "TOOLS",
        "JOBS",
        "TODOS",
        "SUBAGENTS",
        "CAPABILITIES",
    ] {
        assert!(
            !rows.iter().any(|row| row.contains(mayusculas)),
            "sigue el título {mayusculas} en mayúsculas: {rows:?}"
        );
    }
    for titulo in ["Files", "Tools", "Agents", "Todos"] {
        assert!(
            rows.iter().any(|row| row.contains(titulo)),
            "falta el título {titulo}: {rows:?}"
        );
    }
    let negrita = app.theme.styles.default.add_modifier(Modifier::BOLD);
    for inicio in ["Files", "Tools", "Agents", "Todos"] {
        assert_eq!(
            estilo_de(&app, inicio),
            negrita,
            "el título {inicio} no va en negrita con el estilo por defecto"
        );
    }
}

#[test]
fn session_block_replaces_model_block() {
    let mut app = App::with_context("/proj", "mistral-nemo");
    app.session_id = Some(SessionId(1));
    app.phase = "thinking".into();
    app.input_tokens = 1200;
    app.output_tokens = 300;

    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter().any(|row| row.contains("Session")),
        "falta el bloque Session: {rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("thinking")),
        "falta la fase: {rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("↑1200 ↓300")),
        "faltan los tokens: {rows:?}"
    );
    for prohibido in ["LOOMCODING", "MODEL", "/proj", "mistral-nemo"] {
        assert!(
            !rows.iter().any(|row| row.contains(prohibido)),
            "el rail aún muestra {prohibido:?}: {rows:?}"
        );
    }
}

#[test]
fn tokens_block_moves_out_of_session() {
    let mut app = App::with_context("/proj", "mistral-nemo");
    app.session_id = Some(SessionId(1));
    app.phase = "thinking".into();
    app.input_tokens = 1200;
    app.output_tokens = 300;

    let rows = rows_of(&app, 40, 24);
    let idx_session = rows
        .iter()
        .position(|row| row.contains("Session"))
        .expect("falta el bloque Session");
    let idx_fase = rows
        .iter()
        .position(|row| row.contains("thinking"))
        .expect("falta la fase");
    let idx_tokens = rows
        .iter()
        .position(|row| row.contains("Tokens"))
        .expect("falta el bloque Tokens");
    assert!(
        idx_session < idx_fase && idx_fase < idx_tokens,
        "Tokens no va justo tras Session: {rows:?}"
    );
    assert!(
        rows[idx_session..idx_tokens]
            .iter()
            .all(|row| !row.contains('↑')),
        "las cifras de tokens siguen bajo Session: {rows:?}"
    );
    assert!(
        rows[idx_tokens + 1].contains("↑1200 ↓300"),
        "la cifra de tokens no va bajo el título Tokens: {:?}",
        rows[idx_tokens + 1]
    );
    assert_eq!(
        estilo_de(&app, "Tokens"),
        app.theme.styles.default.add_modifier(Modifier::BOLD),
        "el título Tokens no va en negrita"
    );
}

#[test]
fn empty_tokens_block_is_not_drawn() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let rows = rows_of(&app, 40, 24);
    assert!(
        !rows.iter().any(|row| row.contains("Tokens")),
        "sin tokens ni coste se pinta el bloque Tokens: {rows:?}"
    );
    assert!(
        !rows.iter().any(|row| row.contains('↑')),
        "sin tokens aparece una cifra ↑: {rows:?}"
    );
}

#[test]
fn empty_blocks_draw_no_title() {
    let app = App::new();
    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter().any(|row| row.contains("Session")),
        "con la sesión vacía falta Session: {rows:?}"
    );
    for titulo in ["Files", "Tools", "Jobs", "Todos", "Agents", "Capabilities"] {
        assert!(
            !rows.iter().any(|row| row.contains(titulo)),
            "un bloque vacío pinta su título {titulo}: {rows:?}"
        );
    }
}
