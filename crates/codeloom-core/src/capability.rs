//! Capacidades de tools por destino (protocolo, endpoint, modelo y ruta) y su evidencia.
//!
//! Cabe señalar que nada aquí hace red ni cambia permisos: es el registro puro que consultan
//! los adapters antes de enviar una petición. Lo que no se sabe se conserva como `Unknown`;
//! jamás se presenta como `Supported`. Spec: `docs/specs/2026-10-28-codeloom-capacidades-tools-provider-modelo.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Caducidad por defecto de una observación.
pub const DEFAULT_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Protocolo de la ruta que se va a llamar; dos protocolos nunca son intercambiables aunque
/// su JSON se parezca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    ChatCompletions,
    Responses,
    AnthropicMessages,
    Gemini,
}

/// Función que se quiere usar. Cada una se evalúa por separado: que una esté soportada no
/// prueba ninguna de las demás.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    DirectToolCalling,
    ParallelToolCalls,
    ToolStreaming,
    ProgrammaticToolCalling,
    CodeExecution,
    ToolSearch,
    ProviderManagedTools,
}

impl Capability {
    pub fn name(self) -> &'static str {
        match self {
            Capability::DirectToolCalling => "direct_tool_calling",
            Capability::ParallelToolCalls => "parallel_tool_calls",
            Capability::ToolStreaming => "tool_streaming",
            Capability::ProgrammaticToolCalling => "programmatic_tool_calling",
            Capability::CodeExecution => "code_execution",
            Capability::ToolSearch => "tool_search",
            Capability::ProviderManagedTools => "provider_managed_tools",
        }
    }
}

/// Qué implementa el adapter de cada protocolo. Es conocimiento local, sin red.
fn adapter_implements(protocol: Protocol, capability: Capability) -> bool {
    use Capability::{
        CodeExecution, DirectToolCalling, ParallelToolCalls, ProgrammaticToolCalling,
        ProviderManagedTools, ToolSearch, ToolStreaming,
    };
    match (protocol, capability) {
        (_, DirectToolCalling | ParallelToolCalls) => true,
        // Solo Chat Completions y Anthropic leen deltas de argumentos en streaming.
        (Protocol::ChatCompletions | Protocol::AnthropicMessages, ToolStreaming) => true,
        // PTC: Responses y Anthropic lo serializan; Gemini lo expresa con `code_execution`.
        (
            Protocol::Responses | Protocol::AnthropicMessages | Protocol::Gemini,
            ProgrammaticToolCalling,
        ) => true,
        (Protocol::AnthropicMessages | Protocol::Gemini, CodeExecution) => true,
        (_, ToolStreaming | ProgrammaticToolCalling | CodeExecution) => false,
        (_, ToolSearch | ProviderManagedTools) => false,
    }
}

/// Destino exacto de una petición. Cambiar cualquiera de sus campos es otro destino.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Target {
    pub protocol: Protocol,
    pub base_url: String,
    pub model: String,
    /// Proveedor de enrutamiento fijado (p. ej. en OpenRouter), si lo hay.
    pub route: Option<String>,
}

impl Target {
    pub fn new(protocol: Protocol, base_url: &str, model: &str) -> Self {
        let normalized = reqwest::Url::parse(base_url)
            .map(|url| url.as_str().to_string())
            .unwrap_or_else(|_| base_url.to_string());
        Self {
            protocol,
            base_url: normalized.trim_end_matches('/').to_string(),
            model: model.to_string(),
            route: None,
        }
    }

    pub fn routed_via(mut self, provider: &str) -> Self {
        self.route = Some(provider.to_string());
        self
    }
}

/// De dónde sale la evidencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Adapter,
    OfficialMetadata,
    Catalog,
    Response,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Source::Adapter => "adapter",
            Source::OfficialMetadata => "official metadata",
            Source::Catalog => "catalog",
            Source::Response => "real response",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Supported,
    Unsupported,
    Unknown,
    /// Una petición real fue rechazada. Solo se conserva el estado HTTP, nunca el cuerpo.
    Rejected {
        status: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub state: State,
    pub source: Option<Source>,
    pub observed_at_ms: Option<i64>,
}

