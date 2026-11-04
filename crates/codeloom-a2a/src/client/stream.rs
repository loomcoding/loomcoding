//! Seguimiento de una tarea remota por SSE: lectura del flujo y aplicación de sus eventos.

use super::*;

/// Tiempo máximo de una conexión de flujo. Quien sigue la tarea la suelta antes (`max_follow`); esto
/// solo evita que el tope de petición del cliente (pensado para llamadas cortas) corte un flujo legítimo.
pub(super) const MAX_CONEXION_FLUJO: Duration = Duration::from_secs(6 * 3600);

/// Artefactos que se acumulan como máximo al seguir un flujo.
pub(super) const MAX_ARTEFACTOS_FLUJO: usize = 64;

/// Estado de la lectura de un flujo de tarea.
pub(super) struct LecturaFlujo {
    pub(super) cuerpo: BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    pub(super) decodificador: SseDecoder,
    pub(super) pendientes: VecDeque<String>,
    pub(super) actual: Option<RemoteTask>,
    pub(super) tarea: String,
    pub(super) rpc_id: u64,
    pub(super) max_bytes: usize,
    pub(super) terminado: bool,
    pub(super) agent: String,
    pub(super) observer: Option<EventBus>,
}

/// Aplica un evento (`data:`) del flujo a la tarea que se sigue. Todo lo que no cuadra corta el flujo:
/// es dato de un tercero y no se interpreta a medias.
pub(super) fn observar_flujo(
    lectura: &LecturaFlujo,
    operation: A2aOperation,
    task_id: String,
    context_id: Option<String>,
    state: Option<String>,
    artifact: Option<String>,
) {
    if let Some(bus) = &lectura.observer {
        bus.publish(LiveEvent::A2aObserved(A2aObservation {
            direction: A2aDirection::Outbound,
            agent: lectura.agent.clone(),
            operation,
            rpc_id: Some(lectura.rpc_id.to_string()),
            task_id: Some(task_id),
            context_id,
            state,
            artifact,
            succeeded: true,
            elapsed_ms: None,
        }));
    }
}

pub(super) fn aplicar_evento(
    lectura: &mut LecturaFlujo,
    datos: &str,
) -> Result<RemoteTask, RemoteTaskError> {
    let rpc: RespuestaRpc = serde_json::from_str(datos).map_err(invalida)?;
    if rpc.id != json!(lectura.rpc_id) {
        return Err(RemoteTaskError::Other(
            "stream event id does not match the request".into(),
        ));
    }
    let resultado = match (rpc.result, rpc.error) {
        (Some(resultado), None) => resultado,
        (None, Some(error)) => return Err(traducir_error_rpc(error)),
        _ => {
            return Err(RemoteTaskError::Other(
                "stream event must carry exactly one of result and error".into(),
            ))
        }
    };
    let evento: WireStreamResponse = serde_json::from_value(resultado).map_err(invalida)?;
    evento.validate().map_err(invalida)?;
    let propia = |id: &str| -> Result<(), RemoteTaskError> {
        if id == lectura.tarea {
            Ok(())
        } else {
            Err(RemoteTaskError::Other(
                "the stream carries events of a different task".into(),
            ))
        }
    };
    if let Some(tarea) = evento.task {
        propia(&tarea.id)?;
        lectura.actual = Some(tarea_desde_wire(tarea)?);
    } else if let Some(cambio) = evento.status_update {
        propia(&cambio.task_id)?;
        let (task_id, context_id, state) = (
            cambio.task_id.clone(),
            Some(cambio.context_id.clone()),
            cambio.status.state.clone(),
        );
        let actual = lectura
            .actual
            .as_mut()
            .ok_or_else(|| RemoteTaskError::Other("the stream must start with the task".into()))?;
        let temporal = WireTask {
            id: cambio.task_id,
            context_id: None,
            status: cambio.status,
            artifacts: Vec::new(),
        };
        actual.status = tarea_desde_wire(temporal)?.status;
        observar_flujo(
            lectura,
            A2aOperation::TaskStatusUpdateEvent,
            task_id,
            context_id,
            Some(state),
            None,
        );
    } else if let Some(cambio) = evento.artifact_update {
        propia(&cambio.task_id)?;
        let task_id = cambio.task_id.clone();
        let context_id = Some(cambio.context_id.clone());
        let nombre = cambio
            .artifact
            .name
            .clone()
            .or_else(|| Some(cambio.artifact.artifact_id.clone()));
        let actual = lectura
            .actual
            .as_mut()
            .ok_or_else(|| RemoteTaskError::Other("the stream must start with the task".into()))?;
        let artefacto = Artifact::try_from(cambio.artifact).map_err(invalida)?;
        match actual
            .artifacts
            .iter_mut()
            .find(|existente| existente.artifact_id == artefacto.artifact_id)
        {
            Some(existente) if cambio.append => existente.parts.extend(artefacto.parts),
            Some(existente) => *existente = artefacto,
            None => actual.artifacts.push(artefacto),
        }
        if actual.artifacts.len() > MAX_ARTEFACTOS_FLUJO
            || codeloom_core::a2a::artifacts_text(&actual.artifacts).len() > lectura.max_bytes
        {
            return Err(RemoteTaskError::Other(
                "the stream result is larger than allowed".into(),
            ));
        }
        observar_flujo(
            lectura,
            A2aOperation::TaskArtifactUpdateEvent,
            task_id,
            context_id,
            None,
            nombre,
        );
    } else {
        return Err(RemoteTaskError::Other(
            "unexpected message in a task stream".into(),
        ));
    }
    lectura
        .actual
        .clone()
        .ok_or_else(|| RemoteTaskError::Other("the stream must start with the task".into()))
}

pub(super) fn tarea_de_resultado(resultado: Value) -> Result<RemoteTask, RemoteTaskError> {
    let wire: WireTask = serde_json::from_value(resultado).map_err(invalida)?;
    tarea_desde_wire(wire)
}
