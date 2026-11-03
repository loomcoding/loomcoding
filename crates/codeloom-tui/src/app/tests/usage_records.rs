use super::*;

/// Apunte de prueba con precios conocidos (3 y 15 por millón), como los que guarda el libro.
fn usage_record(model: &str, input: u64, output: u64) -> UsageRecord {
    UsageRecord {
        session: SessionId(1),
        model: model.into(),
        input_tokens: input,
        output_tokens: output,
        input_price: Some(3.0),
        output_price: Some(15.0),
        reported_cost_usd: None,
        source: UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: 1,
    }
}

#[test]
fn context_breakdown_is_kept_for_the_open_session_and_dropped_on_load() {
    let mut app = App::new();
    app.apply(LiveEvent::ContextBreakdown {
        request_id: 1,
        breakdown: codeloom_core::agent::compact::ContextBreakdown {
            system: 1,
            ..Default::default()
        },
    });
    assert_eq!(app.context_breakdown.map(|b| b.system), Some(1));
    app.load_session(SessionId(2), Vec::new());
    assert_eq!(app.context_breakdown, None);
}

#[test]
fn load_usage_replaces_records_for_the_open_session() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.usage_records = vec![usage_record("viejo", 1, 1)];
    let stored = vec![usage_record("priced", 10, 2), usage_record("priced", 20, 4)];
    app.load_usage(SessionId(1), stored.clone());
    assert_eq!(
        app.usage_records, stored,
        "la lista de la base de datos sustituye a la anterior"
    );
}

#[test]
fn load_usage_ignores_other_sessions() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let mine = vec![usage_record("priced", 10, 2)];
    app.load_usage(SessionId(1), mine.clone());
    app.load_usage(SessionId(2), vec![usage_record("ajena", 99, 99)]);
    assert_eq!(
        app.usage_records, mine,
        "una carga de otra sesión no se mezcla"
    );
}

#[test]
fn live_usage_appends_a_priced_record() {
    let mut app = priced_app("priced", "https://acme.example/v1");
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::Usage {
        input_tokens: 100,
        output_tokens: 10,
    });
    assert_eq!(app.usage_records.len(), 1);
    let record = &app.usage_records[0];
    assert_eq!(record.model, "priced");
    assert_eq!((record.input_tokens, record.output_tokens), (100, 10));
    assert_eq!(
        (record.input_price, record.output_price),
        (Some(3.0), Some(15.0))
    );
    assert!(record.created_at_ms > 0, "el apunte lleva marca de tiempo");
}

#[test]
fn live_child_usage_appends_a_record() {
    let mut app = priced_app("priced", "https://acme.example/v1");
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        7,
        LiveEvent::Usage {
            input_tokens: 50,
            output_tokens: 5,
        },
    ));
    assert_eq!(app.usage_records.len(), 1);
    assert_eq!(app.usage_records[0].model, "priced");
    assert_eq!(app.usage_records[0].input_tokens, 50);
    // Un hijo de otra sesión no entra en la sesión abierta.
    app.apply(child_event(
        SessionId(9),
        8,
        LiveEvent::Usage {
            input_tokens: 1,
            output_tokens: 1,
        },
    ));
    assert_eq!(
        app.usage_records.len(),
        1,
        "solo los hijos de la sesión abierta"
    );
}

#[test]
fn session_load_clears_usage_records() {
    let mut app = priced_app("priced", "https://acme.example/v1");
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::Usage {
        input_tokens: 1,
        output_tokens: 1,
    });
    app.load_session(SessionId(2), Vec::new());
    assert!(
        app.usage_records.is_empty(),
        "la sesión anterior no deja apuntes"
    );
}