impl Report {
    /// Aviso conciso con estado y fuente, sin cuerpos ni secretos.
    pub fn notice(&self, capability: Capability) -> String {
        let state = match self.state {
            State::Supported => "supported".to_string(),
            State::Unsupported => "unsupported".to_string(),
            State::Unknown => "unknown (could not be verified)".to_string(),
            State::Rejected { status } => format!("rejected (HTTP {status})"),
        };
        match self.source {
            Some(source) => format!("{}: {state} [{}]", capability.name(), source.name()),
            None => format!("{}: {state}", capability.name()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    state: State,
    source: Source,
    observed_at_ms: i64,
}

type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// Registro de observaciones por destino y capacidad, con caducidad.
pub struct CapabilityRegistry {
    ttl_ms: i64,
    clock: Clock,
    entries: Mutex<HashMap<(Target, Capability), Entry>>,
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_TTL)
    }
}

impl CapabilityRegistry {
    pub fn new(ttl: Duration) -> Self {
        Self::with_clock(ttl, Arc::new(crate::message::now_ms))
    }

    pub fn with_clock(ttl: Duration, clock: Clock) -> Self {
        Self {
            ttl_ms: i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX),
            clock,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn live(&self, entry: &Entry) -> bool {
        (self.clock)().saturating_sub(entry.observed_at_ms) < self.ttl_ms
    }

    /// Consulta por petición: lo que el adapter no implementa es `Unsupported`; si no hay
    /// evidencia vigente para este destino exacto, `Unknown`.
    pub fn query(&self, target: &Target, capability: Capability) -> Report {
        if !adapter_implements(target.protocol, capability) {
            return Report {
                state: State::Unsupported,
                source: Some(Source::Adapter),
                observed_at_ms: None,
            };
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        match entries.get(&(target.clone(), capability)) {
            Some(entry) if self.live(entry) => Report {
                state: entry.state,
                source: Some(entry.source),
                observed_at_ms: Some(entry.observed_at_ms),
            },
            _ => Report {
                state: State::Unknown,
                source: None,
                observed_at_ms: None,
            },
        }
    }

    /// Registra una observación positiva o negativa de metadata. Un rechazo real vigente
    /// no se revive con metadata: solo otra respuesta real o su caducidad lo sustituyen.
    pub fn record(&self, target: &Target, capability: Capability, supported: bool, source: Source) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let key = (target.clone(), capability);
        if let Some(existing) = entries.get(&key) {
            if matches!(existing.state, State::Rejected { .. })
                && self.live(existing)
                && source != Source::Response
            {
                return;
            }
        }
        entries.insert(
            key,
            Entry {
                state: if supported {
                    State::Supported
                } else {
                    State::Unsupported
                },
                source,
                observed_at_ms: (self.clock)(),
            },
        );
    }

    /// Registra el rechazo de una petición real: invalida la afirmación positiva de este
    /// destino y capacidad, y no se generaliza a otros modelos.
    pub fn record_rejection(&self, target: &Target, capability: Capability, status: u16) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.insert(
            (target.clone(), capability),
            Entry {
                state: State::Rejected { status },
                source: Source::Response,
                observed_at_ms: (self.clock)(),
            },
        );
    }
}

/// Clasificación de un rechazo. Solo `CapabilityIncompatible` autoriza un fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    CapabilityIncompatible(Capability),
    Auth,
    Quota,
    Transport,
    /// Petición inválida sin señal inequívoca de qué función falla: `HTTP 400` por sí solo
    /// no significa «no admite tools».
    InvalidRequest,
}

impl Rejection {
    pub fn allows_fallback(self) -> bool {
        matches!(self, Rejection::CapabilityIncompatible(_))
    }
}

/// Marcas inequívocas en el cuerpo de error de que la función pedida es la incompatible.
fn mentions_capability(body: &str, capability: Capability) -> bool {
    let body = body.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|needle| body.contains(needle));
    match capability {
        Capability::ProgrammaticToolCalling => any(&[
            "allowed_callers",
            "programmatic tool calling",
            "programmatic_tool_calling",
            "code_execution",
        ]),
        Capability::CodeExecution => any(&["code_execution"]),
        Capability::ToolSearch => any(&["tool_search"]),
        Capability::ParallelToolCalls => any(&["parallel_tool_calls"]),
        Capability::DirectToolCalling => any(&[
            "does not support tools",
            "does not support tool use",
            "tools are not supported",
            "tool use is not supported",
        ]),
        Capability::ToolStreaming | Capability::ProviderManagedTools => false,
    }
}

