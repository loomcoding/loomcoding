use super::*;

#[test]
fn model_prices_match_provider_by_url_ignoring_trailing_slash() {
    // La `api` del catálogo lleva barra final; los ajustes, no. Debe casar en ambos sentidos.
    assert_eq!(
        priced_app("priced", "https://acme.example/v1").model_prices(),
        Some((3.0, 15.0))
    );
    assert_eq!(
        priced_app("priced", "https://acme.example/v1/").model_prices(),
        Some((3.0, 15.0))
    );
}

#[test]
fn model_prices_none_without_catalog() {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "priced".into(),
        base_url: "https://acme.example/v1".into(),
        api_key_env: "ACME_KEY".into(),
        effort: None,
    });
    assert_eq!(app.model_prices(), None, "sin catálogo no hay precio");
}

/// App con el catálogo de ventanas y el proveedor de la URL de muestra, en el modelo indicado.
fn metered_app(model: &str) -> App {
    let mut app = App::new();
    app.set_catalog(windowed_sample_catalog());
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: model.into(),
        base_url: "https://acme.example/v1".into(),
        api_key_env: "ACME_KEY".into(),
        effort: None,
    });
    app
}

#[test]
fn child_skill_load_status_is_kept_for_the_capability_widget() {
    let mut app = App::new();
    let status = loomcoding_core::skills::SkillLoadStatus {
        id: "review".into(),
        name: "Review".into(),
        state: loomcoding_core::skills::SkillLoadState::Loaded,
        source: Some("project".into()),
        input_tokens_per_request: 12,
    };
    app.apply_child_event(
        SessionId(7),
        LiveEvent::SkillsLoaded {
            skills: vec![status.clone()],
        },
    );
    assert_eq!(app.skill_statuses_by_child.get(&7), Some(&vec![status]));
}

#[test]
fn child_usage_progress_records_mcp_and_skill_components() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply_child_event(
        SessionId(7),
        LiveEvent::UsageProgress {
            request_id: 3,
            model: "m".into(),
            provider_usage: Some(Usage {
                input_tokens: 120,
                output_tokens: 8,
                reported_cost_usd: None,
            }),
            provider_final: true,
            estimated_usage: Usage {
                input_tokens: 130,
                output_tokens: 9,
                reported_cost_usd: None,
            },
            finished: true,
            components: vec![
                loomcoding_core::session::UsageComponent {
                    kind: loomcoding_core::session::UsageComponentKind::Mcp,
                    id: "token-demo".into(),
                    input_tokens: 24,
                },
                loomcoding_core::session::UsageComponent {
                    kind: loomcoding_core::session::UsageComponentKind::Skill,
                    id: "token-counter-demo".into(),
                    input_tokens: 16,
                },
            ],
        },
    );
    assert_eq!(app.usage_records.len(), 1);
    assert_eq!(app.usage_records[0].session, SessionId(7));
    assert_eq!(app.usage_records[0].components.len(), 2);
    assert_eq!(app.usage_records[0].components[0].id, "token-demo");
    assert_eq!(app.usage_records[0].components[1].id, "token-counter-demo");
}

#[test]
fn usage_progress_replaces_estimate_with_provider_snapshot_once() {
    let mut app = App::new();
    let session = SessionId(1);
    app.session_id = Some(session);
    let progress =
        |provider_usage, provider_final, estimated_usage, finished| LiveEvent::UsageProgress {
            request_id: 44,
            model: "m".into(),
            provider_usage,
            provider_final,
            estimated_usage,
            finished,
            components: vec![loomcoding_core::session::UsageComponent {
                kind: loomcoding_core::session::UsageComponentKind::Skill,
                id: "review".into(),
                input_tokens: 20,
            }],
        };
    app.apply(progress(
        None,
        false,
        Usage {
            input_tokens: 100,
            output_tokens: 20,
            reported_cost_usd: None,
        },
        false,
    ));
    assert_eq!((app.turn_input_tokens, app.turn_output_tokens), (100, 20));
    app.apply(progress(
        Some(Usage {
            input_tokens: 90,
            output_tokens: 10,
            reported_cost_usd: Some(0.1),
        }),
        true,
        Usage {
            input_tokens: 100,
            output_tokens: 20,
            reported_cost_usd: None,
        },
        true,
    ));
    assert_eq!((app.turn_input_tokens, app.turn_output_tokens), (90, 10));
    assert_eq!(app.usage_records.len(), 1);
    assert_eq!(app.usage_records[0].source, UsageSource::Provider);
    assert_eq!(app.usage_records[0].components[0].id, "review");
    assert_eq!((app.family_tokens().0, app.family_tokens().1), (90, 10));
}

#[test]
fn usage_event_sets_the_context_tokens() {
    let mut app = App::new();
    app.apply(LiveEvent::Usage {
        input_tokens: 1_200,
        output_tokens: 30,
    });
    assert_eq!(app.context_tokens, 1_200);
    app.apply(LiveEvent::Usage {
        input_tokens: 5_000,
        output_tokens: 10,
    });
    assert_eq!(app.context_tokens, 5_000, "el último uso gana");
    app.apply(LiveEvent::Usage {
        input_tokens: 0,
        output_tokens: 4,
    });
    assert_eq!(
        app.context_tokens, 5_000,
        "un uso sin entrada no pisa el tamaño"
    );
}

#[test]
fn compacted_event_sets_the_memory_estimate() {
    let mut app = App::new();
    app.apply(LiveEvent::ContextCompacted {
        replaced: 4,
        tokens_before: 90_000,
        summary_bytes: 100,
        activation: loomcoding_core::event::Activation::Sync, // ADAPTADO: campo nuevo
    });
    assert_eq!(app.context_memory, 25, "100 bytes son 25 tokens");
    app.apply(LiveEvent::ContextCompacted {
        replaced: 6,
        tokens_before: 91_000,
        summary_bytes: 101,
        activation: loomcoding_core::event::Activation::Sync, // ADAPTADO: campo nuevo
    });
    assert_eq!(app.context_memory, 26, "el redondeo sube");
}

