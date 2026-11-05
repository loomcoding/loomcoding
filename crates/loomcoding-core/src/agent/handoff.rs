//! Informe de traspaso: una sola petición sin herramientas para que el modelo diga qué hizo y qué falta.

use super::*;

/// Tope del informe de traspaso (decálogo de contexto).
pub const HANDOFF_MAX_BYTES: usize = 4096;

impl AgentLoop {
    /// Pide al modelo, sin herramientas, un informe breve de lo hecho y lo que falta.
    ///
    /// Cabe señalar que no es un turno: no cuenta pasos ni ejecuta nada. La petición y la respuesta se
    /// añaden al historial para que queden en el diario. Si el modelo solo deja texto en el razonamiento,
    /// se usa ese. Devuelve cadena vacía, no error, si no hay respuesta.
    pub(crate) async fn handoff_report(
        &self,
        session: SessionId,
        reason: &str,
        cancel: &CancellationToken,
    ) -> Result<String> {
        // Con `hard_stop` y la familia agotada no se pide nada más al modelo, tampoco el traspaso.
        if self.budget.hard_stop {
            let family = self
                .store
                .session_usage(self.budget_family.unwrap_or(session))
                .await?;
            if crate::budget::budget_exhausted(&self.budget, &family).is_some() {
                return Ok(String::new());
            }
        }
        let provider = self.provider.read().unwrap().clone();
        let config = self.config.read().unwrap().clone();
        let ask = Message::user(format!(
            "[Hand-off] Your work is being cut ({reason}). Do not call tools. In at most 15 short lines, \
             report what you did, what is left, and your conclusion so far. If you were asked for a verdict \
             or a JSON object, give it now as the last line."
        ));
        self.store.append_message(session, &ask).await?;
        let history = self.store.recent_messages(session, HISTORY_WINDOW).await?;
        let req = ProviderRequest {
            model: config.model.clone(),
            system: self.system_prompt(),
            messages: clear::clear_old_tool_results(
                &compact::effective_history(&history),
                clear::KEEP_TOOL_RESULTS,
            ),
            tools: Vec::new(),
            temperature: self.limits.temperature,
            max_tokens: 4096,
            effort: config.effort.clone(),
            programmatic_tool_calling: false,
        };
        let mut stream = provider.stream(req, cancel.clone()).await?;
        let mut text = String::new();
        let mut reasoning = String::new();
        while let Some(chunk) = tokio::select! {
            _ = cancel.cancelled() => None,
            chunk = stream.next() => chunk,
        } {
            match chunk {
                StreamChunk::TextDelta(t) => text.push_str(&t),
                StreamChunk::ReasoningDelta(r) => {
                    turn::push_tail_capped(&mut reasoning, &r, HANDOFF_MAX_BYTES * 4)
                }
                StreamChunk::Error(error) => return Err(CoreError::Provider(error)),
                _ => {}
            }
        }
        if text.trim().is_empty() {
            text = reasoning;
        }
        let text = limit_bytes(text.trim(), HANDOFF_MAX_BYTES).into_owned();
        if !text.is_empty() {
            let reply = Message::assistant(vec![ContentPart::Text { text: text.clone() }]);
            self.store.append_message(session, &reply).await?;
        }
        Ok(text)
    }
}
