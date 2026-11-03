//! Uso de tokens, ventana de contexto y coste, propio y por hijo.

use super::*;

impl App {
    /// Sustituye los apuntes de la sesión abierta por los que devuelve la base de datos. Una
    /// carga de otra sesión se ignora, para no mezclar totales.
    pub fn load_usage(&mut self, session: SessionId, records: Vec<UsageRecord>) {
        if self.session_id == Some(session) {
            self.input_tokens = records
                .iter()
                .filter(|record| record.session == session)
                .map(|record| record.input_tokens)
                .sum();
            self.output_tokens = records
                .iter()
                .filter(|record| record.session == session)
                .map(|record| record.output_tokens)
                .sum();
            self.usage_records = records;
        }
    }

    /// Añade un apunte vivo con el modelo y los precios vigentes en este instante, atribuido a
    /// `session` (el padre para su turno, el hijo para lo suyo). Sin sesión abierta se usa
    /// `SessionId(0)` como marcador: la recarga de la sesión sustituye esta lista por la de la
    /// base de datos, así que no duplica ni deja marcadores persistidos.
    /// Suma un consumo a los contadores de la sesión y del turno, y lo apunta. No toca el medidor
    /// de contexto: eso lo decide quien llama, según sea `Usage` o `CompactionUsage`.
    pub(super) fn add_usage_counters(&mut self, input_tokens: u32, output_tokens: u32) {
        self.input_tokens = self.input_tokens.saturating_add(u64::from(input_tokens));
        self.output_tokens = self.output_tokens.saturating_add(u64::from(output_tokens));
        self.turn_input_tokens = self
            .turn_input_tokens
            .saturating_add(u64::from(input_tokens));
        self.turn_output_tokens = self
            .turn_output_tokens
            .saturating_add(u64::from(output_tokens));
        let session = self.session_id.unwrap_or(SessionId(0));
        self.record_live_usage(session, input_tokens, output_tokens, None);
    }

    pub(super) fn apply_usage_progress(
        &mut self,
        session: SessionId,
        event: &codeloom_core::event::LiveEvent,
    ) {
        let codeloom_core::event::LiveEvent::UsageProgress {
            request_id,
            model,
            provider_usage,
            provider_final,
            estimated_usage,
            finished,
            components,
        } = event
        else {
            return;
        };
        let key = (session.0, *request_id);
        let previous = self
            .active_usage
            .get(&key)
            .map(ActiveUsage::live_counts)
            .unwrap_or(Usage {
                input_tokens: 0,
                output_tokens: 0,
                reported_cost_usd: None,
            });
        let active = ActiveUsage {
            model: model.clone(),
            provider: provider_usage.clone(),
            provider_final: *provider_final,
            estimate: estimated_usage.clone(),
            components: components.clone(),
        };
        let current = active.live_counts();
        let adjust = |total: &mut u64, before: u32, after: u32| {
            if after >= before {
                *total = total.saturating_add(u64::from(after - before));
            } else {
                *total = total.saturating_sub(u64::from(before - after));
            }
        };
        if session == self.session_id.unwrap_or(SessionId(0)) {
            adjust(
                &mut self.input_tokens,
                previous.input_tokens,
                current.input_tokens,
            );
            adjust(
                &mut self.output_tokens,
                previous.output_tokens,
                current.output_tokens,
            );
            adjust(
                &mut self.turn_input_tokens,
                previous.input_tokens,
                current.input_tokens,
            );
            adjust(
                &mut self.turn_output_tokens,
                previous.output_tokens,
                current.output_tokens,
            );
            if current.input_tokens > 0 {
                self.context_tokens = u64::from(current.input_tokens);
            }
        } else {
            let stats = self.agent_stats.entry(session.0).or_default();
            adjust(
                &mut stats.input_tokens,
                previous.input_tokens,
                current.input_tokens,
            );
            adjust(
                &mut stats.output_tokens,
                previous.output_tokens,
                current.output_tokens,
            );
        }
        if *finished {
            let (usage, source) = active.stored_counts();
            self.record_usage_record(session, model, usage, source, true, active.components);
            self.active_usage.remove(&key);
        } else {
            self.active_usage.insert(key, active);
        }
    }

