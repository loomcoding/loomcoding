//! Vigilancia de jobs (`watch`): troceo de la salida en líneas, emparejado con un patrón y
//! entrega acotada en lotes.
//!
//! Todo el módulo es puro: el tiempo entra como argumento (`Instant`), así que las pruebas
//! avanzan un reloj simulado sin dormir ni tocar la red. El estado compartido con el job vive
//! en `job.rs`; aquí solo está la lógica.

use crate::error::{CoreError, Result};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Longitud máxima del patrón, en bytes.
pub const MAX_PATTERN_BYTES: usize = 256;
/// Tamaño máximo del autómata compilado, en bytes.
pub const MAX_REGEX_BYTES: usize = 64 * 1024;
/// Avisos por defecto cuando no se indica `max_events`.
pub const DEFAULT_MAX_EVENTS: usize = 20;
/// Tope de `max_events`.
pub const MAX_EVENTS_CAP: usize = 100;
/// Resto de línea sin salto que se conserva para emparejarlo.
pub const MAX_PARTIAL_BYTES: usize = 4 * 1024;
/// Bytes máximos de cada línea avisada.
pub const MAX_EVENT_BYTES: usize = 512;
/// Intervalo mínimo entre dos lotes del mismo job.
pub const BATCH_INTERVAL: Duration = Duration::from_secs(5);
/// Líneas máximas por lote.
pub const MAX_LINES_PER_BATCH: usize = 10;
/// Líneas pendientes máximas; lo que no cabe se cuenta como suprimido.
pub const MAX_QUEUED_LINES: usize = 50;

fn tool_error(message: impl Into<String>) -> CoreError {
    CoreError::Tool(message.into())
}

/// Vigilancia pedida por el modelo, ya validada.
#[derive(Debug, Clone)]
pub struct WatchSpec {
    pub pattern: Regex,
    pub max_events: usize,
}

impl WatchSpec {
    /// Valida el patrón y el tope. `None` en `max_events` equivale a [`DEFAULT_MAX_EVENTS`].
    pub fn parse(pattern: &str, max_events: Option<usize>) -> Result<Self> {
        if pattern.is_empty() {
            return Err(tool_error("watch pattern must not be empty"));
        }
        if pattern.len() > MAX_PATTERN_BYTES {
            return Err(tool_error(format!(
                "watch pattern must be at most {MAX_PATTERN_BYTES} bytes"
            )));
        }
        let max_events = max_events.unwrap_or(DEFAULT_MAX_EVENTS);
        if !(1..=MAX_EVENTS_CAP).contains(&max_events) {
            return Err(tool_error(format!(
                "watch max_events must be from 1 to {MAX_EVENTS_CAP}"
            )));
        }
        let pattern = RegexBuilder::new(pattern)
            .size_limit(MAX_REGEX_BYTES)
            .build()
            .map_err(|error| tool_error(format!("invalid watch pattern: {error}")))?;
        Ok(Self {
            pattern,
            max_events,
        })
    }
}

/// Recorta `text` a `max` bytes como mucho, sin partir un carácter UTF-8.
fn truncate_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    // El índice 0 siempre es frontera de carácter, así que el bucle termina.
    let mut cut = max;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

/// Decodifica bytes de salida a texto conservando los bytes UTF-8 incompletos al final de cada
/// lectura, para que un carácter partido entre dos lecturas no se convierta en U+FFFD.
#[derive(Debug, Default)]
pub struct ChunkDecoder {
    pending: Vec<u8>,
}

impl ChunkDecoder {
    /// Devuelve el texto completo disponible; como mucho 3 bytes quedan retenidos.
    pub fn decode(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut text = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(valid) => {
                    text.push_str(valid);
                    self.pending.clear();
                    return text;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    text.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                    match error.error_len() {
                        // Secuencia incompleta al final: se espera a la siguiente lectura.
                        None => {
                            self.pending.drain(..valid);
                            return text;
                        }
                        Some(len) => {
                            text.push('\u{FFFD}');
                            self.pending.drain(..valid + len);
                        }
                    }
                }
            }
        }
    }

    /// Vuelca lo retenido al terminar el flujo, con sustitución de lo que quede incompleto.
    pub fn finish(&mut self) -> String {
        let rest = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        rest
    }
}

