use super::*;

/// Árbol raíz(1) → hijo(9) → nieto(10) → bisnieto(11), con etiquetas `hijo-N`.
fn arbol_de_tres() -> BTreeMap<i64, SubagentDescriptor> {
    [
        child_descriptor(9, TaskState::Working, SessionId(1)),
        child_descriptor(10, TaskState::Working, SessionId(9)),
        child_descriptor(11, TaskState::Working, SessionId(10)),
    ]
    .into_iter()
    .map(|d| (d.child.0, d))
    .collect()
}

#[test]
fn depth_and_path_follow_the_parent_chain_up_to_the_root() {
    let arbol = arbol_de_tres();
    let raiz = Some(SessionId(1));
    assert_eq!(
        profundidad_y_camino(&arbol, raiz, 9),
        Some((1, vec!["hijo-9".to_string()]))
    );
    assert_eq!(
        profundidad_y_camino(&arbol, raiz, 11),
        Some((
            3,
            vec![
                "hijo-9".to_string(),
                "hijo-10".to_string(),
                "hijo-11".to_string()
            ]
        ))
    );
}

#[test]
fn a_node_with_an_unknown_parent_has_no_position() {
    let mut arbol = arbol_de_tres();
    arbol.remove(&10);
    assert_eq!(profundidad_y_camino(&arbol, Some(SessionId(1)), 11), None);
    assert_eq!(profundidad_y_camino(&arbol, None, 9), None);
}

#[test]
fn a_path_is_truncated_from_the_left_keeping_the_last_actor() {
    let camino = vec!["coordinador".to_string(), "revisor".to_string()];
    assert_eq!(etiqueta_de_camino(&camino, 48), "coordinador › revisor");
    let corto = etiqueta_de_camino(&camino, 12);
    assert!(
        corto.starts_with('…') && corto.ends_with("revisor"),
        "{corto}"
    );
    assert_eq!(corto.chars().count(), 12);
}

#[test]
fn a_grandchild_shows_under_its_parent_with_indent_path_and_exact_state() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        9,
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        10,
        TaskState::InputRequired,
        SessionId(9),
    )));
    let lineas = texts(&app);
    assert!(
        lineas
            .iter()
            .any(|t| t.starts_with("  ▾") && t.contains("hijo-9 › hijo-10 (input-required)")),
        "{lineas:?}"
    );
    assert!(
        lineas
            .iter()
            .any(|t| t.starts_with("▾") && t.contains("hijo-9 (working)")),
        "{lineas:?}"
    );
}

#[test]
fn an_orphan_descriptor_is_dropped() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        10,
        TaskState::Working,
        SessionId(9),
    )));
    assert!(app.subagents.is_empty());
}

#[test]
fn grandchild_child_events_feed_its_stats_but_do_not_wake_the_root() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        9,
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        10,
        TaskState::Working,
        SessionId(9),
    )));
    app.apply(child_event(SessionId(9), 10, LiveEvent::TurnStarted));
    assert!(app.agent_stats[&10].started_at.is_some());
    app.apply(LiveEvent::SubagentSettled {
        parent: SessionId(9),
        child: SessionId(10),
        status: TaskState::Completed,
        summary: None,
    });
    assert!(!app.wake_pending);
}

#[test]
fn the_rail_only_walks_direct_children() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        9,
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        10,
        TaskState::Working,
        SessionId(9),
    )));
    app.rail_move(1);
    app.rail_move(1);
    assert_eq!(app.rail_selected, Some(9));
    assert!(!app.open_child(10));
}

#[test]
fn child_events_feed_the_stats() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));

    app.apply(child_event(SessionId(1), 9, LiveEvent::TurnStarted));
    let stats = &app.agent_stats[&9];
    assert!(stats.started_at.is_some());
    assert!(stats.finished_at.is_none());

    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::StepStarted {
            step: 2,
            max_steps: 5,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolStarted {
            call_id: "c1".into(),
            name: "glob".into(),
        },
    ));
    let stats = &app.agent_stats[&9];
    assert_eq!(stats.step, 2);
    assert_eq!(stats.max_steps, 5);
    assert_eq!(stats.tool.as_deref(), Some("glob"));

    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 100,
            output_tokens: 7,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolFinished {
            call_id: "c1".into(),
            is_error: false,
        },
    ));
    let stats = &app.agent_stats[&9];
    assert_eq!(stats.tool, None);
    assert_eq!(stats.input_tokens, 100);
    assert_eq!(stats.output_tokens, 7);

    app.apply(child_event(SessionId(1), 9, LiveEvent::TurnEnded));
    let stats = &app.agent_stats[&9];
    assert!(stats.finished_at.is_some());
    assert_eq!(stats.tool, None);
}

