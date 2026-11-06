//! Órdenes sobre subagentes, lanzador y avisos de los hijos.

use super::*;

impl AgentLoop {
    pub async fn cancel_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
    ) -> crate::error::Result<crate::session::SubagentDescriptor> {
        self.subagents
            .as_ref()
            .ok_or_else(|| crate::error::CoreError::Tool("subagents are unavailable".into()))?
            .cancel(parent, child)
            .await
    }

    /// Da por fallido a un hijo perdido y detiene su trabajo (lo pide el supervisor).
    pub async fn fail_lost_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
        reason: String,
    ) -> crate::error::Result<()> {
        self.subagents
            .as_ref()
            .ok_or_else(|| crate::error::CoreError::Tool("subagents are unavailable".into()))?
            .fail_lost(parent, child, reason)
            .await
    }

    /// Responde la pregunta pendiente (`input-required`) de un hijo con el texto de la persona
    /// usuaria, sin turno del modelo principal.
    pub async fn answer_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
        text: String,
    ) -> crate::error::Result<()> {
        self.subagents
            .as_ref()
            .ok_or_else(|| crate::error::CoreError::Tool("subagents are unavailable".into()))?
            .answer(parent, child, vec![text])
            .await
    }

    /// Lanza un subagente por orden directa de la persona usuaria, sin turno del modelo principal.
    pub async fn run_subagent_request(
        &self,
        parent: SessionId,
        request: crate::subagent::SubagentRequest,
    ) -> crate::error::Result<(SessionId, crate::job::JobId)> {
        self.subagents
            .as_ref()
            .ok_or_else(|| crate::error::CoreError::Tool("subagents are unavailable".into()))?
            .start_requested(parent, request, CancellationToken::new())
            .await
    }

    /// Prompt de sistema vigente: contexto del proyecto, lista de agentes y prompt propio, cada
    /// parte con su tope. Se calcula en cada turno, de modo que la recarga en caliente del registro
    /// también le llega al modelo.
    /// El lanzador, salvo con `delegation = "off"`: entonces el modelo no ve nada de subagentes.
    pub(super) fn subagents_visibles(&self) -> Option<&Arc<dyn crate::subagent::SubagentLauncher>> {
        match self.delegation {
            Some(crate::config::DelegationMode::Off) => None,
            _ => self.subagents.as_ref(),
        }
    }

    /// Lanzador de subagentes con el estado actual de este agente, catálogo y registro incluidos.
    pub(super) fn subagent_launcher(&self) -> Arc<dyn crate::subagent::SubagentLauncher> {
        let mut tools = self.tools.as_ref().clone();
        for tool in &self.subagent_tools {
            tools.register(tool.clone());
        }
        let lanzador = crate::subagent::InProcessSubagentLauncher::new(
            self.provider.clone(),
            self.store.clone(),
            Arc::new(tools),
            self.permission.clone(),
            self.config.clone(),
            self.cwd.clone(),
            self.jobs.clone(),
            self.bus.clone(),
        )
        .with_catalog(self.catalog.clone())
        .with_budget(self.budget.clone())
        .with_limits(self.limits.clone())
        .with_max_active(self.max_active_subagents)
        .with_delegation_confirm(self.delegation_confirm)
        .with_delegation_mode(self.delegation)
        .with_levels_left(crate::subagent::root_levels(self.depth_ceiling));
        let lanzador = match &self.profile_provider_resolver {
            Some(resolver) => lanzador.with_profile_provider_resolver(resolver.clone()),
            None => lanzador,
        };
        let lanzador = match &self.registry {
            Some(registro) => lanzador.with_registry(registro.clone()),
            None => lanzador,
        };
        let lanzador = match &self.router {
            Some(router) => lanzador.with_router(router.clone(), self.delegation_confirm),
            None => lanzador,
        };
        let lanzador = match &self.spill {
            Some(spill) => lanzador.with_spill(spill.clone()),
            None => lanzador,
        };
        let lanzador = match &self.remote_clients {
            Some(fabrica) => lanzador.with_remote_clients(fabrica.clone()),
            None => lanzador,
        };
        Arc::new(lanzador)
    }

    /// Entrega los avisos pendientes de jobs y subagentes. Devuelve cuántos se
    /// entregaron en esta llamada; cada aviso sale una sola vez.
    pub(super) async fn flush_subagent_notices(&self, session: SessionId) -> Result<usize> {
        let mut delivered = 0usize;
        // Los lotes de vigilancia van antes que el aviso de fin: el transcript sigue así el orden
        // real, primero lo que el job escribió y después su final.
        let ahora = std::time::Instant::now();
        for (job, lote) in self.jobs.take_watch_batches(session, ahora) {
            let message = Message::user(aviso_de_vigilancia(&job, &lote));
            self.store.append_message(session, &message).await?;
            self.bus
                .publish(LiveEvent::SessionMessageAdded { session, message });
            delivered += 1;
        }
        for job in self.jobs.pending_settled_notices(session) {
            let status = match job.status {
                crate::job::JobStatus::Completed => "completed",
                crate::job::JobStatus::Killed => "killed",
                crate::job::JobStatus::Failed => "failed",
                _ => continue,
            };
            let detail = job
                .detail
                .or_else(|| job.result.map(|result| result.content));
            let message = Message::user(format!(
                "[Job {} {status}] {}{}",
                job.id.0,
                job.label,
                detail
                    .map(|text| format!("\n{}", limit_bytes(&text, MAX_NOTICE_BYTES)))
                    .unwrap_or_default()
            ));
            self.store.append_message(session, &message).await?;
            self.jobs.mark_notice_delivered(session, &job.id)?;
            self.bus
                .publish(LiveEvent::SessionMessageAdded { session, message });
            delivered += 1;
        }
        if let Some(subagents) = &self.subagents {
            for message in subagents.deliver_notices(session).await? {
                self.bus
                    .publish(LiveEvent::SessionMessageAdded { session, message });
                delivered += 1;
            }
            // Las preguntas de los hijos se guardan aquí, a diferencia de los avisos
            // de finalización, que el propio lanzador persiste al entregarlos.
            for message in subagents.pending_input_notices(session).await? {
                self.store.append_message(session, &message).await?;
                self.bus
                    .publish(LiveEvent::SessionMessageAdded { session, message });
                delivered += 1;
            }
        }
        Ok(delivered)
    }
}

