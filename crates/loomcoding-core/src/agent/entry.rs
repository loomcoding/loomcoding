//! Puntos de entrada de un turno y prompt del sistema.

use super::*;

impl AgentLoop {
    pub(super) fn system_prompt(&self) -> String {
        let mut system = crate::config::context::load_context_files(&self.cwd).join("\n\n");
        // Con la herramienta `subagent` y un registro, el modelo recibe la lista vigente de agentes:
        // se lee en cada turno, de modo que la recarga en caliente también le llega.
        if let (Some(_), Some(registro)) = (self.subagents_visibles(), &self.registry) {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            let seccion = registro.snapshot().prompt_section(self.delegation_confirm);
            system.push_str(&limit_bytes(&seccion, MAX_AGENT_SECTION_BYTES));
        }
        // Solo quien puede delegar recibe la guía de paralelismo y delegación.
        if self.subagents_visibles().is_some() {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(PARALLEL_GUIDANCE);
        }
        // Las reglas de `on_request` y `proactive` son persuasión del prompt, no una garantía.
        if let Some(regla) = self
            .delegation
            .and_then(|modo| modo.rule())
            .filter(|_| self.subagents.is_some())
        {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(regla);
        }
        // Las contribuciones de prompt (véase `prompt_contributor`) van tras la guía y antes del prompt propio.
        if let Some(secciones) = self.contributed_sections() {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(&secciones);
        }
        // El prompt propio del agente va detrás del contexto del proyecto, sin sustituirlo.
        if let Some(extra) = self
            .system_extra
            .as_deref()
            .filter(|t| !t.trim().is_empty())
        {
            if !system.is_empty() {
                system.push_str("\n\n");
            }
            system.push_str(&limit_bytes(extra, MAX_SYSTEM_EXTRA_BYTES));
        }
        system
    }

    /// Turno iniciado por un mensaje del usuario.
    pub async fn run(
        &self,
        session: SessionId,
        user_prompt: String,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome> {
        self.run_with_origin(session, user_prompt, cancel, MessageOrigin::Human)
            .await
    }

    /// Ejecuta un turno iniciado por una persona con texto y contenido adjunto.
    pub async fn run_message(
        &self,
        session: SessionId,
        mut message: Message,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome> {
        if message.role != crate::message::Role::User {
            return Err(CoreError::Provider(
                "prompt message must have user role".into(),
            ));
        }
        message.origin = MessageOrigin::Human;
        self.run_with_message(session, message, cancel).await
    }

    /// Ejecuta un turno cuyo prompt fue asignado por Loomcoding (por ejemplo, a un subagente).
    pub(crate) async fn run_internal(
        &self,
        session: SessionId,
        prompt: String,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome> {
        self.run_with_origin(session, prompt, cancel, MessageOrigin::Internal)
            .await
    }

    pub(super) async fn run_with_origin(
        &self,
        session: SessionId,
        user_prompt: String,
        cancel: CancellationToken,
        origin: MessageOrigin,
    ) -> Result<TurnOutcome> {
        let mut message = Message::user(user_prompt);
        message.origin = origin;
        self.run_with_message(session, message, cancel).await
    }

    pub(super) async fn run_with_message(
        &self,
        session: SessionId,
        user_message: Message,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome> {
        self.bus.publish(LiveEvent::TurnStarted);
        let result = match self.flush_subagent_notices(session).await {
            Ok(_) => self.run_turn(session, Some(user_message), cancel).await,
            Err(error) => Err(error),
        };
        self.publish_turn_result(&result);
        result
    }

    /// Despertar del padre ocioso: entrega los avisos pendientes y, solo si había
    /// alguno, ejecuta un turno sin mensaje de usuario. Sin avisos devuelve `None`
    /// y publica `WakeIdle`, sin turno ni gasto alguno.
    pub async fn run_woken(
        &self,
        session: SessionId,
        cancel: CancellationToken,
    ) -> Result<Option<TurnOutcome>> {
        let delivered = match self.flush_subagent_notices(session).await {
            Ok(delivered) => delivered,
            Err(error) => {
                self.bus.publish(LiveEvent::Error {
                    message: error.to_string(),
                });
                return Err(error);
            }
        };
        if delivered == 0 {
            self.bus.publish(LiveEvent::WakeIdle);
            return Ok(None);
        }
        self.bus.publish(LiveEvent::TurnStarted);
        let result = self.run_turn(session, None, cancel).await;
        self.publish_turn_result(&result);
        result.map(Some)
    }

    /// Publica el desenlace de un turno ya ejecutado (fin o error).
    pub(super) fn publish_turn_result(&self, result: &Result<TurnOutcome>) {
        match result {
            Ok(_) => self.bus.publish(LiveEvent::TurnEnded),
            Err(error) => self.bus.publish(LiveEvent::Error {
                message: error.to_string(),
            }),
        }
    }
}
