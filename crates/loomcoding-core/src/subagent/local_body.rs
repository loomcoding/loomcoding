//! El trabajo de un hijo local: espera la compuerta, construye su agente, lo ejecuta y asienta el
//! resultado. Es el equivalente local de `remote.rs`.
//!
//! Cabe señalar que todo lo que el trabajo necesita llega ya resuelto en [`EjecucionLocal`]: este
//! módulo no decide nada sobre perfiles, permisos ni capacidad (eso es `local_run` y `admission`),
//! solo ejecuta.

use super::*;

/// Todo lo que el trabajo de un hijo local necesita para ejecutarse.
pub(super) struct EjecucionLocal {
    /// Puesto de capacidad: se suelta al terminar el trabajo, sea cual sea el desenlace.
    pub(super) permit: OwnedSemaphorePermit,
    pub(super) broker: Arc<InputBroker>,
    pub(super) parent: SessionId,
    pub(super) child: SessionId,
    /// La compuerta retiene al hijo hasta que su tarea `submitted` está guardada.
    pub(super) puerta: tokio::sync::oneshot::Receiver<()>,
    pub(super) tracker: Arc<TaskTracker>,
    pub(super) prompt: String,
    pub(super) parent_cancel: CancellationToken,
    pub(super) provider: Arc<dyn Provider>,
    pub(super) store: Arc<dyn SessionStore>,
    pub(super) tools: Arc<ToolRegistry>,
    /// Bus propio del hijo: se cierra cuando se sueltan todos sus emisores.
    pub(super) bus: EventBus,
    pub(super) permission: Arc<PermissionService>,
    pub(super) config: LoomcodingConfig,
    pub(super) cwd: PathBuf,
    pub(super) jobs: Arc<dyn JobRegistry>,
    pub(super) catalog: Arc<RwLock<Catalog>>,
    pub(super) system: Option<String>,
    pub(super) skill_usage: Vec<crate::session::UsageComponent>,
    pub(super) budget: BudgetConfig,
    pub(super) family: SessionId,
    pub(super) spill: Option<ToolSpill>,
    pub(super) registry: Option<Arc<AgentRegistry>>,
    pub(super) confirm: ConfirmDelegation,
    pub(super) delegation: Option<crate::config::DelegationMode>,
    /// Lanzador anidado: solo lo reciben los hijos que pueden delegar.
    pub(super) nested: Option<Arc<dyn SubagentLauncher>>,
}

pub(super) fn cuerpo_local(ejecucion: EjecucionLocal) -> JobBody {
    Box::new(move |handle, job_cancel| {
        Box::pin(async move {
            let EjecucionLocal {
                permit,
                broker: child_broker,
                parent,
                child,
                puerta,
                tracker: cuerpo_tracker,
                prompt,
                parent_cancel,
                provider: child_provider,
                store: child_store,
                tools: child_tools,
                bus: child_bus,
                permission: child_permission,
                config: child_config,
                cwd: child_cwd,
                jobs: child_jobs,
                catalog: child_catalog,
                system: child_system,
                skill_usage: skill_usage_components,
                budget: child_budget,
                family: child_family,
                spill: child_spill,
                registry: child_registry,
                confirm: child_confirm,
                delegation: child_delegation,
                nested: child_nested,
            } = ejecucion;
            let child_max_steps = child_config.max_steps;
            let _permit = permit;
            let _discard = DiscardOnExit {
                broker: child_broker,
                child,
            };
            // Si el lanzador no llega a abrir la compuerta, el hijo no arranca.
            if puerta.await.is_err() || job_cancel.is_cancelled() {
                return Ok(None);
            }
            if let Err(error) = cuerpo_tracker
                .transition(parent, child, TaskStatus::new(TaskState::Working))
                .await
            {
                tracing::warn!("persist subagent status: {error}");
            }
            let mut child_agent = AgentLoop::new(
                child_provider,
                child_store.clone(),
                child_tools,
                child_bus,
                child_permission,
                child_config,
                child_cwd,
            )
            .with_jobs(child_jobs)
            .with_catalog(child_catalog)
            .with_system_extra(child_system)
            .with_system_usage_components(skill_usage_components)
            .with_budget(child_budget)
            .with_budget_family(child_family);
            if let Some(spill) = child_spill {
                child_agent = child_agent.with_spill(spill);
            }
            if let Some(registro) = child_registry {
                child_agent = child_agent
                    .with_agent_registry(registro)
                    .with_delegation_confirm(child_confirm);
            }
            // Al final: los demás `with_*` reconstruyen el lanzador por defecto del agente.
            if let Some(lanzador) = child_nested {
                child_agent = child_agent
                    .with_delegation_mode(child_delegation)
                    .with_subagent_launcher(lanzador);
            }
            let child_cancel = CancellationToken::new();
            let outcome = tokio::select! {
                result = child_agent.run_internal(child, prompt, child_cancel.clone()) => result?,
                _ = job_cancel.cancelled() => {
                    child_cancel.cancel();
                    return Ok(None);
                }
                _ = parent_cancel.cancelled() => {
                    child_cancel.cancel();
                    // Cancelar al padre cancela al hijo: el trabajo se mata para que la
                    // tarea acabe en `canceled` y no en `completed` sin resultado.
                    job_cancel.cancel();
                    return Ok(None);
                }
            };
            let steps = outcome.steps;
            // Los hijos usan los topes por defecto: el de lectura es `4 × max_steps`.
            let read_cap = crate::config::LimitsConfig::default().read_step_cap(child_max_steps);
            let cut = if steps >= child_max_steps {
                format!("step limit ({steps}/{child_max_steps})")
            } else if outcome.read_steps >= read_cap {
                format!("read limit ({}/{read_cap})", outcome.read_steps)
            } else {
                String::new()
            };
            let mut text = outcome.final_text;
            if text.trim().is_empty() && !job_cancel.is_cancelled() && !parent_cancel.is_cancelled()
            {
                // Traspaso: una petición sin herramientas para no perder lo que el hijo llevaba. Si el
                // modelo contesta, el trabajo termina como parcial y el padre lo ve rotulado.
                let why = if cut.is_empty() {
                    format!("turn cut early after {steps} steps")
                } else {
                    cut.clone()
                };
                match child_agent.handoff_report(child, &why, &child_cancel).await {
                    Ok(report) if !report.trim().is_empty() => {
                        text = format!(
                            "[PARTIAL: {why}]
{report}"
                        );
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!("subagent hand-off failed: {error}"),
                }
            }
            // El agente ya no emite: soltarlo cierra el bus del hijo y permite
            // que la tarea de reenvío termine.
            drop(child_agent);
            if text.trim().is_empty() {
                // El motivo importa: antes todo se reportaba como "sin respuesta
                // final tras N pasos", que sonaba a presupuesto agotado incluso
                // cuando el corte había sido por repetición de tool calls.
                let reason = if !cut.is_empty() {
                    format!("subagent reached its {cut} without a final response")
                } else {
                    format!(
                        "subagent produced no final response after {steps} steps \
                         (the turn was cut early: repeated tool calls or cancellation)"
                    )
                };
                return Err(CoreError::Tool(reason));
            }
            child_store
                .complete_subagent(parent, child, &[Artifact::text_artifact(text.as_str())])
                .await?;
            if let Err(error) = cuerpo_tracker.publish(parent, child).await {
                tracing::warn!("publish subagent status: {error}");
            }
            handle.append(&text);
            Ok(Some(JobResult { content: text }))
        })
    })
}
