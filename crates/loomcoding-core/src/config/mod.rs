use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub mod context;
mod flows;
pub use flows::FlowsConfig;
mod limits;
pub use limits::{ProviderLimitsConfig, MAX_REQUEST_TIMEOUT_SECS};
mod agent_limits;
pub use agent_limits::LimitsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ProviderKind {
    #[serde(rename = "openai_compat")]
    OpenAiCompat,
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "gemini")]
    Gemini,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    /// Variable de entorno con la clave; vacía o ausente si el servidor no la pide (bucle local).
    #[serde(default)]
    pub api_key_env: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoomcodingConfig {
    pub provider: ProviderConfig,
    pub model: String,
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    /// Nivel de esfuerzo de razonamiento; `None` equivale a «default» y no se escribe en el fichero.
    #[serde(default)]
    pub effort: Option<String>,
}

fn default_max_steps() -> usize {
    30
}

/// Tope de llamadas a herramienta en una sola respuesta del proveedor.
///
/// Corta el bucle de un modelo que encadena herramientas sin cerrar respuesta. El valor por
/// defecto es `DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE`; se puede subir sin recompilar con la
/// variable de entorno `LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE`.
pub fn max_tool_calls_per_response() -> usize {
    limite_por_entorno(
        "LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE",
        DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE,
    )
}

/// Tope de llamadas a herramienta acumuladas en un turno completo (varias respuestas).
///
/// Se puede subir sin recompilar con `LOOMCODING_MAX_TOOL_CALLS_PER_TURN`.
pub fn max_tool_calls_per_turn() -> usize {
    limite_por_entorno(
        "LOOMCODING_MAX_TOOL_CALLS_PER_TURN",
        DEFAULT_MAX_TOOL_CALLS_PER_TURN,
    )
}

/// Valor por defecto del tope por respuesta.
pub const DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE: usize = 128;

/// Valor por defecto del tope por turno.
pub const DEFAULT_MAX_TOOL_CALLS_PER_TURN: usize = 256;

fn limite_por_entorno(nombre: &str, por_defecto: usize) -> usize {
    std::env::var(nombre)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(por_defecto)
}

/// Modo de confirmación al delegar en un subagente (`[agents] confirm_delegation`).
///
/// Solo cambia el texto de la regla que el prompt del padre recibe; no es una garantía
/// de comportamiento, pues el cumplimiento depende del modelo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmDelegation {
    /// Pregunta a la persona usuaria solo cuando duda entre varios agentes. Es el valor por defecto.
    #[default]
    Doubt,
    /// Propone la asignación de cada tarea y espera la aprobación antes de delegar.
    Always,
    /// Delega sin preguntar y explica la elección en su respuesta.
    Never,
}

/// Cuánto delega el agente (`[agents] delegation`, C1). Sin valor, el comportamiento actual.
///
/// Solo `off` es una garantía de código; `on_request` y `proactive` son reglas del prompt, y su
/// cumplimiento depende del modelo. Concepto propio de Loomcoding, no de A2A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationMode {
    /// El modelo no recibe las herramientas de subagentes ni la sección de agentes.
    Off,
    /// Delega solo si la persona usuaria lo pide expresamente.
    OnRequest,
    /// Delega de forma proactiva las subtareas independientes y autocontenidas.
    Proactive,
}

impl DelegationMode {
    /// Regla que se añade al prompt de sistema; `Off` no añade ninguna porque retira las herramientas.
    pub fn rule(self) -> Option<&'static str> {
        match self {
            DelegationMode::Off => None,
            DelegationMode::OnRequest => Some(concat!(
                "## Delegation\n\nDelegate to a subagent only when the user explicitly asks you to. ",
                "Otherwise do the work yourself."
            )),
            DelegationMode::Proactive => Some(concat!(
                "## Delegation\n\nDelegate proactively: hand independent, self-contained subtasks to ",
                "subagents whenever that keeps your own context small."
            )),
        }
    }
}

/// Elección automática de agente al delegar (`[agents] routing`, A12b).
///
/// Cabe señalar que A2A no fija cómo se elige un agente: es decisión propia, y por eso va apagada
/// por defecto hasta que se mida lo que ahorra.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingMode {
    /// Sin enrutador: el padre elige como hasta ahora. Es el valor por defecto.
    #[default]
    Off,
    /// Regla determinista sobre las etiquetas de las tarjetas, sin llamar al modelo.
    Rule,
    /// La regla primero y, si se abstiene, un clasificador barato con salida estricta.
    Classifier,
}