/// Clasifica un rechazo sin retener el cuerpo: devuelve solo la categoría.
pub fn classify_rejection(status: u16, body: &str, requested: Capability) -> Rejection {
    match status {
        401 | 403 => Rejection::Auth,
        402 | 429 => Rejection::Quota,
        408 | 500..=599 => Rejection::Transport,
        400 | 422 if mentions_capability(body, requested) => {
            Rejection::CapabilityIncompatible(requested)
        }
        _ => Rejection::InvalidRequest,
    }
}

fn parameters_of(value: &serde_json::Value) -> Option<Vec<&str>> {
    Some(
        value
            .get("supported_parameters")?
            .as_array()?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect(),
    )
}

/// Catálogo agregado de OpenRouter (`GET /api/v1/models`): `Some(true)` solo prueba que
/// **alguna** ruta del modelo acepta `tools`, no que la finalmente elegida lo haga; por eso
/// no basta para marcar el destino como `Supported`. `None` si el modelo o el campo faltan.
pub fn openrouter_catalog_lists_tools(models_json: &str, model_id: &str) -> Option<bool> {
    let value: serde_json::Value = serde_json::from_str(models_json).ok()?;
    let model = value
        .get("data")?
        .as_array()?
        .iter()
        .find(|model| model.get("id").and_then(serde_json::Value::as_str) == Some(model_id))?;
    Some(parameters_of(model)?.contains(&"tools"))
}

