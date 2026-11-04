//! Implementación de `TaskHost`: envío, suscripción, consulta y cancelación.

use super::*;

#[async_trait]
impl TaskHost for LauncherHost {
    async fn start(&self, prompt: String) -> Result<TaskView, HostError> {
        self.start_with_operation(prompt, A2aOperation::SendMessage)
            .await
    }

    async fn subscribe(&self, id: &str) -> Result<EventStream, HostError> {
        let hijo = id.parse::<i64>().map_err(|_| HostError::NotFound)?;
        // Primero se comprueba que la tarea es de este servidor: una ajena es NotFound aunque no
        // queden huecos, y no se revela nada por el orden de los errores.
        let descriptor = self.descriptor(id).await?;
        let permiso = self
            .flujos
            .clone()
            .try_acquire_owned()
            .map_err(|_| HostError::Busy)?;
        let flujo = self.flujo(hijo, permiso, false).await?;
        self.observe(
            A2aDirection::Inbound,
            A2aOperation::SubscribeToTask,
            Some(id.to_string()),
            Some(descriptor.context_id()),
            Some(descriptor.status.state.proto_name().to_string()),
            None,
        );
        Ok(flujo)
    }

    async fn start_streaming(&self, prompt: String) -> Result<EventStream, HostError> {
        // El hueco se toma antes de crear la tarea: sin él no se crea nada que quede sin flujo.
        let permiso = self
            .flujos
            .clone()
            .try_acquire_owned()
            .map_err(|_| HostError::Busy)?;
        let vista = self
            .start_with_operation(prompt, A2aOperation::SendStreamingMessage)
            .await?;
        let hijo = vista
            .json
            .get("id")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| id.parse::<i64>().ok())
            .ok_or_else(|| HostError::Other("tarea sin id".into()))?;
        self.flujo(hijo, permiso, true).await
    }

    async fn get(&self, id: &str) -> Result<TaskView, HostError> {
        let descriptor = self.descriptor(id).await?;
        self.observe(
            A2aDirection::Inbound,
            A2aOperation::GetTask,
            Some(id.to_string()),
            Some(descriptor.context_id()),
            Some(descriptor.status.state.proto_name().to_string()),
            None,
        );
        Ok(self.vista(&descriptor))
    }

    async fn cancel(&self, id: &str) -> Result<TaskView, HostError> {
        let hijo = id.parse::<i64>().map_err(|_| HostError::NotFound)?;
        match self.launcher.cancel(self.parent, SessionId(hijo)).await {
            Ok(descriptor) => {
                self.observe(
                    A2aDirection::Inbound,
                    A2aOperation::CancelTask,
                    Some(id.to_string()),
                    Some(descriptor.context_id()),
                    Some(descriptor.status.state.proto_name().to_string()),
                    None,
                );
                Ok(self.vista(&descriptor))
            }
            Err(CoreError::TaskNotFound(_)) => Err(HostError::NotFound),
            Err(CoreError::TaskNotCancelable { .. }) => Err(HostError::NotCancelable),
            Err(error) => Err(otro(error)),
        }
    }

    async fn wait_terminal(&self, id: &str, max: Duration) -> Result<TaskView, HostError> {
        let limite = Instant::now() + max;
        loop {
            let vista = self.get(id).await?;
            if vista.terminal || Instant::now() >= limite {
                return Ok(vista);
            }
            tokio::time::sleep(SONDEO).await;
        }
    }
}