/// Sección opcional `[agents]` de `loomcoding.toml`. Sin ella, todo son valores por defecto.
/// Vive aparte de `LoomcodingConfig` para que su ausencia no afecte a la configuración principal.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Directorio de agentes globales. Un `~/` inicial se expande al directorio personal.
    pub global_dir: Option<PathBuf>,
    /// Directorio de agentes del proyecto. Una ruta relativa cuelga de la raíz del proyecto.
    pub project_dir: Option<PathBuf>,
    /// Aprueba todos los agentes de proyecto sin preguntar. Por defecto, desactivado.
    pub trust_project_agents: bool,
    /// Cuándo se pregunta a la persona usuaria antes de delegar. Por defecto, `doubt`.
    pub confirm_delegation: ConfirmDelegation,
    /// Elección automática de agente al delegar. Por defecto, `off`.
    pub routing: RoutingMode,
    /// Modelo del clasificador (solo con `routing = "classifier"`). Sin él, el del padre.
    pub routing_model: Option<String>,
    /// Mando de delegación. Sin valor, el comportamiento actual.
    pub delegation: Option<DelegationMode>,
    /// Subagentes activos a la vez. Sin valor, `DEFAULT_ACTIVE_SUBAGENTS`; ver `active_subagents`.
    pub max_active_subagents: Option<usize>,
    /// Techo de profundidad de la delegación. Sin valor, `DEFAULT_DEPTH_CEILING`; ver `depth_ceiling`.
    pub max_depth: Option<u32>,
    /// Segundos sin latido tras los que un subagente se da por atascado (`Stalled`).
    /// Sin valor, `DEFAULT_STALL_THRESHOLD_SECS`; ver `stall_threshold_ms`.
    pub stall_threshold_secs: Option<u64>,
    /// Segundos sin latido tras los que un subagente se da por perdido (`Lost`).
    /// Sin valor, `DEFAULT_LOST_THRESHOLD_SECS`; ver `lost_threshold_ms`.
    pub lost_threshold_secs: Option<u64>,
    /// Qué se hace con un subagente dado por perdido. Por defecto, `notify`.
    pub lost_action: LostAction,
    /// Raíces de catálogos externos de agentes y skills (`<raíz>/.agents/agents` y `<raíz>/.agents/skills`).
    /// Sus agentes son contenido de terceros y exigen aprobación; ver `catalog_roots_effective`.
    pub catalog_roots: Vec<PathBuf>,
}

/// Cota de raíces de catálogo: más allá, una configuración equivocada leería carpetas sin fin.
pub const MAX_CATALOG_ROOTS: usize = 8;

/// Techo de profundidad por defecto: 1 es la delegación sin recursión, la de siempre.
pub const DEFAULT_DEPTH_CEILING: u32 = 1;
/// Cota superior del techo de profundidad: más niveles multiplicarían el coste sin control.
pub const MAX_DEPTH_CEILING: u32 = 3;

/// Subagentes activos a la vez por defecto.
pub const DEFAULT_ACTIVE_SUBAGENTS: usize = 8;
/// Cota superior de `max_active_subagents`: más allá, un valor equivocado agotaría el proveedor.
pub const MAX_ACTIVE_SUBAGENTS: usize = 64;

/// Umbral de atasco por defecto: dos minutos sin latido.
pub const DEFAULT_STALL_THRESHOLD_SECS: u64 = 120;
/// Cota inferior del atasco: por debajo, el ruido de planificación o una pausa de red normal dispara el vigilante.
pub const MIN_STALL_THRESHOLD_SECS: u64 = 10;
/// Cota superior del atasco: más de una hora ya no aporta nada que una cancelación manual no cubra.
pub const MAX_STALL_THRESHOLD_SECS: u64 = 3600;
/// Umbral de pérdida por defecto: cinco minutos sin latido.
pub const DEFAULT_LOST_THRESHOLD_SECS: u64 = 300;
/// Cota inferior de la pérdida (mismo razonamiento que la del atasco).
pub const MIN_LOST_THRESHOLD_SECS: u64 = 30;
/// Cota superior de la pérdida: seis horas; más allá, cancelar a mano es lo sensato.
pub const MAX_LOST_THRESHOLD_SECS: u64 = 21600;
/// Margen mínimo entre atasco y pérdida: con un empate `Lost` ganaría en el mismo instante
/// y `Stalled` jamás sería observable, perdiendo la ventana en la que un latido tardío rescata.
pub const LOST_MARGIN_MS: u64 = 30_000;