/// Endpoints de un modelo (`GET /api/v1/models/{id}/endpoints`): informa de `parameter` para
/// el proveedor fijado. `None` si no hay metadata de ese proveedor; si varios endpoints del
/// mismo proveedor discrepan, también `None` (no se afirma lo que no es unívoco).
pub fn openrouter_endpoint_supports(
    endpoints_json: &str,
    provider: &str,
    parameter: &str,
) -> Option<bool> {
    let value: serde_json::Value = serde_json::from_str(endpoints_json).ok()?;
    let verdicts: Vec<bool> = value
        .pointer("/data/endpoints")?
        .as_array()?
        .iter()
        .filter(|endpoint| {
            endpoint
                .get("provider_name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.eq_ignore_ascii_case(provider))
        })
        .filter_map(|endpoint| Some(parameters_of(endpoint)?.contains(&parameter)))
        .collect();
    let first = *verdicts.first()?;
    verdicts
        .iter()
        .all(|verdict| *verdict == first)
        .then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn registry() -> (CapabilityRegistry, Arc<AtomicI64>) {
        let now = Arc::new(AtomicI64::new(1_000));
        let clock = now.clone();
        (
            CapabilityRegistry::with_clock(
                Duration::from_secs(10),
                Arc::new(move || clock.load(Ordering::SeqCst)),
            ),
            now,
        )
    }

    fn chat(model: &str) -> Target {
        Target::new(
            Protocol::ChatCompletions,
            "https://openrouter.ai/api/v1/",
            model,
        )
    }

    #[test]
    fn missing_metadata_is_unknown_never_supported() {
        let (registry, _) = registry();
        let report = registry.query(
            &chat("anthropic/claude-haiku-4.5"),
            Capability::DirectToolCalling,
        );
        assert_eq!(report.state, State::Unknown);
        assert_eq!(report.source, None);
    }

    #[test]
    fn the_same_model_id_differs_by_protocol_endpoint_and_route() {
        let (registry, _) = registry();
        let model = "anthropic/claude-haiku-4.5";
        let a = chat(model);
        registry.record(
            &a,
            Capability::DirectToolCalling,
            true,
            Source::OfficialMetadata,
        );

        let other_protocol = Target::new(Protocol::AnthropicMessages, &a.base_url, model);
        let other_endpoint = Target::new(
            Protocol::ChatCompletions,
            "https://api.example.com/v1",
            model,
        );
        let routed = chat(model).routed_via("Amazon Bedrock");

        let cap = Capability::DirectToolCalling;
        assert_eq!(registry.query(&a, cap).state, State::Supported);
        assert_eq!(registry.query(&other_protocol, cap).state, State::Unknown);
        assert_eq!(registry.query(&other_endpoint, cap).state, State::Unknown);
        assert_eq!(registry.query(&routed, cap).state, State::Unknown);
    }

    #[test]
    fn trailing_slash_and_host_case_do_not_make_a_new_target() {
        let a = Target::new(
            Protocol::ChatCompletions,
            "https://OpenRouter.ai/api/v1/",
            "m",
        );
        let b = Target::new(
            Protocol::ChatCompletions,
            "https://openrouter.ai/api/v1",
            "m",
        );
        assert_eq!(a, b);
    }

    #[test]
    fn what_the_adapter_does_not_implement_is_unsupported_whatever_the_metadata_says() {
        let (registry, _) = registry();
        let target = chat("m");
        registry.record(
            &target,
            Capability::ProgrammaticToolCalling,
            true,
            Source::Catalog,
        );
        let report = registry.query(&target, Capability::ProgrammaticToolCalling);
        assert_eq!(report.state, State::Unsupported);
        assert_eq!(report.source, Some(Source::Adapter));
    }

    #[test]
    fn capabilities_are_independent() {
        let (registry, _) = registry();
        let target = Target::new(
            Protocol::AnthropicMessages,
            "https://api.anthropic.com",
            "claude-x",
        );
        registry.record(
            &target,
            Capability::DirectToolCalling,
            true,
            Source::OfficialMetadata,
        );
        assert_eq!(
            registry.query(&target, Capability::DirectToolCalling).state,
            State::Supported
        );
        for other in [
            Capability::ParallelToolCalls,
            Capability::ProgrammaticToolCalling,
            Capability::CodeExecution,
        ] {
            assert_eq!(
                registry.query(&target, other).state,
                State::Unknown,
                "{other:?}"
            );
        }
    }

    #[test]
    fn observations_expire() {
        let (registry, now) = registry();
        let target = chat("m");
        registry.record(
            &target,
            Capability::DirectToolCalling,
            true,
            Source::Catalog,
        );
        now.store(1_000 + 9_999, Ordering::SeqCst);
        assert_eq!(
            registry.query(&target, Capability::DirectToolCalling).state,
            State::Supported
        );
        now.store(1_000 + 10_000, Ordering::SeqCst);
        assert_eq!(
            registry.query(&target, Capability::DirectToolCalling).state,
            State::Unknown
        );
    }

    #[test]
    fn a_real_rejection_invalidates_the_positive_claim_only_for_that_target() {
        let (registry, _) = registry();
        let model = "anthropic/claude-haiku-4.5";
        let a = chat(model);
        let b = chat("other/model");
        let cap = Capability::DirectToolCalling;
        registry.record(&a, cap, true, Source::OfficialMetadata);
        registry.record(&b, cap, true, Source::OfficialMetadata);

        registry.record_rejection(&a, cap, 400);

        assert_eq!(
            registry.query(&a, cap).state,
            State::Rejected { status: 400 }
        );
        assert_eq!(registry.query(&b, cap).state, State::Supported);
    }

    #[test]
    fn metadata_does_not_revive_a_live_rejection_but_a_real_response_does() {
        let (registry, now) = registry();
        let target = chat("m");
        let cap = Capability::DirectToolCalling;
        registry.record_rejection(&target, cap, 400);

        registry.record(&target, cap, true, Source::Catalog);
        assert_eq!(
            registry.query(&target, cap).state,
            State::Rejected { status: 400 }
        );

        registry.record(&target, cap, true, Source::Response);
        assert_eq!(registry.query(&target, cap).state, State::Supported);

        registry.record_rejection(&target, cap, 422);
        now.store(1_000 + 10_000, Ordering::SeqCst);
        registry.record(&target, cap, true, Source::Catalog);
        assert_eq!(
            registry.query(&target, cap).state,
            State::Supported,
            "caducó el rechazo"
        );
    }

    #[test]
    fn notices_carry_state_and_source_but_no_body() {
        let (registry, _) = registry();
        let target = chat("m");
        registry.record_rejection(&target, Capability::DirectToolCalling, 400);
        let notice = registry
            .query(&target, Capability::DirectToolCalling)
            .notice(Capability::DirectToolCalling);
        assert_eq!(
            notice,
            "direct_tool_calling: rejected (HTTP 400) [real response]"
        );
        let unknown = registry
            .query(&chat("n"), Capability::ToolStreaming)
            .notice(Capability::ToolStreaming);
        assert_eq!(unknown, "tool_streaming: unknown (could not be verified)");
    }

    #[test]
    fn auth_quota_and_transport_never_allow_fallback() {
        let body = r#"{"error":{"message":"code_execution is not allowed"}}"#;
        for (status, expected) in [
            (401, Rejection::Auth),
            (403, Rejection::Auth),
            (402, Rejection::Quota),
            (429, Rejection::Quota),
            (500, Rejection::Transport),
            (503, Rejection::Transport),
            (408, Rejection::Transport),
        ] {
            let class = classify_rejection(status, body, Capability::ProgrammaticToolCalling);
            assert_eq!(class, expected, "HTTP {status}");
            assert!(!class.allows_fallback(), "HTTP {status}");
        }
    }

    #[test]
    fn a_bare_400_is_not_a_tools_incompatibility() {
        let class = classify_rejection(
            400,
            r#"{"error":{"message":"messages.2: unexpected role"}}"#,
            Capability::DirectToolCalling,
        );
        assert_eq!(class, Rejection::InvalidRequest);
        assert!(!class.allows_fallback());
    }

    #[test]
    fn an_explicit_capability_mismatch_allows_fallback_for_that_capability_only() {
        let body = r#"{"error":{"message":"Unknown field allowed_callers on tool"}}"#;
        let ptc = classify_rejection(400, body, Capability::ProgrammaticToolCalling);
        assert_eq!(
            ptc,
            Rejection::CapabilityIncompatible(Capability::ProgrammaticToolCalling)
        );
        assert!(ptc.allows_fallback());
        // Pedida otra función, la misma frase no autoriza nada.
        let other = classify_rejection(400, body, Capability::ToolSearch);
        assert_eq!(other, Rejection::InvalidRequest);
    }

    const MODELS: &str = r#"{"data":[
        {"id":"anthropic/claude-haiku-4.5","supported_parameters":["max_tokens","tools","tool_choice"]},
        {"id":"vendor/no-tools","supported_parameters":["max_tokens"]},
        {"id":"vendor/no-field"}
    ]}"#;

    #[test]
    fn openrouter_catalog_parses_tools_and_absence_is_none() {
        assert_eq!(
            openrouter_catalog_lists_tools(MODELS, "anthropic/claude-haiku-4.5"),
            Some(true)
        );
        assert_eq!(
            openrouter_catalog_lists_tools(MODELS, "vendor/no-tools"),
            Some(false)
        );
        assert_eq!(
            openrouter_catalog_lists_tools(MODELS, "vendor/no-field"),
            None
        );
        assert_eq!(
            openrouter_catalog_lists_tools(MODELS, "vendor/missing"),
            None
        );
        assert_eq!(openrouter_catalog_lists_tools("no es json", "x"), None);
    }

    const ENDPOINTS: &str = r#"{"data":{"id":"anthropic/claude-haiku-4.5","endpoints":[
        {"provider_name":"Anthropic","tag":"anthropic","supported_parameters":["tools","reasoning"]},
        {"provider_name":"Amazon Bedrock","tag":"amazon-bedrock/us","supported_parameters":["tools"]},
        {"provider_name":"Amazon Bedrock","tag":"amazon-bedrock/eu","supported_parameters":["max_tokens"]},
        {"provider_name":"Google","tag":"google-vertex","supported_parameters":["max_tokens"]}
    ]}}"#;

    #[test]
    fn openrouter_endpoints_answer_per_pinned_provider_and_refuse_ambiguity() {
        assert_eq!(
            openrouter_endpoint_supports(ENDPOINTS, "anthropic", "tools"),
            Some(true)
        );
        assert_eq!(
            openrouter_endpoint_supports(ENDPOINTS, "Anthropic", "reasoning"),
            Some(true)
        );
        assert_eq!(
            openrouter_endpoint_supports(ENDPOINTS, "Google", "tools"),
            Some(false)
        );
        // Dos endpoints de Bedrock discrepan: no se afirma nada.
        assert_eq!(
            openrouter_endpoint_supports(ENDPOINTS, "Amazon Bedrock", "tools"),
            None
        );
        assert_eq!(
            openrouter_endpoint_supports(ENDPOINTS, "Azure", "tools"),
            None
        );
    }

    #[test]
    fn the_aggregated_catalog_alone_never_marks_a_routed_target_supported() {
        // Criterio 3: con el catálogo agregado el registro no registra soporte del destino.
        let (registry, _) = registry();
        let target = chat("anthropic/claude-haiku-4.5");
        assert_eq!(
            openrouter_catalog_lists_tools(MODELS, &target.model),
            Some(true)
        );
        // El llamante solo registra lo que el catálogo prueba: nada sobre este destino.
        assert_eq!(
            registry.query(&target, Capability::DirectToolCalling).state,
            State::Unknown
        );
    }
}
