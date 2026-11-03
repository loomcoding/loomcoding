use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub mod context;

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
    pub api_key_env: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeLoomConfig {
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
/// variable de entorno `CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE`.
pub fn max_tool_calls_per_response() -> usize {
    limite_por_entorno(
        "CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE",
        DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE,
    )
}

/// Tope de llamadas a herramienta acumuladas en un turno completo (varias respuestas).
///
/// Se puede subir sin recompilar con `CODELOOM_MAX_TOOL_CALLS_PER_TURN`.
pub fn max_tool_calls_per_turn() -> usize {
    limite_por_entorno(
        "CODELOOM_MAX_TOOL_CALLS_PER_TURN",
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
/// cumplimiento depende del modelo. Concepto propio de CodeLoom, no de A2A.
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

/// Sección opcional `[agents]` de `codeloom.toml`. Sin ella, todo son valores por defecto.
/// Vive aparte de `CodeLoomConfig` para que su ausencia no afecte a la configuración principal.
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

/// Sección opcional `[ui]` de `codeloom.toml`. Sin ella, todo son valores por defecto.
/// Vive aparte de `CodeLoomConfig` para no añadir un campo que se construye en muchos sitios.
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

pub fn validate_base_url(input: &str) -> std::result::Result<(), String> {
    let url = reqwest::Url::parse(input).map_err(|_| "Base URL is invalid".to_string())?;
    let local_http = url.scheme() == "http"
        && url.host_str().is_some_and(|host| {
            host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        });
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

impl CodeLoomConfig {
    /// Creates a usable project configuration on first launch without credentials.
    pub fn load_or_create(path: &Path) -> Result<CodeLoomConfig> {
        const DEFAULT: &str = "# CodeLoom project settings. Add the API key in /settings.\nmodel = \"nvidia/nemotron-3.5-super-vl:free\"\nmax_steps = 8\n\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://openrouter.ai/api/v1\"\napi_key_env = \"OPENROUTER_API_KEY\"\n";
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(mut file) => file
                .write_all(DEFAULT.as_bytes())
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

    pub fn load(path: &Path) -> Result<CodeLoomConfig> {
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
        let text = document.to_string();
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
        let (temp_path, mut temp) = temp_file
            .ok_or_else(|| CoreError::Config("cannot create temporary config file".into()))?;
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
        let cfg = CodeLoomConfig::load(f.path()).unwrap();
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
        let cfg = CodeLoomConfig::load(f.path()).unwrap();
        assert_eq!(cfg.max_steps, 30);
    }

    #[test]
    fn tool_call_limits_have_sane_defaults() {
        assert_eq!(DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE, 128);
        assert_eq!(DEFAULT_MAX_TOOL_CALLS_PER_TURN, 256);
    }

    #[test]
    fn tool_call_limits_can_be_raised_by_environment() {
        std::env::set_var("CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE", "500");
        std::env::remove_var("CODELOOM_MAX_TOOL_CALLS_PER_TURN");
        assert_eq!(max_tool_calls_per_response(), 500);
        assert_eq!(max_tool_calls_per_turn(), DEFAULT_MAX_TOOL_CALLS_PER_TURN);
        std::env::remove_var("CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE");
    }

    #[test]
    fn tool_call_limits_ignore_invalid_values() {
        std::env::set_var("CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE", "no-es-un-numero");
        assert_eq!(
            max_tool_calls_per_response(),
            DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE
        );
        std::env::set_var("CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE", "0");
        assert_eq!(
            max_tool_calls_per_response(),
            DEFAULT_MAX_TOOL_CALLS_PER_RESPONSE
        );
        std::env::remove_var("CODELOOM_MAX_TOOL_CALLS_PER_RESPONSE");
    }

    #[test]
    fn saves_and_reloads_provider_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codeloom.toml");
        std::fs::write(
            &path,
            "# preserved\nmodel = \"old\"\nmax_steps = 7\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://old.test\"\napi_key_env = \"OLD_KEY\"\n",
        )
        .unwrap();
        let config = CodeLoomConfig {
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
        let loaded = CodeLoomConfig::load(&path).unwrap();
        assert_eq!(loaded.model, "claude-test");
        assert_eq!(loaded.provider.kind, ProviderKind::Anthropic);
        assert_eq!(loaded.provider.api_key_env, "ANTHROPIC_API_KEY");
        assert_eq!(loaded.max_steps, 7);
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("# preserved"));
    }

    #[test]
    fn config_without_effort_still_loads() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, r#"
            model = "gpt-4o"
            provider = {{ kind = "openai_compat", base_url = "https://api.openai.com", api_key_env = "OPENAI_API_KEY" }}
        "#).unwrap();
        let cfg = CodeLoomConfig::load(f.path()).unwrap();
        assert_eq!(cfg.effort, None, "una config antigua equivale a «default»");
    }

    #[test]
    fn config_roundtrips_effort() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codeloom.toml");
        std::fs::write(
            &path,
            "# preserved\nmodel = \"old\"\nmax_steps = 7\n[provider]\nkind = \"openai_compat\"\nbase_url = \"https://old.test\"\napi_key_env = \"OLD_KEY\"\n",
        )
        .unwrap();
        let mut config = CodeLoomConfig {
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
        let loaded = CodeLoomConfig::load(&path).unwrap();
        assert_eq!(loaded.effort.as_deref(), Some("high"));
        assert_eq!(loaded.provider.kind, ProviderKind::Anthropic);

        // Con `None` la clave desaparece del fichero y al releerlo no hay esfuerzo.
        config.effort = None;
        config.save(&path).unwrap();
        let loaded = CodeLoomConfig::load(&path).unwrap();
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
        let cfg: CodeLoomConfig = toml::from_str(text).unwrap();
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
