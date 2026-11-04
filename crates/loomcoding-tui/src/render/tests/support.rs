use super::*;

pub(super) fn record_with_cache(read: u64, write: u64) -> loomcoding_core::session::UsageRecord {
    loomcoding_core::session::UsageRecord {
        session: loomcoding_core::session::SessionId(1),
        model: "m".into(),
        input_tokens: 1,
        output_tokens: 1,
        input_price: None,
        output_price: None,
        reported_cost_usd: None,
        source: loomcoding_core::message::UsageSource::Provider,
        is_final: true,
        components: Vec::new(),
        cache_read_tokens: read,
        cache_write_tokens: write,
        created_at_ms: 0,
    }
}

pub(super) fn buffer_of(app: &mut App, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    term.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

/// Aplicación con el look Crush (layout `ContextRail`, chrome `Flat`).
pub(super) fn crush_ctx() -> App {
    let mut app = App::with_context("/proj", "mistral-nemo");
    app.theme_preset = loomcoding_cloth::widgets::ThemePreset::Crush;
    app.theme =
        loomcoding_cloth::widgets::Theme::preset(loomcoding_cloth::widgets::ThemePreset::Crush);
    app
}

/// Renderiza y devuelve el terminal, para inspeccionar celdas concretas.
pub(super) fn terminal_of(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    term
}

/// Filas `[inicio, fin)` de un bloque, según el diseño del último fotograma.
pub(super) fn block_rows(app: &App, entry: usize) -> (usize, usize) {
    let first = app.transcript.entry_start(entry);
    let start = app.layout.row_of(first);
    let end = app
        .layout
        .row_of(first + app.transcript.entry_lines(entry).len());
    (start, end)
}