#[test]
fn compaction_usage_counts_but_does_not_move_the_meter() {
    let mut app = App::new();
    app.apply(LiveEvent::Usage {
        input_tokens: 1_200,
        output_tokens: 30,
    });
    app.apply(LiveEvent::CompactionUsage {
        input_tokens: 700,
        output_tokens: 90,
    });
    assert_eq!(
        app.context_tokens, 1_200,
        "el consumo del resumen no mueve el medidor"
    );
    assert_eq!((app.input_tokens, app.output_tokens), (1_900, 120));
    assert_eq!(
        (app.turn_input_tokens, app.turn_output_tokens),
        (1_900, 120)
    );
    assert_eq!(
        app.usage_records.len(),
        2,
        "el consumo del resumen se apunta"
    );
}

#[test]
fn restore_messages_recovers_context_tokens_and_memory() {
    let mut app = App::new();
    let mut antes = Message::assistant(vec![ContentPart::Text {
        text: "antes".into(),
    }]);
    antes.usage = Some(loomcoding_core::message::Usage {
        input_tokens: 80_000,
        output_tokens: 5,
        reported_cost_usd: None,
    });
    let mut ultimo = Message::assistant(vec![ContentPart::Text {
        text: "ahora".into(),
    }]);
    ultimo.usage = Some(loomcoding_core::message::Usage {
        input_tokens: 120_000,
        output_tokens: 9,
        reported_cost_usd: None,
    });
    let resumen = loomcoding_core::agent::compact::summary_message(3, &"x".repeat(400));
    app.restore_messages(vec![resumen, Message::user("hola"), antes, ultimo]);
    assert_eq!(
        app.context_tokens, 120_000,
        "gana el último asistente con uso"
    );
    assert_eq!(
        app.context_memory, 100,
        "400 bytes del cuerpo son 100 tokens"
    );

    app.restore_messages(vec![Message::user("sin nada")]);
    assert_eq!((app.context_tokens, app.context_memory), (0, 0));
}

#[test]
fn child_usage_does_not_move_the_parent_meter() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::Usage {
        input_tokens: 2_000,
        output_tokens: 1,
    });
    app.apply(LiveEvent::ChildEvent {
        parent: SessionId(1),
        child: SessionId(9),
        event: Box::new(LiveEvent::Usage {
            input_tokens: 90_000,
            output_tokens: 7,
        }),
    });
    app.apply(LiveEvent::ChildEvent {
        parent: SessionId(1),
        child: SessionId(9),
        event: Box::new(LiveEvent::ContextCompacted {
            replaced: 2,
            tokens_before: 90_000,
            summary_bytes: 400,
            activation: loomcoding_core::event::Activation::Sync, // ADAPTADO: campo nuevo
        }),
    });
    assert_eq!(app.context_tokens, 2_000);
    assert_eq!(app.context_memory, 0);
}

#[test]
fn unknown_window_shows_no_bar_but_the_count() {
    use loomcoding_cloth::widgets::context_meter::{meter_text, MeterStyle};
    let mut app = metered_app("bare");
    app.apply(LiveEvent::Usage {
        input_tokens: 42_000,
        output_tokens: 1,
    });
    let meter = app.context_meter();
    assert_eq!(meter.window, None);
    assert_eq!(
        meter_text(&meter, MeterStyle::Segments),
        "ctx 42k (sin ventana)"
    );

    let sin_catalogo = {
        let mut app = App::new();
        app.apply(LiveEvent::Usage {
            input_tokens: 42_000,
            output_tokens: 1,
        });
        app.context_meter()
    };
    assert_eq!(sin_catalogo.window, None);
}

#[test]
fn meter_window_follows_the_model_and_the_catalog() {
    let mut app = metered_app("windowed");
    app.apply(LiveEvent::Usage {
        input_tokens: 142_000,
        output_tokens: 1,
    });
    assert_eq!(app.context_meter().window, Some(200_000));
    assert_eq!(app.context_meter().percent(), Some(71));

    // Cambiar de modelo recalcula la ventana (la caché no se queda con la anterior).
    app.model = "bare".into();
    assert_eq!(app.context_meter().window, None);
    app.model = "windowed".into();
    assert_eq!(app.context_meter().window, Some(200_000));

    // Un catálogo nuevo, aunque sea igual, también se vuelve a consultar.
    app.set_catalog(windowed_sample_catalog());
    assert_eq!(app.context_meter().window, Some(200_000));
}

#[test]
fn model_prices_none_when_a_price_is_missing() {
    let url = "https://acme.example/v1";
    assert_eq!(priced_app("half", url).model_prices(), None);
    assert_eq!(priced_app("plain", url).model_prices(), None);
    assert_eq!(priced_app("ausente", url).model_prices(), None);
}

#[test]
fn a_declared_window_is_what_the_footer_meter_uses() {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "qwen".into(),
        base_url: "http://127.0.0.1:8091/v1".into(),
        api_key_env: "K".into(),
        effort: None,
    });
    app.model = "qwen".into();
    assert_eq!(app.context_window(), None, "sin declaración ni catálogo");
    app.set_catalog(Arc::new(
        loomcoding_core::catalog::Catalog::default().with_window_override(
            "http://127.0.0.1:8091/v1",
            "qwen",
            35_584,
        ),
    ));
    assert_eq!(app.context_window(), Some(35_584));
}