#[test]
fn reload_does_not_duplicate_live_records() {
    let mut app = priced_app("priced", "https://acme.example/v1");
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::Usage {
        input_tokens: 100,
        output_tokens: 10,
    });
    // La base de datos ya contiene ese apunte: la recarga lo sustituye, no lo suma.
    let persisted = app.usage_records.clone();
    app.load_usage(SessionId(1), persisted.clone());
    assert_eq!(app.usage_records, persisted);
    assert_eq!(app.usage_records.len(), 1);
    // Un apunte vivo nuevo y una recarga posterior que ya lo incluye: sigue habiendo dos.
    app.apply(LiveEvent::Usage {
        input_tokens: 20,
        output_tokens: 2,
    });
    let persisted = app.usage_records.clone();
    app.load_usage(SessionId(1), persisted);
    assert_eq!(app.usage_records.len(), 2);
}

/// Apunte con precios `(entrada, salida)` por millón, o sin precio si `prices` es `None`.
fn priced_record(input: u64, output: u64, prices: Option<(f64, f64)>) -> UsageRecord {
    UsageRecord {
        session: SessionId(1),
        model: "m".into(),
        input_tokens: input,
        output_tokens: output,
        input_price: prices.map(|(price_in, _)| price_in),
        output_price: prices.map(|(_, price_out)| price_out),
        reported_cost_usd: None,
        source: UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        created_at_ms: 1,
    }
}

/// Apunte de un hijo concreto, con precios `(entrada, salida)` o sin ellos.
fn child_record(child: i64, input: u64, output: u64, prices: Option<(f64, f64)>) -> UsageRecord {
    UsageRecord {
        session: SessionId(child),
        ..priced_record(input, output, prices)
    }
}

#[test]
fn child_cost_sums_only_that_childs_records() {
    let mut app = App::new();
    app.usage_records = vec![
        child_record(9, 1_000_000, 0, Some((3.0, 15.0))),
        child_record(9, 0, 100_000, Some((3.0, 15.0))),
        child_record(10, 1_000_000, 0, Some((3.0, 15.0))),
    ];
    let (cost, partial) = app
        .child_cost(SessionId(9))
        .expect("el hijo 9 tiene apuntes con precio");
    // 3,00 de entrada más 1,50 de salida. Los apuntes del hijo 10 no entran.
    assert!((cost - 4.5).abs() < 1e-9, "coste inesperado: {cost}");
    assert!(!partial, "todos los apuntes del hijo tienen precio");
    assert_eq!(
        app.child_cost(SessionId(11)),
        None,
        "un hijo sin apuntes no tiene coste"
    );
}

#[test]
fn child_cost_none_without_prices() {
    let mut app = App::new();
    app.usage_records = vec![child_record(9, 1_000, 1_000, None)];
    assert_eq!(
        app.child_cost(SessionId(9)),
        None,
        "ningún apunte del hijo tiene precio"
    );
}

#[test]
fn child_cost_partial_marks_plus() {
    let mut app = App::new();
    app.usage_records = vec![
        child_record(9, 1_000_000, 0, Some((3.0, 15.0))),
        child_record(9, 1_000_000, 1_000_000, None),
    ];
    let (cost, partial) = app
        .child_cost(SessionId(9))
        .expect("hay un apunte con precio");
    assert!(
        (cost - 3.0).abs() < 1e-9,
        "solo suma el apunte con precio: {cost}"
    );
    assert!(
        partial,
        "un apunte sin precio marca el coste del hijo como parcial"
    );
}

#[test]
fn child_costs_plus_parent_equal_total() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.usage_records = vec![
        child_record(1, 1_000_000, 0, Some((3.0, 15.0))),
        child_record(9, 0, 100_000, Some((3.0, 15.0))),
        child_record(10, 1_000_000, 1_000_000, None),
        child_record(9, 1_000_000, 0, Some((2.0, 10.0))),
    ];
    let (total, partial) = app.estimated_cost().expect("hay apuntes");
    let mut suma = 0.0;
    for child in [9, 10] {
        if let Some((cost, _)) = app.child_cost(SessionId(child)) {
            suma += cost;
        }
    }
    suma += app
        .usage_records
        .iter()
        .filter(|record| record.session == SessionId(1))
        .filter_map(|record| record.cost())
        .sum::<f64>();
    assert!(
        (total - suma).abs() < 1e-9,
        "hijos más padre deben dar el total: {suma} frente a {total}"
    );
    assert!(partial, "el apunte sin precio del hijo 10 marca el total");
}