    pub(super) fn record_usage_record(
        &mut self,
        session: SessionId,
        model: &str,
        usage: Usage,
        source: UsageSource,
        is_final: bool,
        components: Vec<codeloom_core::session::UsageComponent>,
    ) {
        let prices = self.model_prices_for(model);
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        self.usage_records.push(UsageRecord {
            session,
            model: model.to_owned(),
            input_tokens: u64::from(usage.input_tokens),
            output_tokens: u64::from(usage.output_tokens),
            input_price: prices.map(|p| p.0),
            output_price: prices.map(|p| p.1),
            reported_cost_usd: usage.reported_cost_usd,
            source,
            is_final,
            components,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            created_at_ms,
        });
    }

    pub(super) fn record_live_usage(
        &mut self,
        session: SessionId,
        input_tokens: u32,
        output_tokens: u32,
        reported_cost_usd: Option<f64>,
    ) {
        let (input_price, output_price) = self.model_prices().unzip();
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        self.usage_records.push(UsageRecord {
            session,
            model: self.model.clone(),
            input_tokens: u64::from(input_tokens),
            output_tokens: u64::from(output_tokens),
            input_price,
            output_price,
            reported_cost_usd: reported_cost_usd.filter(|cost| cost.is_finite() && *cost >= 0.0),
            source: UsageSource::Provider,
            is_final: true,
            components: Vec::new(),
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            created_at_ms,
        });
    }

    pub(super) fn set_reported_cost(&mut self, session: SessionId, usd: f64) {
        if usd.is_finite() && usd >= 0.0 {
            if let Some(record) = self
                .usage_records
                .iter_mut()
                .rev()
                .find(|record| record.session == session)
            {
                record.reported_cost_usd = Some(usd);
            }
        }
    }

    /// Tokens de entrada y salida sumados de todos los hijos de la sesión abierta.
    pub fn hierarchy_tokens(&self) -> (u64, u64) {
        self.agent_stats
            .values()
            .fold((0, 0), |(input, output), stats| {
                (
                    input.saturating_add(stats.input_tokens),
                    output.saturating_add(stats.output_tokens),
                )
            })
    }

    /// Medidor del pie: uso de la última petición, ventana del modelo y memoria del resumen.
    pub fn context_meter(&self) -> batten::widgets::context_meter::ContextMeter {
        batten::widgets::context_meter::ContextMeter::new(
            self.context_tokens,
            self.context_window(),
        )
        .with_memory(self.context_memory)
        .with_ready(matches!(self.background_summary, BackgroundSummary::Ready))
    }

    /// Etiqueta de la línea de estado mientras corre el resumen anticipado: `resumiendo… N s`.
    /// Los segundos se calculan al pintar a partir del instante de arranque; `None` si no corre.
    pub fn background_summary_label(&self) -> Option<String> {
        match self.background_summary {
            BackgroundSummary::Running { since } => {
                Some(format!("resumiendo… {}s", since.elapsed().as_secs()))
            }
            BackgroundSummary::None | BackgroundSummary::Ready => None,
        }
    }

    /// Ventana de contexto del modelo actual según el catálogo, por el mismo camino que
    /// `model_prices`. Se cachea y solo se recalcula si cambian el catálogo, la URL base o el
    /// modelo: el pie se pinta en cada fotograma y no debe recorrer el catálogo entero cada vez.
    pub(super) fn context_window(&self) -> Option<u64> {
        let base_url = self
            .provider_settings
            .as_ref()
            .map_or("", |settings| settings.base_url.as_str());
        let mut cache = self.context_window_cache.borrow_mut();
        if let Some(entry) = cache.as_ref() {
            if Arc::ptr_eq(&entry.catalog, &self.catalog)
                && entry.base_url == base_url
                && entry.model == self.model
            {
                return entry.window;
            }
        }
        let window = self.catalog.context_window(base_url, &self.model);
        *cache = Some(ContextWindowCache {
            catalog: Arc::clone(&self.catalog),
            base_url: base_url.to_string(),
            model: self.model.clone(),
            window,
        });
        window
    }

    /// Precios `(entrada, salida)` por millón de tokens del modelo actual, buscados en el
    /// catálogo. `None` si falta el catálogo, el proveedor, el modelo o alguno de los dos precios.
    pub fn model_prices(&self) -> Option<(f64, f64)> {
        self.model_prices_for(&self.model)
    }