#[test]
fn child_stats_without_turn_start_get_a_start_time() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::StepStarted {
            step: 1,
            max_steps: 4,
        },
    ));
    let stats = &app.agent_stats[&9];
    assert!(stats.started_at.is_some());
    assert_eq!(stats.step, 1);
}

#[test]
fn child_usage_accumulates_and_does_not_touch_the_parents_counters() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.turn_input_tokens = 40;
    app.turn_output_tokens = 3;

    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 10,
            output_tokens: 2,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 5,
            output_tokens: 1,
        },
    ));

    let stats = &app.agent_stats[&9];
    assert_eq!((stats.input_tokens, stats.output_tokens), (15, 3));
    assert_eq!(app.turn_input_tokens, 40);
    assert_eq!(app.turn_output_tokens, 3);
    assert_eq!(app.input_tokens, 0);
    assert_eq!(app.output_tokens, 0);
}

#[test]
fn child_turn_restart_keeps_the_token_totals() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 10,
            output_tokens: 2,
        },
    ));
    app.apply(child_event(SessionId(1), 9, LiveEvent::TurnEnded));
    app.apply(child_event(SessionId(1), 9, LiveEvent::TurnStarted));

    let stats = &app.agent_stats[&9];
    assert_eq!((stats.input_tokens, stats.output_tokens), (10, 2));
    assert!(stats.started_at.is_some());
    assert!(stats.finished_at.is_none());
}

#[test]
fn child_events_of_another_session_are_ignored() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(SessionId(2), 9, LiveEvent::TurnStarted));
    app.apply(child_event(
        SessionId(2),
        9,
        LiveEvent::Usage {
            input_tokens: 10,
            output_tokens: 2,
        },
    ));
    assert!(app.agent_stats.is_empty());
    assert_eq!(app.hierarchy_tokens(), (0, 0));
}

#[test]
fn child_events_do_not_touch_transcript_or_status() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    // El turno del padre está en marcha cuando llegan los eventos de los hijos.
    app.apply(LiveEvent::TurnStarted);
    app.apply(LiveEvent::Delta {
        text: "respuesta".into(),
    });
    let status = app.status.clone();
    let in_flight = app.turn_in_flight;
    let entries = app.transcript.entry_count();
    let phase = app.phase.clone();

    app.apply(child_event(SessionId(1), 9, LiveEvent::TurnStarted));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolStarted {
            call_id: "c1".into(),
            name: "glob".into(),
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolFinished {
            call_id: "c1".into(),
            is_error: true,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Error {
            message: "fallo del hijo".into(),
        },
    ));
    app.apply(child_event(SessionId(1), 9, LiveEvent::TurnEnded));

    assert_eq!(app.status, status);
    assert_eq!(app.turn_in_flight, in_flight);
    assert_eq!(app.transcript.entry_count(), entries);
    assert_eq!(app.phase, phase);
}

/// Ficheros de un hijo tal como los ve la interfaz: ruta → (añadidas, quitadas).
fn child_files(app: &App, child: i64) -> Vec<(String, (usize, usize))> {
    app.files_by_session
        .get(&child)
        .map(|files| files.iter().map(|(p, c)| (p.clone(), *c)).collect())
        .unwrap_or_default()
}