#[test]
fn live_child_usage_is_attributed_to_the_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 1_000,
            output_tokens: 10,
        },
    ));
    app.apply(LiveEvent::Usage {
        input_tokens: 2_000,
        output_tokens: 20,
    });
    let sesiones: Vec<SessionId> = app
        .usage_records
        .iter()
        .map(|record| record.session)
        .collect();
    assert_eq!(
        sesiones,
        vec![SessionId(9), SessionId(1)],
        "cada apunte vivo lleva la sesión que lo originó"
    );
}

#[test]
fn estimated_cost_sums_each_record_at_its_own_price() {
    // Cambio de modelo a mitad de sesión: cada apunte conserva su precio.
    let mut app = App::new();
    app.usage_records = vec![
        priced_record(1_000_000, 0, Some((3.0, 15.0))),
        priced_record(0, 100_000, Some((2.0, 10.0))),
    ];
    // Primero: 1.000.000 a 3 $/M = 3. Segundo: 100.000 a 10 $/M = 1.
    let (cost, partial) = app.estimated_cost().expect("hay apuntes");
    assert!((cost - 4.0).abs() < 1e-9, "coste inesperado: {cost}");
    assert!(!partial, "todos los apuntes tienen precio");
}

#[test]
fn estimated_cost_marks_partial_when_a_record_has_no_price() {
    let mut app = App::new();
    app.usage_records = vec![
        priced_record(1_000_000, 0, Some((3.0, 15.0))),
        priced_record(1_000_000, 1_000_000, None),
    ];
    let (cost, partial) = app.estimated_cost().expect("hay apuntes");
    assert!(
        (cost - 3.0).abs() < 1e-9,
        "solo suma los apuntes con precio: {cost}"
    );
    assert!(partial, "un apunte sin precio marca el total como parcial");
}

#[test]
fn estimated_cost_none_without_records() {
    let app = priced_app("priced", "https://acme.example/v1");
    assert_eq!(app.estimated_cost(), None, "sin apuntes no hay estimación");
}

#[test]
fn estimated_cost_adds_children() {
    let mut app = priced_app("priced", "https://acme.example/v1");
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::Usage {
        input_tokens: 1_000_000,
        output_tokens: 100_000,
    });
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 1_000_000,
            output_tokens: 0,
        },
    ));
    // Entrada: 2.000.000 a 3 $/M = 6. Salida: 100.000 a 15 $/M = 1,5.
    let (cost, partial) = app.estimated_cost().expect("hay apuntes");
    assert!((cost - 7.5).abs() < 1e-9, "coste inesperado: {cost}");
    assert!(!partial);
}

#[test]
fn hierarchy_tokens_sums_all_children() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Usage {
            input_tokens: 100,
            output_tokens: 10,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        10,
        LiveEvent::Usage {
            input_tokens: 20,
            output_tokens: 5,
        },
    ));
    // Un hijo de otra sesión no entra en el total.
    app.apply(child_event(
        SessionId(2),
        11,
        LiveEvent::Usage {
            input_tokens: 1000,
            output_tokens: 1000,
        },
    ));
    assert_eq!(app.hierarchy_tokens(), (120, 15));
}

#[test]
fn active_agents_counts_only_running_children_of_the_open_session() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.subagents
        .insert(9, child_descriptor(9, TaskState::Working, SessionId(1)));
    app.subagents
        .insert(10, child_descriptor(10, TaskState::Completed, SessionId(1)));
    app.subagents
        .insert(11, child_descriptor(11, TaskState::Working, SessionId(1)));
    app.subagents
        .insert(12, child_descriptor(12, TaskState::Working, SessionId(2)));
    assert_eq!(app.active_agents(), 2);
}

