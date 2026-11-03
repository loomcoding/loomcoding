//! Apuntes de consumo, estimaciones y avisos de progreso.

use super::*;

impl AgentLoop {
    /// Registrador de consumo con el estado actual de este agente. Es clonable y no toma prestado
    /// el agente, así que una tarea en segundo plano puede llevarlo consigo.
    pub(super) fn recorder(&self) -> UsageRecorder {
        UsageRecorder {
            store: self.store.clone(),
            catalog: self.catalog.clone(),
            budget: self.budget.clone(),
            family: self.budget_family,
            bus: self.bus.clone(),
        }
    }

    /// Guarda el apunte de consumo de una respuesta. Véase `UsageRecorder::record`.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_usage(
        &self,
        session: SessionId,
        model: &str,
        base_url: &str,
        usage: &crate::message::Usage,
        cache: crate::provider::CacheUsage,
        source: crate::message::UsageSource,
        is_final: bool,
        components: &[crate::session::UsageComponent],
    ) {
        self.recorder()
            .record(
                session, model, base_url, usage, cache, source, is_final, components,
            )
            .await;
    }
}

pub(super) fn estimate_provider_request_tokens(request: &ProviderRequest) -> u32 {
    let desglose = compact::breakdown_request(&request.system, &request.messages, &request.tools);
    u32::try_from(desglose.total()).unwrap_or(u32::MAX)
}

pub(super) fn estimate_stream_output(
    text: &str,
    calls: &[(String, String, serde_json::Value)],
) -> u32 {
    let calls = calls
        .iter()
        .map(|(_, name, input)| {
            compact::estimate_text_tokens(name) + compact::estimate_text_tokens(&input.to_string())
        })
        .sum::<usize>();
    u32::try_from(compact::estimate_text_tokens(text).saturating_add(calls)).unwrap_or(u32::MAX)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish_usage_progress(
    bus: &EventBus,
    request_id: u64,
    model: &str,
    provider_usage: Option<Usage>,
    provider_final: bool,
    estimated_usage: &Usage,
    finished: bool,
    components: &[crate::session::UsageComponent],
) {
    bus.publish(LiveEvent::UsageProgress {
        request_id,
        model: model.to_owned(),
        provider_usage,
        provider_final,
        estimated_usage: estimated_usage.clone(),
        finished,
        components: components.to_vec(),
    });
}

/// Registrador del consumo de una respuesta. Lleva copias compartidas del almacén, el catálogo y el bus, de modo que es `'static` y puede moverse a una tarea
/// `tokio::spawn`. Su comportamiento es el del antiguo `AgentLoop::record_usage`.
#[derive(Clone)]
pub(super) struct UsageRecorder {
    pub(super) store: Arc<dyn SessionStore>,
    pub(super) catalog: Arc<RwLock<Catalog>>,
    pub(super) budget: BudgetConfig,
    /// Sesión raíz cuya familia (ella y sus hijos) suma al presupuesto; `None` si es la propia.
    pub(super) family: Option<SessionId>,
    pub(super) bus: EventBus,
}

impl UsageRecorder {
    /// Guarda el consumo con el modelo y la URL capturados al crear la petición. Un fallo de escritura no corta el turno.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record(
        &self,
        session: SessionId,
        model: &str,
        base_url: &str,
        usage: &crate::message::Usage,
        cache: crate::provider::CacheUsage,
        source: crate::message::UsageSource,
        is_final: bool,
        components: &[crate::session::UsageComponent],
    ) {
        let prices = self
            .catalog
            .read()
            .ok()
            .and_then(|catalog| catalog.prices(base_url, model));
        let record = UsageRecord {
            session,
            model: model.to_owned(),
            input_tokens: u64::from(usage.input_tokens),
            output_tokens: u64::from(usage.output_tokens),
            input_price: prices.map(|(input, _)| input),
            output_price: prices.map(|(_, output)| output),
            reported_cost_usd: usage
                .reported_cost_usd
                .filter(|cost| cost.is_finite() && *cost >= 0.0),
            source,
            is_final,
            components: components.to_vec(),
            cache_read_tokens: cache.read_tokens,
            cache_write_tokens: cache.write_tokens,
            created_at_ms: crate::message::now_ms(),
        };
        // Con presupuesto, el acumulado previo se lee antes de guardar el apunte para comparar
        // antes y después. Sin presupuesto no se toca el almacén más allá del apunte.
        let anteriores = if self.budget.is_empty() {
            None
        } else {
            match self
                .store
                .session_usage(self.family.unwrap_or(session))
                .await
            {
                Ok(registros) => Some(registros),
                Err(error) => {
                    tracing::warn!(error = %error, "no se pudo leer el consumo previo del presupuesto");
                    None
                }
            }
        };
        if let Err(error) = self.store.record_usage(session, &record).await {
            tracing::warn!(error = %error, "no se pudo guardar el apunte de consumo");
            return;
        }
        if let Some(anteriores) = anteriores {
            for aviso in budget_crossings(&self.budget, &anteriores, &record) {
                self.bus.publish(LiveEvent::BudgetWarning {
                    metric: aviso.metric,
                    level_percent: aviso.level_percent,
                    used: aviso.used,
                    limit: aviso.limit,
                    partial: aviso.partial,
                });
            }
        }
    }
}