/// Qué hace el supervisor con un subagente dado por perdido (`[agents] lost_action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LostAction {
    /// Solo marcarlo y avisar: la tarea sigue como estaba.
    #[default]
    Notify,
    /// Asentar la tarea como `failed`, con el motivo, y detener su trabajo.
    Fail,
}

/// Forma del fichero a efectos de la sección `[agents]`; el resto de claves se ignora.
#[derive(Deserialize)]
struct FicheroConAgentes {
    #[serde(default)]
    agents: AgentsConfig,
}

impl AgentsConfig {
    /// Techo de profundidad efectivo: el configurado, acotado a `1..=MAX_DEPTH_CEILING`, o 1 si no se configuró.
    pub fn depth_ceiling(&self) -> u32 {
        self.max_depth
            .map_or(DEFAULT_DEPTH_CEILING, |n| n.clamp(1, MAX_DEPTH_CEILING))
    }

    /// Raíces de catálogo efectivas: las configuradas sin repetir y sin vacías, como mucho `MAX_CATALOG_ROOTS`.
    pub fn catalog_roots_effective(&self) -> Vec<PathBuf> {
        let mut raices: Vec<PathBuf> = Vec::new();
        for raiz in &self.catalog_roots {
            if raiz.as_os_str().is_empty() || raices.contains(raiz) {
                continue;
            }
            raices.push(raiz.clone());
        }
        raices.truncate(MAX_CATALOG_ROOTS);
        raices
    }

    /// Límite efectivo de subagentes activos: el configurado, acotado a `1..=MAX_ACTIVE_SUBAGENTS`,
    /// o el valor por defecto si no se configuró.
    pub fn active_subagents(&self) -> usize {
        self.max_active_subagents
            .map_or(DEFAULT_ACTIVE_SUBAGENTS, |n| {
                n.clamp(1, MAX_ACTIVE_SUBAGENTS)
            })
    }

    /// Umbral de atasco en milisegundos (listo para compararlo con `last_activity_at_ms`):
    /// el configurado, acotado a `MIN..=MAX_STALL_THRESHOLD_SECS`, o el valor por defecto.
    pub fn stall_threshold_ms(&self) -> u64 {
        self.stall_threshold_secs
            .unwrap_or(DEFAULT_STALL_THRESHOLD_SECS)
            .clamp(MIN_STALL_THRESHOLD_SECS, MAX_STALL_THRESHOLD_SECS)
            * 1000
    }

    /// Umbral de pérdida en milisegundos. Tras acotarlo se normaliza para que supere al de
    /// atasco al menos en `LOST_MARGIN_MS`: un subagente no puede perderse antes de atascarse.
    pub fn lost_threshold_ms(&self) -> u64 {
        let lost = self
            .lost_threshold_secs
            .unwrap_or(DEFAULT_LOST_THRESHOLD_SECS)
            .clamp(MIN_LOST_THRESHOLD_SECS, MAX_LOST_THRESHOLD_SECS)
            * 1000;
        lost.max(self.stall_threshold_ms() + LOST_MARGIN_MS)
    }

