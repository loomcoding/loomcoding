//! Cliente JSON-RPC de un agente remoto (A8a, T4): `SendMessage`, `GetTask` y `CancelTask`.
//!
//! Cabe señalar que habla con la interfaz que la tarjeta aprobada declaró, envía siempre
//! `A2A-Version: 1.0`, acota el cuerpo de cada respuesta y no afirma nada que el remoto no diga:
//! un error de A2A se traduce a su equivalente y cualquier otra cosa es un error visible.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use codeloom_core::a2a::{Artifact, Message};
use codeloom_core::event::{A2aDirection, A2aObservation, A2aOperation, EventBus, LiveEvent};
use codeloom_core::registry::RemoteSpec;
use codeloom_core::remote_task::{
    RemoteClientFactory, RemoteReply, RemoteTask, RemoteTaskClient, RemoteTaskError,
};
use codeloom_core::task::TaskStatus;
use futures::stream::BoxStream;
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::card::VERSION_SOPORTADA;
use crate::config::A2aConfig;
use crate::remote::{http_client, http_client_with_identity, leer_acotado, mapear, RemoteError};
use crate::security::AuthPlan;
use crate::sse::SseDecoder;
use crate::wire::{
    state_from_wire, timestamp_from_wire, WireMessage, WireStreamResponse, WireTask,
};

mod factory;
mod stream;

pub use factory::A2aClientFactory;
use stream::*;

#[cfg(test)]
mod tests;

/// Cota de los textos de error de un tercero que se copian a un mensaje nuestro.
const MAX_TEXTO_ERROR: usize = 300;

// Códigos de error de A2A en JSON-RPC (§5.4).
const TASK_NOT_FOUND: i64 = -32001;

const TASK_NOT_CANCELABLE: i64 = -32002;

const UNSUPPORTED_OPERATION: i64 = -32004;

#[derive(Debug, Deserialize)]
struct RespuestaRpc {
    #[serde(default)]
    id: Value,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<ErrorRpc>,
}

#[derive(Debug, Deserialize)]
struct ErrorRpc {
    code: i64,
    #[serde(default)]
    message: String,
}

/// `SendMessageResponse`: exactamente una de `task` y `message`.
#[derive(Debug, Deserialize)]
struct RespuestaDeEnvio {
    #[serde(default)]
    task: Option<WireTask>,
    #[serde(default)]
    message: Option<WireMessage>,
}

/// Cliente de un remoto concreto.
pub struct JsonRpcClient {
    http: Client,
    url: String,
    tenant: Option<String>,
    max_result_bytes: usize,
    siguiente_id: AtomicU64,
    auth: AuthPlan,
    /// La tarjeta declara streaming: solo entonces se intenta `SubscribeToTask` (A8c).
    streaming: bool,
    agent: String,
    observer: Option<EventBus>,
}

impl JsonRpcClient {
    /// `url` es la de la interfaz JSON-RPC elegida en la tarjeta y `tenant` el que esa interfaz
    /// declara (se envía tal cual en cada petición, §8.3.2).
    pub fn new(
        http: Client,
        url: impl Into<String>,
        tenant: Option<String>,
        max_result_bytes: usize,
    ) -> Self {
        JsonRpcClient {
            http,
            url: url.into(),
            tenant,
            max_result_bytes,
            siguiente_id: AtomicU64::new(1),
            auth: AuthPlan::open(),
            streaming: false,
            agent: String::new(),
            observer: None,
        }
    }

    /// Indica que la tarjeta del remoto declara `capabilities.streaming = true`.
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Autentica cada petición con `auth`. Sin esto el cliente habla a un remoto abierto.
    pub fn with_auth(mut self, auth: AuthPlan) -> Self {
        self.auth = auth;
        self
    }

    fn with_observer(mut self, agent: &str, observer: Option<EventBus>) -> Self {
        self.agent = agent.to_string();
        self.observer = observer;
        self
    }

    fn observe(&self, observation: A2aObservation) {
        if let Some(bus) = &self.observer {
            bus.publish(LiveEvent::A2aObserved(observation));
        }
    }