    pub(super) fn model_prices_for(&self, model_id: &str) -> Option<(f64, f64)> {
        let provider = self.model_catalog_provider()?;
        let model = provider.models.iter().find(|model| model.id == model_id)?;
        Some((model.input_cost?, model.output_cost?))
    }

    /// Coste de la sesión abierta: suma cada apunte del libro (propios e hijos) a su precio
    /// congelado en el momento. Devuelve `(total, parcial)`, donde `parcial` indica que algún
    /// apunte no tenía precio y el total queda por debajo de lo real. `None` sin apuntes.
    pub fn estimated_cost(&self) -> Option<(f64, bool)> {
        if self.usage_records.is_empty() {
            return None;
        }
        let mut total = 0.0;
        let mut partial = false;
        for record in &self.usage_records {
            match record.cost() {
                Some(cost) => total += cost,
                None => partial = true,
            }
        }
        Some((total, partial))
    }

    pub fn cost_is_estimated(&self) -> bool {
        self.usage_records
            .iter()
            .any(|record| record.reported_cost_usd.is_none() && record.cost().is_some())
    }

    pub fn active_usage_label(&self) -> Option<String> {
        let (_, active) = self
            .active_usage
            .iter()
            .find(|((session, _), _)| Some(SessionId(*session)) == self.session_id)?;
        let usage = active.live_counts();
        let origin = if active.provider_final {
            "provider"
        } else if active.provider.is_some() {
            "estimación provisional · provider parcial"
        } else {
            "estimación provisional"
        };
        let partial = active
            .provider
            .as_ref()
            .filter(|_| !active.provider_final)
            .map(|p| {
                format!(
                    " · provider parcial ↑{} ↓{}",
                    p.input_tokens, p.output_tokens
                )
            })
            .unwrap_or_default();
        Some(format!(
            "{} ↑{} ↓{} · {origin}{partial}",
            active.model, usage.input_tokens, usage.output_tokens
        ))
    }

    pub fn family_tokens(&self) -> (u64, u64) {
        let input = self
            .usage_records
            .iter()
            .map(|record| record.input_tokens)
            .sum::<u64>()
            + self
                .active_usage
                .values()
                .map(|active| u64::from(active.live_counts().input_tokens))
                .sum::<u64>();
        let output = self
            .usage_records
            .iter()
            .map(|record| record.output_tokens)
            .sum::<u64>()
            + self
                .active_usage
                .values()
                .map(|active| u64::from(active.live_counts().output_tokens))
                .sum::<u64>();
        (input, output)
    }

    /// Coste de un hijo: suma los apuntes que llevan su sesión, con el mismo criterio que
    /// `estimated_cost` (devuelve `(total, parcial)`). `None` si no tiene apuntes o si ninguno
    /// lleva precio, para no mostrar un `≈$0` que no informa de nada.
    pub fn child_cost(&self, child: SessionId) -> Option<(f64, bool)> {
        let mut total = 0.0;
        let mut priced = false;
        let mut partial = false;
        for record in self.usage_records.iter().filter(|r| r.session == child) {
            match record.cost() {
                Some(cost) => {
                    total += cost;
                    priced = true;
                }
                None => partial = true,
            }
        }
        priced.then_some((total, partial))
    }

    pub fn child_cost_is_estimated(&self, child: SessionId) -> bool {
        self.usage_records.iter().any(|record| {
            record.session == child && record.reported_cost_usd.is_none() && record.cost().is_some()
        })
    }
}

/// Línea del aviso de presupuesto de gasto. Rótulo de CodeLoom: nunca de A2A. Los tokens pasan
/// por `format_tokens`; los dólares, con dos decimales. Si `partial`, se advierte de que el coste
/// no suma los apuntes sin precio conocido.
pub(super) fn budget_warning_line(
    metric: BudgetMetric,
    level_percent: u8,
    used: f64,
    limit: f64,
    partial: bool,
) -> String {
    let (unidad, usado, limite) = match metric {
        BudgetMetric::Tokens => (
            "tokens",
            format_tokens(used as u64),
            format_tokens(limit as u64),
        ),
        BudgetMetric::CostUsd => ("USD", format!("{used:.2}"), format!("{limit:.2}")),
    };
    let mut linea = format!(
        "aviso de presupuesto: {level_percent} % de {limite} {unidad} de la sesión y sus subagentes ({usado} usados)"
    );
    if partial {
        linea.push_str(" (según los precios conocidos)");
    }
    linea
}