/// Trocea texto en líneas completas y devuelve las que casan con el patrón.
pub struct LineMatcher {
    pattern: Regex,
    partial: String,
    /// Verdadero mientras se descartan bytes de una línea que ya superó `MAX_PARTIAL_BYTES`.
    discarding: bool,
}

impl LineMatcher {
    pub fn new(pattern: Regex) -> Self {
        Self {
            pattern,
            partial: String::new(),
            discarding: false,
        }
    }

    /// Procesa `chunk` y devuelve las líneas completas que casan, en orden.
    pub fn push(&mut self, chunk: &str) -> Vec<String> {
        let mut found = Vec::new();
        for piece in chunk.split_inclusive('\n') {
            match piece.strip_suffix('\n') {
                Some(body) => {
                    self.keep(body);
                    found.extend(self.complete());
                }
                None => self.keep(piece),
            }
        }
        found
    }

    /// Cierra el flujo: el resto sin salto, si casa, también cuenta como línea.
    pub fn finish(&mut self) -> Vec<String> {
        if self.partial.is_empty() {
            self.discarding = false;
            return Vec::new();
        }
        self.complete().into_iter().collect()
    }

    /// Añade texto al resto de la línea en curso, hasta `MAX_PARTIAL_BYTES`.
    fn keep(&mut self, text: &str) {
        if self.discarding {
            return;
        }
        let room = MAX_PARTIAL_BYTES - self.partial.len();
        let kept = truncate_bytes(text, room);
        self.partial.push_str(kept);
        if kept.len() < text.len() {
            self.discarding = true;
        }
    }

    /// Cierra la línea en curso: quita los `\r` finales y devuelve la línea si casa.
    fn complete(&mut self) -> Option<String> {
        let mut line = std::mem::take(&mut self.partial);
        self.discarding = false;
        while line.ends_with('\r') {
            line.pop();
        }
        self.pattern.is_match(&line).then_some(line)
    }
}

/// Lote entregable: como mucho `MAX_LINES_PER_BATCH` líneas y el recuento de las que se
/// suprimieron desde el lote anterior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchBatch {
    pub lines: Vec<String>,
    pub suppressed: usize,
    /// Verdadero en el lote que anuncia que se agotó `max_events`.
    pub limit_reached: bool,
}

/// Observabilidad de una vigilancia, para `JobSnapshot` y `job_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchInfo {
    pub pattern: String,
    /// Líneas entregadas en lotes.
    pub delivered: usize,
    /// Líneas que no llegaron a entregarse (cola llena o resumidas en un lote).
    pub suppressed: usize,
    pub limit_reached: bool,
}

/// Cola de eventos con tope de avisos y agrupación por tiempo.
pub struct EventQueue {
    max_events: usize,
    matched: usize,
    lines: VecDeque<String>,
    pending_suppressed: usize,
    delivered: usize,
    suppressed: usize,
    limit_reached: bool,
    limit_announced: bool,
    last_batch: Option<Instant>,
}

impl EventQueue {
    pub fn new(max_events: usize) -> Self {
        Self {
            max_events,
            matched: 0,
            lines: VecDeque::new(),
            pending_suppressed: 0,
            delivered: 0,
            suppressed: 0,
            limit_reached: false,
            limit_announced: false,
            last_batch: None,
        }
    }

    /// Admite una línea que casó. Pasado `max_events`, las líneas siguientes se ignoran.
    pub fn push(&mut self, line: &str) {
        if self.matched >= self.max_events {
            return;
        }
        self.matched += 1;
        if self.matched == self.max_events {
            self.limit_reached = true;
        }
        let line = truncate_bytes(line, MAX_EVENT_BYTES).to_string();
        if self.lines.len() < MAX_QUEUED_LINES {
            self.lines.push_back(line);
        } else {
            self.pending_suppressed += 1;
            self.suppressed += 1;
        }
    }