    /// Envía una petición autenticada. Si el remoto rechaza un token (401), se olvida y se pide otro
    /// una sola vez: un token revocado o caducado antes de tiempo no debe tumbar la tarea.
    async fn enviar(
        &self,
        construir: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, RemoteTaskError> {
        let mut reintentado = false;
        loop {
            let peticion = self
                .auth
                .apply(construir())
                .await
                .map_err(RemoteTaskError::AuthFailed)?;
            let respuesta = peticion
                .send()
                .await
                .map_err(|e| traducir_fallo(mapear(e)))?;
            if respuesta.status() == reqwest::StatusCode::UNAUTHORIZED
                && !reintentado
                && self.auth.uses_token()
            {
                reintentado = true;
                self.auth.invalidate().await;
                continue;
            }
            return Ok(respuesta);
        }
    }

    fn con_tenant(&self, mut params: Value) -> Value {
        if let Some(tenant) = &self.tenant {
            params["tenant"] = json!(tenant);
        }
        params
    }

    async fn llamar(&self, metodo: &str, params: Value) -> Result<Value, RemoteTaskError> {
        let started = Instant::now();
        let operation = match metodo {
            "SendMessage" => A2aOperation::SendMessage,
            "GetTask" => A2aOperation::GetTask,
            "CancelTask" => A2aOperation::CancelTask,
            _ => return Err(RemoteTaskError::Unsupported(metodo.to_string())),
        };
        let requested_task = params
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| {
                params
                    .get("message")
                    .and_then(|message| message.get("taskId"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string);
        let requested_context = params
            .get("message")
            .and_then(|message| message.get("contextId"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let rpc_id = self.siguiente_id.fetch_add(1, Ordering::Relaxed);
        let response = self.llamar_inner(metodo, params, rpc_id).await;
        let task = response.as_ref().ok().and_then(|value| {
            value
                .get("task")
                .filter(|task| task.is_object())
                .or(Some(value))
        });
        self.observe(A2aObservation {
            direction: A2aDirection::Outbound,
            agent: self.agent.clone(),
            operation,
            rpc_id: Some(rpc_id.to_string()),
            task_id: requested_task.or_else(|| {
                task.and_then(|task| task.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
            context_id: requested_context.or_else(|| {
                task.and_then(|task| task.get("contextId"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
            state: task
                .and_then(|task| task.get("status"))
                .and_then(|status| status.get("state"))
                .and_then(Value::as_str)
                .map(str::to_string),
            artifact: None,
            succeeded: response.is_ok(),
            elapsed_ms: Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        });
        response
    }

    async fn llamar_inner(
        &self,
        metodo: &str,
        params: Value,
        id: u64,
    ) -> Result<Value, RemoteTaskError> {
        let cuerpo = json!({ "jsonrpc": "2.0", "id": id, "method": metodo, "params": params });
        let respuesta = self
            .enviar(|| {
                self.http
                    .post(&self.url)
                    .header("A2A-Version", VERSION_SOPORTADA)
                    .header(reqwest::header::ACCEPT, "application/json")
                    .json(&cuerpo)
            })
            .await?;
        let estado = respuesta.status();
        // El cuerpo de un rechazo no se lee ni se repite: podría citar la propia credencial.
        if estado == reqwest::StatusCode::UNAUTHORIZED || estado == reqwest::StatusCode::FORBIDDEN {
            return Err(RemoteTaskError::AuthFailed(format!(
                "HTTP {}; revisa la credencial declarada en `a2a.credentials`",
                estado.as_u16()
            )));
        }
        let bytes = leer_acotado(respuesta, self.max_result_bytes)
            .await
            .map_err(traducir_fallo)?;

        let rpc: RespuestaRpc = match serde_json::from_slice(&bytes) {
            Ok(rpc) => rpc,
            Err(_) if !estado.is_success() => {
                return Err(RemoteTaskError::Other(format!(
                    "HTTP status {}",
                    estado.as_u16()
                )))
            }
            Err(e) => {
                return Err(RemoteTaskError::Other(format!(
                    "response is not JSON-RPC: {e}"
                )))
            }
        };
        if rpc.id != json!(id) {
            return Err(RemoteTaskError::Other(
                "response id does not match the request".into(),
            ));
        }
        match (rpc.result, rpc.error) {
            (Some(resultado), None) => Ok(resultado),
            (None, Some(error)) => Err(traducir_error_rpc(error)),
            _ => Err(RemoteTaskError::Other(
                "response must carry exactly one of result and error".into(),
            )),
        }
    }
}

fn acotar(texto: &str) -> String {
    if texto.len() <= MAX_TEXTO_ERROR {
        return texto.to_string();
    }
    let mut fin = MAX_TEXTO_ERROR;
    while !texto.is_char_boundary(fin) {
        fin -= 1;
    }
    format!("{}…", &texto[..fin])
}

fn traducir_fallo(error: RemoteError) -> RemoteTaskError {
    match error {
        RemoteError::Timeout => RemoteTaskError::Timeout,
        otro => RemoteTaskError::Other(otro.to_string()),
    }
}

fn traducir_error_rpc(error: ErrorRpc) -> RemoteTaskError {
    match error.code {
        TASK_NOT_FOUND => RemoteTaskError::NotFound,
        TASK_NOT_CANCELABLE => RemoteTaskError::NotCancelable,
        UNSUPPORTED_OPERATION => RemoteTaskError::Unsupported(acotar(&error.message)),
        codigo => RemoteTaskError::Other(format!("code {codigo}: {}", acotar(&error.message))),
    }
}

fn invalida(error: impl std::fmt::Display) -> RemoteTaskError {
    RemoteTaskError::Other(format!("invalid response: {error}"))
}

/// Convierte una tarea del cable a la vista del núcleo. El estado ha de ser uno de los ocho.
fn tarea_desde_wire(wire: WireTask) -> Result<RemoteTask, RemoteTaskError> {
    let estado = state_from_wire(&wire.status.state).map_err(invalida)?;
    let mensaje = wire
        .status
        .message
        .map(Message::try_from)
        .transpose()
        .map_err(invalida)?;
    // Sin marca en el cable, la del momento de recibirla (la que pone `TaskStatus::new`).
    let mut status = TaskStatus::new(estado);
    status.message = mensaje;
    if let Some(texto) = &wire.status.timestamp {
        status.timestamp_ms = timestamp_from_wire(texto).map_err(invalida)?;
    }
    let artifacts = wire
        .artifacts
        .into_iter()
        .map(Artifact::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalida)?;
    Ok(RemoteTask {
        id: wire.id,
        context_id: wire.context_id,
        status,
        artifacts,
    })
}

#[async_trait]
impl RemoteTaskClient for JsonRpcClient {
    async fn send(&self, message: Message) -> Result<RemoteReply, RemoteTaskError> {
        // Sin `returnImmediately` el remoto bloquea hasta un estado terminal o interrumpido (§3.2.2).
        let params = self.con_tenant(json!({
            "message": WireMessage::from(&message),
            "configuration": { "returnImmediately": true },
        }));
        let resultado = self.llamar("SendMessage", params).await?;
        let respuesta: RespuestaDeEnvio = serde_json::from_value(resultado).map_err(invalida)?;
        match (respuesta.task, respuesta.message) {
            (Some(tarea), None) => Ok(RemoteReply::Task(tarea_desde_wire(tarea)?)),
            (None, Some(mensaje)) => Ok(RemoteReply::Message(
                Message::try_from(mensaje).map_err(invalida)?,
            )),
            _ => Err(RemoteTaskError::Other(
                "SendMessage must answer with exactly one of task and message".into(),
            )),
        }
    }

    async fn get(&self, id: &str) -> Result<RemoteTask, RemoteTaskError> {
        // `historyLength: 0`: no se pide historial, que no se usa y puede ser enorme.
        let params = self.con_tenant(json!({ "id": id, "historyLength": 0 }));
        tarea_de_resultado(self.llamar("GetTask", params).await?)
    }

    async fn cancel(&self, id: &str) -> Result<RemoteTask, RemoteTaskError> {
        let params = self.con_tenant(json!({ "id": id }));
        tarea_de_resultado(self.llamar("CancelTask", params).await?)
    }

    async fn watch(
        &self,
        id: &str,
    ) -> Result<BoxStream<'static, Result<RemoteTask, RemoteTaskError>>, RemoteTaskError> {
        let started = Instant::now();
        // Se comprueba la tarjeta antes de intentarlo (§3.3.4): sin streaming declarado, ni se pregunta.
        if !self.streaming {
            return Err(RemoteTaskError::Unsupported(
                "the remote card does not declare streaming".into(),
            ));
        }
        let rpc_id = self.siguiente_id.fetch_add(1, Ordering::Relaxed);
        let params = self.con_tenant(json!({ "id": id }));
        let cuerpo = json!({ "jsonrpc": "2.0", "id": rpc_id, "method": "SubscribeToTask", "params": params });
        let respuesta = self
            .enviar(|| {
                self.http
                    .post(&self.url)
                    .header("A2A-Version", VERSION_SOPORTADA)
                    .header(reqwest::header::ACCEPT, "text/event-stream")
                    .timeout(MAX_CONEXION_FLUJO)
                    .json(&cuerpo)
            })
            .await?;
        let estado = respuesta.status();
        if estado == reqwest::StatusCode::UNAUTHORIZED || estado == reqwest::StatusCode::FORBIDDEN {
            return Err(RemoteTaskError::AuthFailed(format!(
                "HTTP {}; revisa la credencial declarada en `a2a.credentials`",
                estado.as_u16()
            )));
        }
        let es_flujo = respuesta
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|valor| valor.to_str().ok())
            .is_some_and(|valor| valor.starts_with("text/event-stream"));
        if !es_flujo {
            // Un error previo al flujo viaja como JSON-RPC normal (§9.4.6).
            let bytes = leer_acotado(respuesta, self.max_result_bytes)
                .await
                .map_err(traducir_fallo)?;
            return match serde_json::from_slice::<RespuestaRpc>(&bytes) {
                Ok(RespuestaRpc {
                    error: Some(error), ..
                }) => Err(traducir_error_rpc(error)),
                _ => Err(RemoteTaskError::Other(
                    "the remote did not answer with an event stream".into(),
                )),
            };
        }
        let lectura = LecturaFlujo {
            cuerpo: respuesta.bytes_stream().boxed(),
            decodificador: SseDecoder::new(),
            pendientes: VecDeque::new(),
            actual: None,
            tarea: id.to_string(),
            rpc_id,
            max_bytes: self.max_result_bytes,
            terminado: false,
            agent: self.agent.clone(),
            observer: self.observer.clone(),
        };
        self.observe(A2aObservation {
            direction: A2aDirection::Outbound,
            agent: self.agent.clone(),
            operation: A2aOperation::SubscribeToTask,
            rpc_id: Some(rpc_id.to_string()),
            task_id: Some(id.to_string()),
            context_id: None,
            state: None,
            artifact: None,
            succeeded: true,
            elapsed_ms: Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        });
        Ok(futures::stream::unfold(lectura, |mut lectura| async move {
            loop {
                if lectura.terminado {
                    return None;
                }
                if let Some(datos) = lectura.pendientes.pop_front() {
                    return match aplicar_evento(&mut lectura, &datos) {
                        Ok(tarea) => Some((Ok(tarea), lectura)),
                        Err(error) => {
                            lectura.terminado = true;
                            Some((Err(error), lectura))
                        }
                    };
                }
                match lectura.cuerpo.next().await {
                    Some(Ok(trozo)) => match lectura.decodificador.push(&trozo) {
                        Ok(eventos) => lectura.pendientes.extend(eventos),
                        Err(error) => {
                            lectura.terminado = true;
                            return Some((Err(invalida(error)), lectura));
                        }
                    },
                    Some(Err(error)) => {
                        lectura.terminado = true;
                        return Some((Err(traducir_fallo(mapear(error))), lectura));
                    }
                    None => return None,
                }
            }
        })
        .boxed())
    }
}