#[test]
fn child_file_diff_accumulates_by_child_and_keeps_last_per_path() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));

    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::FileDiff {
            path: "a.rs".into(),
            added: 1,
            removed: 0,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::FileDiff {
            path: "b.rs".into(),
            added: 1,
            removed: 1,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::FileDiff {
            path: "a.rs".into(),
            added: 4,
            removed: 2,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        10,
        LiveEvent::FileDiff {
            path: "a.rs".into(),
            added: 7,
            removed: 7,
        },
    ));

    assert_eq!(
        child_files(&app, 9),
        vec![("a.rs".to_string(), (4, 2)), ("b.rs".to_string(), (1, 1))],
        "la última edición de cada ruta gana y las rutas se ordenan"
    );
    assert_eq!(child_files(&app, 10), vec![("a.rs".to_string(), (7, 7))]);
    assert_eq!(app.child_file_count(SessionId(9)), 2);
    assert_eq!(app.child_file_count(SessionId(42)), 0);
}

#[test]
fn child_file_diff_does_not_touch_parent_changes() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::FileDiff {
        path: "propio.rs".into(),
        added: 3,
        removed: 1,
    });
    let antes = app.changes.totals();

    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::FileDiff {
            path: "del_hijo.rs".into(),
            added: 5,
            removed: 5,
        },
    ));

    assert_eq!(app.changes.len(), 1, "el panel del padre no cambia");
    assert_eq!(app.changes.totals(), antes);
    assert_eq!(app.changes.iter().next().unwrap().path, "propio.rs");
}

#[test]
fn own_file_diff_is_recorded_for_the_open_session() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::FileDiff {
        path: "a.rs".into(),
        added: 1,
        removed: 0,
    });
    app.apply(LiveEvent::FileDiff {
        path: "a.rs".into(),
        added: 2,
        removed: 3,
    });

    assert_eq!(
        app.files_by_session
            .get(&1)
            .and_then(|files| files.get("a.rs")),
        Some(&(2, 3)),
        "el fichero propio queda anotado en la sesión abierta"
    );
    assert_eq!(app.changes.len(), 1);
}

#[test]
fn parent_files_return_to_changes_panel_after_child_roundtrip() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.subagents
        .insert(9, child_descriptor(9, TaskState::Working, SessionId(1)));
    app.apply(LiveEvent::FileDiff {
        path: "a.rs".into(),
        added: 1,
        removed: 0,
    });

    // Entrar en el hijo: su panel empieza vacío, sin los ficheros del padre.
    assert!(app.open_child(9));
    app.load_session(SessionId(9), Vec::new());
    assert!(
        app.changes.is_empty(),
        "el hijo no hereda los ficheros del padre"
    );

    // Mientras el hijo está abierto, sus propias ediciones llegan directas.
    app.apply(LiveEvent::FileDiff {
        path: "b.rs".into(),
        added: 2,
        removed: 0,
    });
    assert_eq!(app.changes.len(), 1);

    // Volver al padre: sus ficheros reaparecen y los del hijo no.
    assert!(app.go_parent());
    app.load_session(SessionId(1), Vec::new());
    let rutas: Vec<&str> = app.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(rutas, ["a.rs"], "el panel del padre recupera sus ficheros");
    assert!(
        app.files_by_session.contains_key(&9),
        "la navegación conserva al hijo"
    );
}

#[test]
fn loading_an_unrelated_session_clears_files_by_session() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::FileDiff {
        path: "a.rs".into(),
        added: 1,
        removed: 0,
    });
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::FileDiff {
            path: "b.rs".into(),
            added: 1,
            removed: 0,
        },
    ));

    app.load_session(SessionId(7), Vec::new());

    assert!(
        app.files_by_session.is_empty(),
        "la carga ajena vacía el mapa"
    );
    assert!(app.changes.is_empty());
}

#[test]
fn un_cambio_de_fase_actualiza_al_hijo_sin_tocar_su_estado_a2a() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        7,
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(LiveEvent::SubagentPhaseChanged {
        parent: SessionId(1),
        child: SessionId(7),
        phase: SubagentPhase::ToolRunning,
    });
    let hijo = &app.subagents[&7];
    assert_eq!(hijo.phase, SubagentPhase::ToolRunning);
    assert_eq!(hijo.status.state, TaskState::Working);
}

#[test]
fn una_fase_de_un_hijo_desconocido_no_hace_nada() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.apply(LiveEvent::SubagentPhaseChanged {
        parent: SessionId(1),
        child: SessionId(99),
        phase: SubagentPhase::Streaming,
    });
    assert!(app.subagents.is_empty());
}