    /// Devuelve el siguiente lote si ha pasado `BATCH_INTERVAL` desde el anterior. Las líneas que
    /// no caben en el lote se descartan y se cuentan como suprimidas.
    pub fn drain_batch(&mut self, now: Instant) -> Option<WatchBatch> {
        self.drain_batch_with(now, BATCH_INTERVAL)
    }

    /// ¿Entregaría `drain_batch_with` un lote en `now`? Es el mismo criterio sin consumir nada:
    /// hay algo que entregar (líneas, supresiones o el aviso de límite) y ha pasado `interval`
    /// desde el lote anterior.
    fn ready_with(&self, now: Instant, interval: Duration) -> bool {
        let announce = self.limit_reached && !self.limit_announced;
        if self.lines.is_empty() && self.pending_suppressed == 0 && !announce {
            return false;
        }
        match self.last_batch {
            Some(last) => now.saturating_duration_since(last) >= interval,
            None => true,
        }
    }

    /// Como [`EventQueue::drain_batch`], pero con el intervalo mínimo que se exige entre lotes.
    /// Un job asentado usa intervalo cero: ya no volverá a emitir líneas, así que su resto se
    /// entrega en el aviso final en vez de quedarse retenido.
    fn drain_batch_with(&mut self, now: Instant, interval: Duration) -> Option<WatchBatch> {
        if !self.ready_with(now, interval) {
            return None;
        }
        let announce = self.limit_reached && !self.limit_announced;
        let take = self.lines.len().min(MAX_LINES_PER_BATCH);
        let lines: Vec<String> = self.lines.drain(..take).collect();
        let rest = self.lines.len();
        self.lines.clear();
        self.suppressed += rest;
        let suppressed = std::mem::take(&mut self.pending_suppressed) + rest;
        self.delivered += lines.len();
        self.last_batch = Some(now);
        if announce {
            self.limit_announced = true;
        }
        Some(WatchBatch {
            lines,
            suppressed,
            limit_reached: announce,
        })
    }
}

/// Vigilancia activa de un job: emparejador más cola de eventos.
pub struct Watch {
    matcher: LineMatcher,
    queue: EventQueue,
    pattern: String,
    /// Tras `finish` el job ha terminado y ya no admite líneas nuevas.
    closed: bool,
}

impl Watch {
    pub fn new(spec: WatchSpec) -> Self {
        let pattern = spec.pattern.as_str().to_string();
        Self {
            matcher: LineMatcher::new(spec.pattern),
            queue: EventQueue::new(spec.max_events),
            pattern,
            closed: false,
        }
    }

    /// Alimenta la vigilancia con texto nuevo de la salida del job.
    pub fn feed(&mut self, text: &str) {
        if self.closed {
            return;
        }
        for line in self.matcher.push(text) {
            self.queue.push(&line);
        }
    }

    /// Cierra la vigilancia al terminar el job: el resto sin salto también se prueba.
    pub fn finish(&mut self) {
        if self.closed {
            return;
        }
        for line in self.matcher.finish() {
            self.queue.push(&line);
        }
        self.closed = true;
    }

    /// Intervalo que exige el siguiente lote. Tras `finish` no hay nada que esperar.
    fn interval(&self) -> Duration {
        if self.closed {
            Duration::ZERO
        } else {
            BATCH_INTERVAL
        }
    }

    /// Devuelve el lote listo en `now`. Tras `finish` no hay intervalo que esperar: el resto del
    /// job se entrega en cuanto se pide.
    pub fn drain(&mut self, now: Instant) -> Option<WatchBatch> {
        let interval = self.interval();
        self.queue.drain_batch_with(now, interval)
    }

    /// ¿Hay un lote que `drain` entregaría en `now`? No consume nada: es la consulta que usa el
    /// aviso `JobWatchReady` para despertar al padre ocioso.
    pub fn ready(&self, now: Instant) -> bool {
        self.queue.ready_with(now, self.interval())
    }

    pub fn info(&self) -> WatchInfo {
        WatchInfo {
            pattern: self.pattern.clone(),
            delivered: self.queue.delivered,
            suppressed: self.queue.suppressed,
            limit_reached: self.queue.limit_reached,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod registry_tests;

#[cfg(test)]
mod ready_tests;