#[test]
fn child_log_records_text_and_tool_calls_and_results() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![
            ContentPart::Text {
                text: "voy a mirar".into(),
            },
            ContentPart::ToolCall {
                id: "c1".into(),
                name: "bash".into(),
                input: json!({ "command": "cargo test" }),
            },
        ])),
    ));
    // Un texto muy largo se recorta a 240 caracteres (no bytes): las vocales acentuadas
    // ocupan dos bytes en UTF-8 y no deben contar doble.
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
            text: "á".repeat(300),
        }])),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "c1".into(),
                content: "ok".into(),
                is_error: false,
            }],
        )),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::ToolCall {
            id: "c2".into(),
            name: "bash".into(),
            input: json!({ "command": "cargo build" }),
        }])),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolFinished {
            call_id: "c2".into(),
            is_error: true,
        },
    ));

    let items = app
        .child_logs
        .get(&9)
        .map(|log| log.items.clone())
        .unwrap_or_default();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], ChildItem::Text("voy a mirar".into()));
    assert_eq!(
        items[1],
        ChildItem::Tool {
            call_id: "c1".into(),
            name: "bash".into(),
            summary: Some("cargo test".into()),
            status: ToolStatus::Success,
        }
    );
    assert_eq!(items[2], ChildItem::Text("á".repeat(MAX_CHILD_TEXT)));
    assert_eq!(
        items[3],
        ChildItem::Tool {
            call_id: "c2".into(),
            name: "bash".into(),
            summary: Some("cargo build".into()),
            status: ToolStatus::Error,
        }
    );
    // El registro no crea métricas: estas siguen siendo las del ToolFinished.
    assert_eq!(app.agent_stats[&9].tool, None);
}

#[test]
fn child_live_text_deltas_become_one_chat_entry_when_message_arrives() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Delta {
            text: "voy ".into(),
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::Delta {
            text: "a mirar".into(),
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
            text: "voy a mirar".into(),
        }])),
    ));
    assert_eq!(
        app.child_logs[&9].items.iter().collect::<Vec<_>>(),
        [&ChildItem::Text("voy a mirar".into())]
    );
}

#[test]
fn child_log_is_bounded() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    for i in 0..MAX_CHILD_ITEMS + 5 {
        app.apply(child_event(
            SessionId(1),
            9,
            LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
                text: format!("mensaje {i}"),
            }])),
        ));
    }
    let items = app
        .child_logs
        .get(&9)
        .map(|log| log.items.clone())
        .unwrap_or_default();
    assert_eq!(items.len(), MAX_CHILD_ITEMS);
    // Se descartan los más antiguos: se conservan los últimos MAX_CHILD_ITEMS.
    assert_eq!(items.front(), Some(&ChildItem::Text("mensaje 5".into())));
    assert_eq!(
        items.back(),
        Some(&ChildItem::Text(format!("mensaje {}", MAX_CHILD_ITEMS + 4)))
    );
}

#[test]
fn child_log_ignores_user_messages_and_unknown_results() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::user("hola")),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "desconocida".into(),
                content: "nada".into(),
                is_error: false,
            }],
        )),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolFinished {
            call_id: "desconocida".into(),
            is_error: false,
        },
    ));
    // Ni el texto del usuario ni los resultados sin llamada previa dejan rastro.
    assert!(app
        .child_logs
        .get(&9)
        .is_none_or(|log| log.items.is_empty()));
}

#[test]
fn stats_are_cleared_on_session_load() {
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
    assert!(!app.agent_stats.is_empty());
    app.load_session(SessionId(2), Vec::new());
    assert!(app.agent_stats.is_empty());
}