/// Texto del aviso de un lote de vigilancia: `[Job <id> watch] <línea1>\n<línea2>…`, con el recuento
/// de líneas suprimidas y la marca de límite alcanzado cuando corresponden, y con tope de bytes.
/// Lo calcula el registro de jobs; aquí solo se da forma y se acota.
fn aviso_de_vigilancia(
    job: &crate::job::JobSnapshot,
    lote: &crate::job_watch::WatchBatch,
) -> String {
    let mut texto = format!("[Job {} watch]", job.id.0);
    for (indice, linea) in lote.lines.iter().enumerate() {
        texto.push_str(if indice == 0 { " " } else { "\n" });
        texto.push_str(linea);
    }
    if lote.suppressed > 0 {
        texto.push_str(&format!("\n+{} more suppressed", lote.suppressed));
    }
    if job.watch.as_ref().is_some_and(|info| info.limit_reached) {
        texto.push_str("\nwatch limit reached");
    }
    limit_bytes(&texto, MAX_NOTICE_BYTES).into_owned()
}

/// El supervisor da por fallidos a los hijos perdidos a través de una referencia débil: así su
/// tarea no mantiene vivo al agente (que es dueño del bus que ella escucha).
#[async_trait::async_trait]
impl crate::subagent::supervisor_driver::LostHandler for std::sync::Weak<AgentLoop> {
    async fn fail_lost(&self, parent: SessionId, child: SessionId, reason: String) {
        let Some(agent) = self.upgrade() else { return };
        if let Err(error) = agent.fail_lost_subagent(parent, child, reason).await {
            tracing::warn!("could not fail lost subagent {}: {error}", child.0);
        }
    }
}