    /// Lee la sección `[agents]` de un texto TOML. Su ausencia da los valores por defecto.
    pub fn from_toml(text: &str) -> Result<AgentsConfig> {
        toml::from_str::<FicheroConAgentes>(text)
            .map(|fichero| fichero.agents)
            .map_err(|e| CoreError::Config(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<AgentsConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }
}

/// Clave opcional `prompt_cache` de la sección `[provider]`. Va en un cargador aparte para no añadir un
/// campo a `ProviderConfig`, que se construye en muchos sitios; el resto de claves de `[provider]` se ignora.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ProviderCacheConfig {
    /// Puntos de caché de prompts del adaptador de Anthropic. Sin valor, decide el resto de la precedencia.
    pub prompt_cache: Option<bool>,
}

/// Forma del fichero a efectos de `[provider] prompt_cache`.
#[derive(Deserialize)]
struct FicheroConProveedor {
    #[serde(default)]
    provider: ProviderCacheConfig,
}

impl ProviderCacheConfig {
    /// Lee `[provider] prompt_cache` de un texto TOML. Su ausencia da `None`.
    pub fn from_toml(text: &str) -> Result<ProviderCacheConfig> {
        toml::from_str::<FicheroConProveedor>(text)
            .map(|fichero| fichero.provider)
            .map_err(|e| CoreError::Config(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<ProviderCacheConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }
}

/// Sección opcional `[ui]` de `loomcoding.toml`. Sin ella, todo son valores por defecto.
/// Vive aparte de `LoomcodingConfig` para no añadir un campo que se construye en muchos sitios.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Mostrar en el transcript los avisos de proveedor («request started» / «response received»).
    /// Por defecto, desactivado: en su lugar la barra de estado muestra un spinner mientras hay
    /// una petición en vuelo.
    pub verbose_provider_notices: bool,
}

/// Forma del fichero a efectos de `[ui]`.
#[derive(Deserialize)]
struct FicheroConUi {
    #[serde(default)]
    ui: UiConfig,
}

impl UiConfig {
    /// Lee la sección `[ui]` de un texto TOML. Su ausencia da los valores por defecto.
    pub fn from_toml(text: &str) -> Result<UiConfig> {
        toml::from_str::<FicheroConUi>(text)
            .map(|fichero| fichero.ui)
            .map_err(|e| CoreError::Config(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<UiConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml(&text)
    }
}

/// Dice si el host de `url` es `localhost` o una IP de bucle (`127.0.0.1`, `::1`).
fn host_de_bucle(url: &reqwest::Url) -> bool {
    url.host_str().is_some_and(|host| {
        // Los literales IPv6 llegan entre corchetes: `[::1]`.
        let sin_corchetes = host.trim_start_matches('[').trim_end_matches(']');
        host == "localhost"
            || sin_corchetes
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

/// Dice si `input` es una URL de bucle local (`localhost` o IP de loopback), sea cual sea su esquema.
pub fn is_loopback_url(input: &str) -> bool {
    reqwest::Url::parse(input).is_ok_and(|url| host_de_bucle(&url))
}

pub fn validate_base_url(input: &str) -> std::result::Result<(), String> {
    let url = reqwest::Url::parse(input).map_err(|_| "Base URL is invalid".to_string())?;
    let local_http = url.scheme() == "http" && host_de_bucle(&url);
    if url.scheme() != "https" && !local_http {
        return Err("Use HTTPS, or HTTP for a localhost endpoint".into());
    }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() {
        return Err(
            "Keep credentials out of the Base URL; use the API key environment variable".into(),
        );
    }
    Ok(())
}

impl LoomcodingConfig {
    /// Creates a usable project configuration on first launch without credentials.
    pub fn load_or_create(path: &Path) -> Result<LoomcodingConfig> {
        Self::load_or_create_with(path, None)
    }

    /// Como `load_or_create`, pero si el fichero no existe lo crea con `initial` en vez del
    /// valor por defecto de OpenRouter. Un fichero existente nunca se toca.
    pub fn load_or_create_with(path: &Path, initial: Option<&str>) -> Result<LoomcodingConfig> {
        const DEFAULT: &str = "# Loomcoding project settings. Add the API key in /settings.\nmodel = \"nvidia/nemotron-3.5-super-vl:free\"\nmax_steps = 8\n\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://openrouter.ai/api/v1\"\napi_key_env = \"OPENROUTER_API_KEY\"\n";
        let contenido = initial.unwrap_or(DEFAULT);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut file) => file
                .write_all(contenido.as_bytes())
                .map_err(|e| CoreError::Config(format!("write {}: {e}", path.display())))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Self::load(path)
                    .map_err(|_| CoreError::Config(format!("create {}: {error}", path.display())))
            }
            Err(error) => {
                return Err(CoreError::Config(format!(
                    "create {}: {error}",
                    path.display()
                )))
            }
        }
        Self::load(path)
    }

    pub fn load(path: &Path) -> Result<LoomcodingConfig> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?;
        toml::from_str(&text).map_err(|e| CoreError::Config(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let mut document = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("read {}: {e}", path.display())))?
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| CoreError::Config(format!("parse {}: {e}", path.display())))?;
        document["model"] = toml_edit::value(self.model.clone());
        document["provider"]["kind"] = toml_edit::value(match self.provider.kind {
            ProviderKind::OpenAiCompat => "openai_compat",
            ProviderKind::OpenAiResponses => "openai_responses",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Gemini => "gemini",
        });
        document["provider"]["base_url"] = toml_edit::value(self.provider.base_url.clone());
        document["provider"]["api_key_env"] = toml_edit::value(self.provider.api_key_env.clone());
        match &self.effort {
            Some(effort) => document["effort"] = toml_edit::value(effort.clone()),
            None => {
                document.remove("effort");
            }
        }
        escribir_config(path, &document.to_string())
    }
}

/// Escribe `text` en `path` de forma atómica: primero en un temporal junto al fichero y luego
/// renombrado, para que un corte no deje la configuración a medias.
fn escribir_config(path: &Path, text: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut temp_file = None;
    for attempt in 0..100 {
        let candidate = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), attempt));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temp_file = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(CoreError::Config(format!(
                    "create config temp file: {error}"
                )))
            }
        }
    }
    let (temp_path, mut temp) =
        temp_file.ok_or_else(|| CoreError::Config("cannot create temporary config file".into()))?;
    let result = (|| {
        temp.write_all(text.as_bytes())?;
        temp.sync_all()?;
        std::fs::rename(&temp_path, path)?;
        Ok::<_, std::io::Error>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result.map_err(|e| CoreError::Config(format!("write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn ui_config_defaults_to_quiet_provider_notices() {
        let ui = UiConfig::from_toml("model = \"x\"\n").unwrap();
        assert!(!ui.verbose_provider_notices);
    }

    #[test]
    fn ui_config_reads_the_verbose_flag() {
        let ui = UiConfig::from_toml("[ui]\nverbose_provider_notices = true\n").unwrap();
        assert!(ui.verbose_provider_notices);
    }

    #[test]
    fn ui_config_ignores_the_rest_of_the_file() {
        let ui = UiConfig::from_toml(
            "model = \"x\"\nmax_steps = 3\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://a.test\"\napi_key_env = \"K\"\n[ui]\nverbose_provider_notices = true\n",
        )
        .unwrap();
        assert!(ui.verbose_provider_notices);
    }

    #[test]
    fn parses_minimal_config() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, r#"
            model = "deepseek-chat"
            provider = {{ kind = "openai_compat", base_url = "https://api.deepseek.com", api_key_env = "DEEPSEEK_API_KEY" }}
            max_steps = 20
        "#).unwrap();
        let cfg = LoomcodingConfig::load(f.path()).unwrap();
        assert_eq!(cfg.model, "deepseek-chat");
        assert_eq!(cfg.max_steps, 20);
        assert_eq!(cfg.provider.kind, ProviderKind::OpenAiCompat);
    }

    #[test]
    fn defaults_max_steps() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, r#"
            model = "gpt-4o"
            provider = {{ kind = "openai_compat", base_url = "https://api.openai.com", api_key_env = "OPENAI_API_KEY" }}
        "#).unwrap();
        let cfg = LoomcodingConfig::load(f.path()).unwrap();
        assert_eq!(cfg.max_steps, 30);
    }

    #[test]
    fn tool_call_limits_have_sane_defaults() {
        assert_eq!(DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE, 128);
        assert_eq!(DEFAULT_MAX_TOOL_CALLS_PER_TURN, 256);
    }

    #[test]
    fn tool_call_limits_can_be_raised_by_environment() {
        std::env::set_var("LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE", "500");
        std::env::remove_var("LOOMCODING_MAX_TOOL_CALLS_PER_TURN");
        assert_eq!(max_tool_calls_per_response(), 500);
        assert_eq!(max_tool_calls_per_turn(), DEFAULT_MAX_TOOL_CALLS_PER_TURN);
        std::env::remove_var("LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE");
    }

    #[test]
    fn tool_call_limits_ignore_invalid_values() {
        std::env::set_var("LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE", "no-es-un-numero");
        assert_eq!(
            max_tool_calls_per_response(),
            DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE
        );
        std::env::set_var("LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE", "0");
        assert_eq!(
            max_tool_calls_per_response(),
            DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE
        );
        std::env::remove_var("LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE");
    }

    #[test]
    fn saves_and_reloads_provider_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        std::fs::write(
            &path,
            "# preserved\nmodel = \"old\"\nmax_steps = 7\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://old.test\"\napi_key_env = \"OLD_KEY\"\n",
        )
        .unwrap();
        let config = LoomcodingConfig {
            provider: ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url: "https://api.anthropic.com".into(),
                api_key_env: "ANTHROPIC_API_KEY".into(),
            },
            model: "claude-test".into(),
            max_steps: 7,
            effort: None,
        };
        config.save(&path).unwrap();
        let loaded = LoomcodingConfig::load(&path).unwrap();
        assert_eq!(loaded.model, "claude-test");
        assert_eq!(loaded.provider.kind, ProviderKind::Anthropic);
        assert_eq!(loaded.provider.api_key_env, "ANTHROPIC_API_KEY");
        assert_eq!(loaded.max_steps, 7);
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("# preserved"));
    }

    #[test]
    fn guarda_y_relee_una_variable_de_clave_vacia() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        std::fs::write(
            &path,
            "model = \"old\"\n[provider]\nkind = \"openai_compat\"\nbase_url = \"http://127.0.0.1:8081/v1\"\napi_key_env = \"OLD_KEY\"\n",
        )
        .unwrap();
        let config = LoomcodingConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://127.0.0.1:8081/v1".into(),
                api_key_env: String::new(),
            },
            model: "Ornith-1.5-9B".into(),
            max_steps: 8,
            effort: None,
        };
        config.save(&path).unwrap();
        let loaded = LoomcodingConfig::load(&path).unwrap();
        assert_eq!(loaded.provider.api_key_env, "");
        assert_eq!(loaded.model, "Ornith-1.5-9B");
    }

    #[test]
    fn config_without_effort_still_loads() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, r#"
            model = "gpt-4o"
            provider = {{ kind = "openai_compat", base_url = "https://api.openai.com", api_key_env = "OPENAI_API_KEY" }}
        "#).unwrap();
        let cfg = LoomcodingConfig::load(f.path()).unwrap();
        assert_eq!(cfg.effort, None, "una config antigua equivale a «default»");
    }

    #[test]
    fn config_roundtrips_effort() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loomcoding.toml");
        std::fs::write(
            &path,
            "# preserved\nmodel = \"old\"\nmax_steps = 7\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://old.test\"\napi_key_env = \"OLD_KEY\"\n",
        )
        .unwrap();
        let mut config = LoomcodingConfig {
            provider: ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url: "https://api.anthropic.com".into(),
                api_key_env: "ANTHROPIC_API_KEY".into(),
            },
            model: "claude-test".into(),
            max_steps: 7,
            effort: Some("high".into()),
        };
        config.save(&path).unwrap();
        let loaded = LoomcodingConfig::load(&path).unwrap();
        assert_eq!(loaded.effort.as_deref(), Some("high"));
        assert_eq!(loaded.provider.kind, ProviderKind::Anthropic);

        // Con `None` la clave desaparece del fichero y al releerlo no hay esfuerzo.
        config.effort = None;
        config.save(&path).unwrap();
        let loaded = LoomcodingConfig::load(&path).unwrap();
        assert_eq!(loaded.effort, None);
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("# preserved"));
    }

    #[test]
    fn accepts_secure_or_local_provider_urls_without_embedded_credentials() {
        assert!(validate_base_url("https://api.example.test/v1").is_ok());
        assert!(validate_base_url("http://127.0.0.1:8000/v1").is_ok());
        assert!(validate_base_url("http://api.example.test/v1").is_err());
        assert!(validate_base_url("https://user:secret@example.test/v1").is_err());
    }

    #[test]
    fn is_loopback_url_reconoce_el_bucle_local() {
        assert!(is_loopback_url("http://127.0.0.1:8081/v1"));
        assert!(is_loopback_url("http://localhost:8080/v1"));
        assert!(is_loopback_url("http://[::1]:8082/v1"));
        assert!(!is_loopback_url("https://api.example.test/v1"));
        assert!(!is_loopback_url("no es una url"));
    }

    #[test]
    fn agents_section_is_optional_in_config() {
        let text = "model = \"gpt-4o\"\nprovider = { kind = \"openai_compat\", base_url = \"https://api.openai.com\", api_key_env = \"OPENAI_API_KEY\" }\n";
        let agents = AgentsConfig::from_toml(text).unwrap();
        assert_eq!(agents, AgentsConfig::default());
        assert_eq!(agents.global_dir, None);
        assert_eq!(agents.project_dir, None);
        assert!(
            !agents.trust_project_agents,
            "por defecto no se confía en proyectos"
        );
    }

    #[test]
    fn agents_config_parses_dirs_and_trust_flag() {
        let text = r#"
            model = "gpt-4o"
            provider = { kind = "openai_compat", base_url = "https://api.openai.com", api_key_env = "OPENAI_API_KEY" }
            [agents]
            global_dir = "~/mis-agentes"
            project_dir = "config/agentes"
            trust_project_agents = true
        "#;
        let agents = AgentsConfig::from_toml(text).unwrap();
        assert_eq!(agents.global_dir, Some(PathBuf::from("~/mis-agentes")));
        assert_eq!(agents.project_dir, Some(PathBuf::from("config/agentes")));
        assert!(agents.trust_project_agents);
        // La sección convive con el resto: la configuración principal sigue leyéndose igual.
        let cfg: LoomcodingConfig = toml::from_str(text).unwrap();
        assert_eq!(cfg.model, "gpt-4o");
    }

    #[test]
    fn catalog_roots_are_optional_deduplicated_and_capped() {
        assert!(AgentsConfig::default().catalog_roots_effective().is_empty());
        let mut texto = String::from(
            "[agents]
catalog_roots = [\"/a\", \"/a\", \"\"",
        );
        for n in 0..12 {
            texto.push_str(&format!(", \"/r{n}\""));
        }
        texto.push_str(
            "]
",
        );
        let agents = AgentsConfig::from_toml(&texto).unwrap();
        let raices = agents.catalog_roots_effective();
        assert_eq!(raices.len(), MAX_CATALOG_ROOTS);
        assert_eq!(raices[0], PathBuf::from("/a"));
        assert_eq!(raices[1], PathBuf::from("/r0"));
    }

    #[test]
    fn provider_prompt_cache_is_optional_and_boolean() {
        let base = "model = \"m\"\n[provider]\nkind = \"anthropic\"\n";
        assert_eq!(
            ProviderCacheConfig::from_toml("").unwrap().prompt_cache,
            None
        );
        assert_eq!(
            ProviderCacheConfig::from_toml(base).unwrap().prompt_cache,
            None
        );
        for (texto, esperado) in [("true", true), ("false", false)] {
            let leido = ProviderCacheConfig::from_toml(&format!("{base}prompt_cache = {texto}\n"));
            assert_eq!(leido.unwrap().prompt_cache, Some(esperado), "{texto}");
        }
        let no_booleano = format!("{base}prompt_cache = \"off\"\n");
        assert!(ProviderCacheConfig::from_toml(&no_booleano).is_err());
    }

    #[test]
    fn delegation_is_optional_reads_its_three_values_and_rejects_others() {
        assert_eq!(AgentsConfig::default().delegation, None);
        let leer = |valor: &str| {
            AgentsConfig::from_toml(&format!("[agents]\ndelegation = \"{valor}\"\n"))
                .map(|agents| agents.delegation)
        };
        assert_eq!(leer("off").unwrap(), Some(DelegationMode::Off));
        assert_eq!(leer("on_request").unwrap(), Some(DelegationMode::OnRequest));
        assert_eq!(leer("proactive").unwrap(), Some(DelegationMode::Proactive));
        assert!(leer("sometimes").is_err());
    }

    #[test]
    fn only_the_prompt_modes_carry_a_rule() {
        assert_eq!(DelegationMode::Off.rule(), None);
        assert!(DelegationMode::OnRequest
            .rule()
            .is_some_and(|rule| rule.contains("explicitly asks")));
        assert!(DelegationMode::Proactive
            .rule()
            .is_some_and(|rule| rule.contains("proactively")));
    }

    #[test]
    fn depth_ceiling_defaults_to_one_reads_and_clamps() {
        assert_eq!(AgentsConfig::default().depth_ceiling(), 1);
        let leer = |valor: &str| {
            AgentsConfig::from_toml(&format!(
                "[agents]
max_depth = {valor}
"
            ))
            .unwrap()
            .depth_ceiling()
        };
        assert_eq!(leer("2"), 2);
        assert_eq!(leer("3"), 3);
        assert_eq!(leer("0"), 1);
        assert_eq!(leer("9"), 3);
    }

    fn umbrales(toml: &str) -> (u64, u64) {
        let cfg = AgentsConfig::from_toml(toml).unwrap();
        (cfg.stall_threshold_ms(), cfg.lost_threshold_ms())
    }

    #[test]
    fn lost_action_por_defecto_solo_avisa() {
        assert_eq!(AgentsConfig::default().lost_action, LostAction::Notify);
        let cfg = AgentsConfig::from_toml(
            "[agents]
lost_action = \"notify\"
",
        )
        .unwrap();
        assert_eq!(cfg.lost_action, LostAction::Notify);
        let cfg = AgentsConfig::from_toml(
            "[agents]
lost_action = \"fail\"
",
        )
        .unwrap();
        assert_eq!(cfg.lost_action, LostAction::Fail);
    }

    #[test]
    fn lost_action_desconocida_es_un_error_y_no_se_ignora() {
        assert!(AgentsConfig::from_toml(
            "[agents]
lost_action = \"cancel\"
"
        )
        .is_err());
    }

    #[test]
    fn umbrales_por_defecto() {
        let cfg = AgentsConfig::default();
        assert_eq!(cfg.stall_threshold_ms(), 120_000);
        assert_eq!(cfg.lost_threshold_ms(), 300_000);
    }

    #[test]
    fn umbrales_acotados_por_ambos_lados() {
        let (stall, lost) =
            umbrales("[agents]\nstall_threshold_secs = 1\nlost_threshold_secs = 1\n");
        assert_eq!(stall, 10_000);
        assert_eq!(lost, 40_000, "lost mínimo 30 s, pero sube a stall + margen");
        let (stall, lost) =
            umbrales("[agents]\nstall_threshold_secs = 999999\nlost_threshold_secs = 999999\n");
        assert_eq!(stall, 3_600_000);
        assert_eq!(lost, 21_600_000);
    }

    #[test]
    fn lost_siempre_supera_a_stall() {
        // Stall en su máximo con lost por defecto: lost se normaliza a stall + margen.
        let (stall, lost) = umbrales("[agents]\nstall_threshold_secs = 3600\n");
        assert_eq!(lost, stall + LOST_MARGIN_MS);
        // Lost configurado por debajo de stall: idem.
        let (stall, lost) =
            umbrales("[agents]\nstall_threshold_secs = 200\nlost_threshold_secs = 100\n");
        assert_eq!(lost, stall + LOST_MARGIN_MS);
    }

    #[test]
    fn toml_lee_las_claves_del_spec() {
        let (stall, lost) =
            umbrales("[agents]\nstall_threshold_secs = 60\nlost_threshold_secs = 200\n");
        assert_eq!((stall, lost), (60_000, 200_000));
    }

    #[test]
    fn active_subagents_defaults_reads_and_clamps() {
        assert_eq!(AgentsConfig::default().active_subagents(), 8);
        let leer = |valor: &str| {
            AgentsConfig::from_toml(&format!(
                "[agents]
max_active_subagents = {valor}
"
            ))
            .unwrap()
            .active_subagents()
        };
        assert_eq!(leer("3"), 3);
        assert_eq!(leer("0"), 1);
        assert_eq!(leer("1000"), 64);
    }

    #[test]
    fn confirm_delegation_defaults_to_doubt() {
        let sin_seccion = "model = \"gpt-4o\"\nprovider = { kind = \"openai_compat\", base_url = \"https://api.openai.com\", api_key_env = \"OPENAI_API_KEY\" }\n";
        let agents = AgentsConfig::from_toml(sin_seccion).unwrap();
        assert_eq!(agents.confirm_delegation, ConfirmDelegation::Doubt);

        let sin_clave = format!("{sin_seccion}[agents]\ntrust_project_agents = false\n");
        let agents = AgentsConfig::from_toml(&sin_clave).unwrap();
        assert_eq!(agents.confirm_delegation, ConfirmDelegation::Doubt);
    }

    #[test]
    fn confirm_delegation_reads_the_three_modes() {
        let casos = [
            ("doubt", ConfirmDelegation::Doubt),
            ("always", ConfirmDelegation::Always),
            ("never", ConfirmDelegation::Never),
        ];
        for (valor, esperado) in casos {
            let text = format!("[agents]\nconfirm_delegation = \"{valor}\"\n");
            let agents = AgentsConfig::from_toml(&text).unwrap();
            assert_eq!(agents.confirm_delegation, esperado, "valor {valor}");
        }
    }

    #[test]
    fn routing_defaults_to_off_and_reads_its_modes_and_model() {
        let agents = AgentsConfig::from_toml("").unwrap();
        assert_eq!(agents.routing, RoutingMode::Off);
        assert_eq!(agents.routing_model, None);

        for (valor, esperado) in [
            ("off", RoutingMode::Off),
            ("rule", RoutingMode::Rule),
            ("classifier", RoutingMode::Classifier),
        ] {
            let text = format!("[agents]\nrouting = \"{valor}\"\nrouting_model = \"barato\"\n");
            let agents = AgentsConfig::from_toml(&text).unwrap();
            assert_eq!(agents.routing, esperado, "valor {valor}");
            assert_eq!(agents.routing_model.as_deref(), Some("barato"));
        }
    }

    #[test]
    fn routing_rejects_unknown_values() {
        let error = AgentsConfig::from_toml("[agents]\nrouting = \"magic\"\n").unwrap_err();
        assert!(matches!(error, CoreError::Config(_)), "got {error:?}");
    }

    #[test]
    fn confirm_delegation_rejects_unknown_values() {
        let text = "[agents]\nconfirm_delegation = \"sometimes\"\n";
        let error = AgentsConfig::from_toml(text).unwrap_err();
        assert!(matches!(error, CoreError::Config(_)), "got {error:?}");
    }
}
