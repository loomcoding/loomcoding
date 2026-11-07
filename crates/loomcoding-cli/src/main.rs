use anyhow::Result;
use clap::Parser;
use loomcoding_core::agent::AgentLoop;
use loomcoding_core::catalog::{
    cache_path, fetch_raw, is_stale, read_cache, refresh, Catalog, CATALOG_URL,
};
use loomcoding_core::config::{AgentsConfig, LoomcodingConfig, ProviderKind};
use loomcoding_core::event::EventBus;
use loomcoding_core::provider::anthropic::AnthropicProvider;
use loomcoding_core::provider::gemini::GeminiProvider;
use loomcoding_core::provider::openai::OpenAiCompatProvider;
use loomcoding_core::provider::openai::OpenAiResponsesProvider;
use loomcoding_core::provider::Provider;
use loomcoding_core::registry::{
    AgentRegistry, AgentSource, BuiltInSource, FileSource, ProfileLevel, TrustStore,
};
use loomcoding_core::tools::ToolRegistry;
use loomcoding_tui::app::ProviderSettings;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

mod credentials;
mod home;
mod ipc;
mod registro_local;
mod server;
mod sondeo_local;

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"), version)]
struct Cli {
    /// Ejecuta un prompt y sale (sin TUI).
    #[arg(long, conflicts_with = "attach")]
    print: Option<String>,
    /// Reanuda una instancia local desconectada.
    #[arg(long)]
    attach: Option<String>,
    /// Arranca el daemon local (uso interno).
    #[arg(long, hide = true)]
    server: bool,
    #[arg(long)]
    yolo: bool,
}

/// Interruptor de los puntos de caché de prompts del adaptador de Anthropic. Precedencia: la variable
/// `LOOMCODING_PROMPT_CACHE` si no está vacía (`0`, `off` o `false`, sin distinguir mayúsculas, apaga; otro valor
/// enciende); si no, `[provider] prompt_cache` de `loomcoding.toml`; si no, encendido.
fn prompt_cache_enabled(value: Option<&str>, configured: Option<bool>) -> bool {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => !matches!(value.to_ascii_lowercase().as_str(), "0" | "off" | "false"),
        None => configured.unwrap_or(true),
    }
}

/// Traduce `[provider] request_timeout_secs` (ya validada por el cargador) al plazo del adaptador.
fn request_timeout_from_secs(secs: Option<u64>) -> Option<std::time::Duration> {
    secs.map(std::time::Duration::from_secs)
}

/// Envuelve el proveedor con el tope de peticiones simultáneas y el reintento ante 429 de `[limits]`.
fn limit_provider(
    provider: Arc<dyn Provider>,
    limits: &loomcoding_core::config::LimitsConfig,
) -> Arc<dyn Provider> {
    Arc::new(loomcoding_core::provider::limited::LimitedProvider::new(
        provider,
        limits.max_concurrent_requests,
    ))
}

fn build_provider(
    cfg: &LoomcodingConfig,
    project_env: &HashMap<String, String>,
    prompt_cache: Option<bool>,
    request_timeout: Option<std::time::Duration>,
) -> Result<Arc<dyn Provider>> {
    Ok(build_provider_state(cfg, project_env, None, prompt_cache, request_timeout)?.0)
}

fn build_provider_state(
    cfg: &LoomcodingConfig,
    project_env: &HashMap<String, String>,
    api_key_override: Option<&str>,
    prompt_cache: Option<bool>,
    request_timeout: Option<std::time::Duration>,
) -> Result<(Arc<dyn Provider>, bool)> {
    let nombre = cfg.provider.api_key_env.as_str();
    let key = api_key_override
        .map(str::to_string)
        .or_else(|| std::env::var(nombre).ok())
        .or_else(|| project_env.get(nombre).cloned())
        .filter(|key| !key.trim().is_empty())
        .or_else(|| {
            (!nombre.is_empty())
                .then(|| credentials::get(nombre).ok().flatten())
                .flatten()
        });
    // Un servidor de bucle local (llama-server) no pide clave: sin ella, se usa la ficticia.
    let Some(key) = key.or_else(|| registro_local::clave_de_bucle(&cfg.provider.base_url)) else {
        return Ok((Arc::new(UnconfiguredProvider), false));
    };
    Ok((
        match cfg.provider.kind {
            ProviderKind::OpenAiCompat => {
                let provider = OpenAiCompatProvider::new(cfg.provider.base_url.clone(), key);
                Arc::new(match request_timeout {
                    Some(timeout) => provider.with_request_timeout(timeout),
                    None => provider,
                })
            }
            ProviderKind::OpenAiResponses => Arc::new(OpenAiResponsesProvider::new(
                cfg.provider.base_url.clone(),
                key,
            )),
            ProviderKind::Anthropic => Arc::new(
                AnthropicProvider::new(cfg.provider.base_url.clone(), key).with_prompt_cache(
                    prompt_cache_enabled(
                        std::env::var("LOOMCODING_PROMPT_CACHE").ok().as_deref(),
                        prompt_cache,
                    ),
                ),
            ),
            ProviderKind::Gemini => {
                Arc::new(GeminiProvider::new(cfg.provider.base_url.clone(), key))
            }
        },
        true,
    ))
}

struct UnconfiguredProvider;

#[async_trait::async_trait]
impl Provider for UnconfiguredProvider {
    fn capabilities(&self) -> loomcoding_core::provider::Capabilities {
        loomcoding_core::provider::Capabilities {
            tool_use: false,
            reasoning: false,
            streaming: false,
        }
    }

    async fn stream(
        &self,
        _req: loomcoding_core::provider::ProviderRequest,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> loomcoding_core::error::Result<loomcoding_core::provider::ProviderStream> {
        Err(loomcoding_core::error::CoreError::Provider(
            "No API key configured. Open /settings and save your provider key.".into(),
        ))
    }
}

fn config_from_provider_settings(
    settings: ProviderSettings,
    current: &LoomcodingConfig,
) -> Result<LoomcodingConfig, String> {
    let mut candidate = current.clone();
    candidate.model = settings.model.trim().to_string();
    candidate.provider.kind = settings.kind;
    candidate.provider.base_url = settings.base_url.trim().to_string();
    candidate.provider.api_key_env = settings.api_key_env.trim().to_string();
    candidate.effort = settings.effort;

    if candidate.model.is_empty() {
        return Err("Model cannot be empty".into());
    }
    loomcoding_core::config::validate_base_url(&candidate.provider.base_url)?;
    // Un servidor de bucle local (llama-server) no pide clave: la variable puede quedar vacía.
    // Para cualquier otra URL es obligatoria y debe ser un nombre de variable válido.
    if candidate.provider.api_key_env.is_empty()
        && loomcoding_core::config::is_loopback_url(&candidate.provider.base_url)
    {
        return Ok(candidate);
    }
    let mut chars = candidate.provider.api_key_env.chars();
    if !chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        || !chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        return Err("API key variable must be a valid environment variable name".into());
    }
    Ok(candidate)
}

fn apply_provider_settings(
    settings: ProviderSettings,
    api_key: Option<&str>,
    current: &LoomcodingConfig,
    config_path: &std::path::Path,
    agent: &AgentLoop,
    project_env: &HashMap<String, String>,
) -> Result<(LoomcodingConfig, bool), String> {
    let candidate = config_from_provider_settings(settings, current)?;
    let api_key = api_key.map(str::trim).filter(|key| !key.is_empty());
    if let Some(key) = api_key {
        if key.len() > 4096 || key.chars().any(char::is_control) {
            return Err(
                "API key must be at most 4096 bytes and contain no control characters".into(),
            );
        }
        credentials::set(&candidate.provider.api_key_env, key)
            .map_err(|error| format!("could not save API key securely: {error}"))?;
    }
    // Un fichero ilegible no impide guardar los ajustes: se usa el valor por defecto.
    let prompt_cache = loomcoding_core::config::ProviderCacheConfig::load(config_path)
        .ok()
        .and_then(|config| config.prompt_cache);
    let request_timeout = request_timeout_from_secs(
        loomcoding_core::config::ProviderLimitsConfig::load(config_path)
            .ok()
            .and_then(|limits| limits.request_timeout_secs),
    );
    let (provider, configured) = build_provider_state(
        &candidate,
        project_env,
        api_key,
        prompt_cache,
        request_timeout,
    )
    .map_err(|error| error.to_string())?;
    // Un fichero ilegible no impide guardar los ajustes: se usan los topes por defecto.
    let provider = limit_provider(
        provider,
        &loomcoding_core::config::LimitsConfig::load(config_path).unwrap_or_default(),
    );
    candidate
        .save(config_path)
        .map_err(|error| error.to_string())?;
    agent.update_provider(provider, candidate.clone());
    Ok((candidate, configured))
}

/// Registro de herramientas del proyecto y el estado de ODD que comparte con su política de cierre.
fn build_project_tools(
    project_env: HashMap<String, String>,
) -> (ToolRegistry, loomcoding_workflows::OddState) {
    let mut tools = loomcoding_core::tools::project_registry(project_env);
    let odd = loomcoding_workflows::register(&mut tools);
    (tools, odd)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.server {
        server::run_server().await
    } else {
        server::run_client(cli).await
    }
}
/// Ventana declarada: (URL base, modelo, tokens).
type DeclaredWindow = Option<(String, String, u64)>;

/// Construye la declaración a partir de `[provider] context_window`; sin clave, no hay declaración.
fn declared_window(
    limits: &loomcoding_core::config::ProviderLimitsConfig,
    base_url: &str,
    model: &str,
) -> DeclaredWindow {
    limits
        .context_window
        .map(|window| (base_url.to_owned(), model.to_owned(), window))
}

fn with_declared_window(catalog: Catalog, declared: &DeclaredWindow) -> Catalog {
    match declared {
        Some((base_url, model, window)) => catalog.with_window_override(base_url, model, *window),
        None => catalog,
    }
}

/// Entrega a la interfaz el catálogo de models.dev: primero la caché (si existe) y, si falta
/// o ha caducado, uno descargado. Cada catálogo que llega pasa por el publicador, que añade los
/// proveedores locales del último sondeo y lo copia en el compartido del agente. Los fallos se
/// ignoran en silencio: sin catálogo, `/settings` funciona como antes. Corre en su propia tarea,
/// así que nunca retrasa el arranque.
///
/// La ventana declarada en `loomcoding.toml` se publica antes que nada (así rige sin caché ni red)
/// y el publicador la aplica a cada catálogo que llega.
async fn load_catalog(publicador: Arc<sondeo_local::Publicador>) {
    if publicador.tiene_ventana_declarada() {
        publicador.publicar_base(Catalog::default());
    }
    let Some(path) = cache_path() else {
        return;
    };
    // La lectura de la caché es E/S síncrona: se aparta del hilo del ejecutor.
    let cached = tokio::task::spawn_blocking({
        let path = path.clone();
        move || read_cache(&path)
    })
    .await
    .ok()
    .flatten();

    let needs_refresh = match &cached {
        Some((_, modified)) => is_stale(*modified, SystemTime::now()),
        None => true,
    };
    if let Some((catalog, _)) = cached {
        publicador.publicar_base(catalog);
    }
    if needs_refresh {
        if let Ok(catalog) = refresh(&path, || fetch_raw(CATALOG_URL)).await {
            publicador.publicar_base(catalog);
        }
    }
}

/// Lee el `.env` del proyecto sin incorporarlo al entorno compartido del servidor.
fn load_dotenv(dir: &std::path::Path) -> HashMap<String, String> {
    dotenvy::from_path_iter(dir.join(".env"))
        .map(|entries| entries.filter_map(|entry| entry.ok()).collect())
        .unwrap_or_default()
}

/// Entorno mínimo para un shell de proyecto; las variables de otros proyectos y las claves del
/// entorno padre no se heredan.
fn shell_env(project_env: &HashMap<String, String>) -> HashMap<String, String> {
    const SAFE: &[&str] = &[
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "SYSTEMDRIVE",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "HOME",
        "USER",
        "USERNAME",
        "LOGNAME",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "CARGO_HOME",
        "RUSTUP_HOME",
    ];
    let mut env: HashMap<_, _> = std::env::vars()
        .filter(|(name, _)| SAFE.iter().any(|safe| name.eq_ignore_ascii_case(safe)))
        .collect();
    for (name, value) in project_env {
        env.entry(name.clone()).or_insert_with(|| value.clone());
    }
    env
}

/// Registro de agentes y directorios que vigila, tal como quedan al arrancar.
struct ArranqueAgentes {
    registro: AgentRegistry,
    directorios: Vec<PathBuf>,
}

/// Construye el registro con `default` integrado, la fuente global y la de proyecto, de menor
/// a mayor precedencia. Sin `home` no hay fuente global ni almacén de aprobaciones: los agentes
/// de proyecto quedan pendientes y nunca se guarda una aprobación dentro del repositorio.
#[cfg(test)]
fn arrancar_agentes(home: Option<&Path>, raiz: &Path, agents: &AgentsConfig) -> ArranqueAgentes {
    arrancar_agentes_con_runtime(home, raiz, agents, None)
}

fn arrancar_agentes_con_runtime(
    home: Option<&Path>,
    raiz: &Path,
    agents: &AgentsConfig,
    runtime_peers: Option<PathBuf>,
) -> ArranqueAgentes {
    let global = match &agents.global_dir {
        Some(ruta) => resolver_ruta(ruta, home, raiz),
        None => home.map(|h| h.join(".agents").join("agents")),
    };
    let proyecto = match &agents.project_dir {
        Some(ruta) => resolver_ruta(ruta, home, raiz),
        None => Some(raiz.join(".agents").join("agents")),
    };
    let mut fuentes: Vec<Box<dyn AgentSource>> = vec![Box::new(BuiltInSource)];
    let mut directorios = Vec::new();
    if let Some(dir) = global {
        directorios.push(dir.clone());
        fuentes.push(Box::new(FileSource::new(dir, ProfileLevel::Global)));
    }
    if let Some(dir) = runtime_peers {
        directorios.push(dir.clone());
        fuentes.push(Box::new(FileSource::new(dir, ProfileLevel::Global)));
    }
    // Catálogos externos: contenido de terceros, entre el global y el del proyecto.
    for raiz_catalogo in agents.catalog_roots_effective() {
        let Some(base) = resolver_ruta(&raiz_catalogo, home, raiz) else {
            continue;
        };
        let dir = base.join(".agents").join("agents");
        directorios.push(dir.clone());
        fuentes.push(Box::new(FileSource::new(dir, ProfileLevel::Catalog)));
    }
    if let Some(dir) = proyecto {
        directorios.push(dir.clone());
        fuentes.push(Box::new(FileSource::new(dir, ProfileLevel::Project)));
    }
    let mut registro = AgentRegistry::new(fuentes);
    if let Some(home) = home {
        match TrustStore::open(home.join(".loomcoding")) {
            Ok(almacen) => {
                registro =
                    registro.with_trust(almacen, raiz.to_path_buf(), agents.trust_project_agents);
            }
            Err(error) => {
                eprintln!("loomcoding: almacén de aprobaciones de agentes no disponible: {error}");
            }
        }
    }
    ArranqueAgentes {
        registro,
        directorios,
    }
}

fn runtime_peer_dir(data_dir: &Path, generation: &str, project_root: &Path) -> PathBuf {
    // shortcut: stale generations are left in user data; prune them if repeated runs accumulate noticeably.
    let project_hash = loomcoding_core::registry::content_hash(
        &std::fs::canonicalize(project_root)
            .unwrap_or_else(|_| project_root.to_path_buf())
            .to_string_lossy(),
    );
    data_dir
        .join("runtime")
        .join("agents")
        .join(generation)
        .join(&project_hash[..16])
}

fn resolve_credential(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| credentials::get(name).ok().flatten())
        .filter(|value| !value.trim().is_empty())
}

/// Pone en marcha el consumo de agentes remotos si `[a2a] role` lo pide (A8a). Con el rol apagado
/// no se crea cliente alguno ni se hace petición. Las tarjetas se descargan en segundo plano para
/// no retrasar el arranque; un remoto que no responde queda como problema de carga de ese agente
/// y nunca se aprueba solo.
fn arrancar_remotos(
    a2a: &loomcoding_a2a::config::A2aConfig,
    registro: &Arc<AgentRegistry>,
    bus: &EventBus,
) -> Result<Option<Arc<dyn loomcoding_core::remote_task::RemoteClientFactory>>> {
    let Some(cliente) = loomcoding_a2a::remote::http_client(a2a)? else {
        return Ok(None);
    };
    let Some(fabrica) =
        loomcoding_a2a::client::A2aClientFactory::from_config(a2a)?.map(|fabrica| {
            fabrica
                .with_event_bus(bus.clone())
                .with_env(resolve_credential)
        })
    else {
        return Ok(None);
    };
    let (registro, bus, config) = (registro.clone(), bus.clone(), a2a.clone());
    tokio::spawn(async move {
        // El informe se descarta: el registro ya publica el cambio y deja cada fallo como
        // problema de carga de su agente, que es donde la persona usuaria lo ve.
        let _ = loomcoding_a2a::sync::refresh_remote_cards_with_env(
            &registro,
            &bus,
            &cliente,
            &config,
            &resolve_credential,
        )
        .await;
        // Después, cada `card_refresh_secs` (una hora por defecto; `0` lo desactiva): una tarjeta
        // que cambia vuelve a pedir aprobación y un fallo de red no estropea lo que funcionaba.
        if let Some(intervalo) = config.card_refresh_interval() {
            loomcoding_a2a::sync::refresh_loop_with_env(
                registro,
                bus,
                cliente,
                config,
                intervalo,
                resolve_credential,
            )
            .await;
        }
    });
    Ok(Some(Arc::new(fabrica)))
}

/// Atiende `/refresh-agents`: cada petición vuelve a descargar las tarjetas de los remotos y publica
/// cuántas salieron bien. Devuelve el emisor de peticiones, o `None` si el consumo de remotos está
/// apagado (entonces la orden dice que no está disponible).
fn arrancar_refresco_manual(
    a2a: &loomcoding_a2a::config::A2aConfig,
    registro: &Arc<AgentRegistry>,
    bus: &EventBus,
) -> Result<Option<tokio::sync::mpsc::UnboundedSender<()>>> {
    let Some(cliente) = loomcoding_a2a::remote::http_client(a2a)? else {
        return Ok(None);
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let (registro, bus, config) = (registro.clone(), bus.clone(), a2a.clone());
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Varias pulsaciones seguidas valen por una.
            while rx.try_recv().is_ok() {}
            registro.reload_and_publish(&bus);
            let informe = loomcoding_a2a::sync::refresh_remote_cards_with_env(
                &registro,
                &bus,
                &cliente,
                &config,
                &resolve_credential,
            )
            .await;
            bus.publish(loomcoding_core::event::LiveEvent::AgentCardsRefreshed {
                ok: informe.ok.len(),
                failed: informe.failed,
            });
        }
    });
    Ok(Some(tx))
}

/// Enrutador de agentes que pide `[agents] routing` (A12b). Apagado por defecto. El clasificador
/// aún no existe: `classifier` usa solo la regla determinista, que ya es su primer nivel.
fn enrutador_de(
    agents: &AgentsConfig,
    provider: Arc<dyn Provider>,
    modelo_por_defecto: &str,
) -> Option<Arc<dyn loomcoding_core::registry::router::AgentRouter>> {
    use loomcoding_core::config::RoutingMode;
    use loomcoding_core::registry::router::{ClassifierRouter, RuleRouter};
    match agents.routing {
        RoutingMode::Off => None,
        RoutingMode::Rule => Some(Arc::new(RuleRouter)),
        // El clasificador usa `routing_model` (barato) y, si no se indica, el modelo de la sesión.
        RoutingMode::Classifier => {
            let modelo = agents
                .routing_model
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or(modelo_por_defecto);
            Some(Arc::new(ClassifierRouter::new(provider, modelo)))
        }
    }
}

/// Resuelve una ruta de `[agents]`: un `~/` inicial va al directorio personal y una ruta
/// relativa cuelga de la raíz del proyecto. `None` si depende de un home que no existe.
fn resolver_ruta(ruta: &Path, home: Option<&Path>, raiz: &Path) -> Option<PathBuf> {
    let texto = ruta.to_string_lossy();
    if let Some(resto) = texto
        .strip_prefix("~/")
        .or_else(|| texto.strip_prefix("~\\"))
    {
        return home.map(|h| h.join(resto));
    }
    if ruta.is_absolute() {
        Some(ruta.to_path_buf())
    } else {
        Some(raiz.join(ruta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loomcoding_core::registry::Approval;

    fn test_config() -> LoomcodingConfig {
        LoomcodingConfig {
            provider: loomcoding_core::config::ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "https://old.test/v1".into(),
                api_key_env: "OLD_KEY".into(),
            },
            model: "old-model".into(),
            max_steps: 8,
            effort: None,
        }
    }

    #[test]
    fn loads_env_from_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "LOOMCODING_TEST_ENV_VAR=hello\n").unwrap();
        let env = load_dotenv(dir.path());
        assert_eq!(
            env.get("LOOMCODING_TEST_ENV_VAR").map(String::as_str),
            Some("hello")
        );
        assert!(shell_env(&env).contains_key("LOOMCODING_TEST_ENV_VAR"));
    }

    #[test]
    fn project_env_files_stay_separate_and_do_not_mutate_the_process() {
        let key = format!("LOOMCODING_ISOLATION_TEST_{}", std::process::id());
        assert!(std::env::var(&key).is_err());
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(first.path().join(".env"), format!("{key}=first\n")).unwrap();
        std::fs::write(second.path().join(".env"), format!("{key}=second\n")).unwrap();

        let first_env = load_dotenv(first.path());
        let second_env = load_dotenv(second.path());

        assert_eq!(first_env.get(&key).map(String::as_str), Some("first"));
        assert_eq!(second_env.get(&key).map(String::as_str), Some("second"));
        assert!(std::env::var(&key).is_err());
    }

    #[test]
    fn validates_provider_settings_before_persisting() {
        let current = test_config();
        let settings = ProviderSettings {
            kind: ProviderKind::Anthropic,
            model: "  claude-test  ".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            effort: None,
        };
        let updated = config_from_provider_settings(settings.clone(), &current).unwrap();
        assert_eq!(updated.model, "claude-test");
        assert_eq!(updated.provider.kind, ProviderKind::Anthropic);

        let mut invalid = settings;
        invalid.base_url = "https://".into();
        assert!(config_from_provider_settings(invalid, &current).is_err());
    }

    #[test]
    fn la_variable_de_clave_puede_quedar_vacia_solo_en_bucle_local() {
        let current = test_config();
        let local = ProviderSettings {
            kind: ProviderKind::OpenAiCompat,
            model: "Ornith-1.5-9B".into(),
            base_url: "http://127.0.0.1:8081/v1".into(),
            api_key_env: String::new(),
            effort: None,
        };
        let updated = config_from_provider_settings(local.clone(), &current)
            .expect("un servidor de bucle local no pide clave");
        assert_eq!(updated.provider.api_key_env, "");

        let mut sin_bucle = local;
        sin_bucle.base_url = "https://api.example.test/v1".into();
        assert_eq!(
            config_from_provider_settings(sin_bucle, &current).unwrap_err(),
            "API key variable must be a valid environment variable name"
        );
    }

    #[test]
    fn settings_effort_reaches_the_config() {
        let current = test_config();
        let settings = ProviderSettings {
            kind: ProviderKind::Anthropic,
            model: "claude-think".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            effort: Some("high".into()),
        };
        let updated = config_from_provider_settings(settings, &current).unwrap();
        assert_eq!(updated.effort.as_deref(), Some("high"));
    }

    fn perfil_de_prueba(nombre: &str) -> String {
        format!("---\nname: {nombre}\ndescription: Agente de prueba\n---\n\nCuerpo.\n")
    }

    #[test]
    fn startup_without_home_directory_does_not_panic() {
        let proyecto = tempfile::tempdir().unwrap();
        let agentes_dir = proyecto.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agentes_dir).unwrap();
        std::fs::write(agentes_dir.join("revisor.md"), perfil_de_prueba("revisor")).unwrap();

        let arranque = arrancar_agentes(None, proyecto.path(), &AgentsConfig::default());

        // Sin home: ni fuente global ni vigilancia global, y el proyecto se sigue leyendo.
        assert_eq!(arranque.directorios, vec![agentes_dir]);
        let instantanea = arranque.registro.snapshot();
        assert!(instantanea.get("default").is_some());
        assert!(instantanea.get("revisor").is_some());
        // Sin home no hay almacén de aprobaciones: el agente de proyecto queda pendiente.
        assert_eq!(instantanea.approval("revisor"), Some(Approval::Pending));
    }

    #[test]
    fn startup_with_home_reads_global_and_project_agents() {
        let home = tempfile::tempdir().unwrap();
        let proyecto = tempfile::tempdir().unwrap();
        let global_dir = home.path().join(".agents").join("agents");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(global_dir.join("global.md"), perfil_de_prueba("global")).unwrap();
        let proyecto_dir = proyecto.path().join(".agents").join("agents");
        std::fs::create_dir_all(&proyecto_dir).unwrap();
        std::fs::write(proyecto_dir.join("local.md"), perfil_de_prueba("local")).unwrap();

        let arranque =
            arrancar_agentes(Some(home.path()), proyecto.path(), &AgentsConfig::default());

        assert_eq!(arranque.directorios, vec![global_dir, proyecto_dir]);
        let instantanea = arranque.registro.snapshot();
        assert_eq!(instantanea.approval("global"), Some(Approval::NotRequired));
        assert_eq!(instantanea.approval("local"), Some(Approval::Pending));
        // El almacén de aprobaciones vive en el directorio de datos del usuario, nunca en el proyecto.
        assert!(home.path().join(".loomcoding").is_dir());
    }

    #[test]
    fn agents_config_overrides_the_default_directories() {
        let home = tempfile::tempdir().unwrap();
        let proyecto = tempfile::tempdir().unwrap();
        let agents = AgentsConfig {
            global_dir: Some(PathBuf::from("~/mis-agentes")),
            project_dir: Some(PathBuf::from("config/agentes")),
            trust_project_agents: true,
            ..AgentsConfig::default()
        };

        let arranque = arrancar_agentes(Some(home.path()), proyecto.path(), &agents);

        assert_eq!(
            arranque.directorios,
            vec![
                home.path().join("mis-agentes"),
                proyecto.path().join("config").join("agentes"),
            ]
        );
    }

    /// Registro con un único remoto `lejano` apuntando a `endpoint`, y el directorio que lo guarda.
    fn registro_con_remoto(endpoint: &str) -> (Arc<AgentRegistry>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("lejano.md"),
            format!(
                "---
name: lejano
description: Alta manual
transport: a2a-remote
a2a:
  endpoint: {endpoint}
---
"
            ),
        )
        .unwrap();
        let registro = Arc::new(AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FileSource::new(
                dir.path().to_path_buf(),
                ProfileLevel::Global,
            )),
        ]));
        (registro, dir)
    }

    #[test]
    fn a_declared_window_is_built_only_from_the_toml_key() {
        use loomcoding_core::config::ProviderLimitsConfig;
        let sin = ProviderLimitsConfig::default();
        assert_eq!(declared_window(&sin, "http://x/v1", "m"), None);
        let con = ProviderLimitsConfig {
            context_window: Some(35_584),
            request_timeout_secs: None,
            efforts: Vec::new(),
        };
        assert_eq!(
            declared_window(&con, "http://x/v1", "m"),
            Some(("http://x/v1".into(), "m".into(), 35_584))
        );
    }

    #[test]
    fn a_declaration_is_applied_to_any_catalog_and_none_leaves_it_alone() {
        let declared = Some(("http://x/v1".to_string(), "m".to_string(), 35_584));
        let catalog = with_declared_window(Catalog::default(), &declared);
        assert_eq!(catalog.context_window("http://x/v1", "m"), Some(35_584));
        let sin = with_declared_window(Catalog::default(), &None);
        assert_eq!(sin.context_window("http://x/v1", "m"), None);
    }

    #[test]
    fn a_limited_provider_keeps_the_capabilities_of_the_inner_one() {
        let limited = limit_provider(
            Arc::new(UnconfiguredProvider),
            &loomcoding_core::config::LimitsConfig::default(),
        );
        assert!(!limited.capabilities().tool_use);
    }

    #[test]
    fn request_timeout_secs_become_a_duration_only_when_configured() {
        assert_eq!(request_timeout_from_secs(None), None);
        assert_eq!(
            request_timeout_from_secs(Some(600)),
            Some(std::time::Duration::from_secs(600))
        );
    }

    #[test]
    fn a_configured_timeout_reaches_the_openai_compat_adapter() {
        let cfg = LoomcodingConfig {
            provider: loomcoding_core::config::ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://127.0.0.1:1/v1".into(),
                api_key_env: "LOOMCODING_TEST_KEY_PLAZO".into(),
            },
            model: "m".into(),
            max_steps: 1,
            effort: None,
        };
        let (_, configured) = build_provider_state(
            &cfg,
            &HashMap::new(),
            Some("sk-test"),
            None,
            request_timeout_from_secs(Some(300)),
        )
        .unwrap();
        assert!(configured);
    }

    #[test]
    fn prompt_cache_is_on_unless_explicitly_switched_off() {
        assert!(prompt_cache_enabled(None, None));
        assert!(prompt_cache_enabled(Some("1"), None));
        assert!(prompt_cache_enabled(Some("on"), None));
        assert!(prompt_cache_enabled(Some(""), None));
        for apagado in ["0", "off", "OFF", "false", " False "] {
            assert!(!prompt_cache_enabled(Some(apagado), None), "{apagado}");
        }
    }

    #[test]
    fn prompt_cache_takes_the_toml_key_unless_the_environment_decides() {
        assert!(!prompt_cache_enabled(None, Some(false)));
        assert!(prompt_cache_enabled(None, Some(true)));
        // Un entorno vacío no decide: vale la clave.
        assert!(!prompt_cache_enabled(Some(""), Some(false)));
        // La variable de entorno manda sobre el fichero, en ambos sentidos.
        assert!(!prompt_cache_enabled(Some("off"), Some(true)));
        assert!(prompt_cache_enabled(Some("1"), Some(false)));
    }

    #[test]
    fn routing_is_off_by_default_and_on_with_rule_or_classifier() {
        use loomcoding_core::config::RoutingMode;
        // Un proveedor que no se llega a usar: construirlo no abre ninguna conexión.
        let proveedor: Arc<dyn Provider> = Arc::new(AnthropicProvider::new(
            "http://127.0.0.1:9".to_string(),
            "k".to_string(),
        ));
        assert!(enrutador_de(&AgentsConfig::default(), proveedor.clone(), "m").is_none());
        for modo in [RoutingMode::Rule, RoutingMode::Classifier] {
            let cfg = AgentsConfig {
                routing: modo,
                ..AgentsConfig::default()
            };
            assert!(
                enrutador_de(&cfg, proveedor.clone(), "m").is_some(),
                "{modo:?}"
            );
        }
    }

    #[tokio::test]
    async fn with_the_a2a_role_off_no_remote_machinery_is_started() {
        let servidor = wiremock::MockServer::start().await;
        let (registro, _dir) = registro_con_remoto(&servidor.uri());

        let fabrica = arrancar_remotos(
            &loomcoding_a2a::config::A2aConfig::default(),
            &registro,
            &EventBus::new(16),
        )
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(fabrica.is_none());
        assert!(servidor.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn with_the_a2a_role_consume_the_remote_card_is_fetched_and_never_approved() {
        use wiremock::matchers::{method, path};
        let servidor = wiremock::MockServer::start().await;
        let mut tarjeta: serde_json::Value = serde_json::from_str(include_str!(
            "../../loomcoding-a2a/tests/fixtures/agent-card.sample.json"
        ))
        .unwrap();
        for interfaz in tarjeta["supportedInterfaces"].as_array_mut().unwrap() {
            interfaz["url"] = serde_json::json!(format!("{}/a2a/v1", servidor.uri()));
        }
        tarjeta
            .as_object_mut()
            .unwrap()
            .remove("securityRequirements");
        wiremock::Mock::given(method("GET"))
            .and(path("/.well-known/agent-card.json"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(tarjeta))
            .mount(&servidor)
            .await;
        let (registro, _dir) = registro_con_remoto(&servidor.uri());
        let a2a = loomcoding_a2a::config::A2aConfig {
            role: loomcoding_a2a::config::A2aRole::Consume,
            request_timeout_secs: 2,
            ..Default::default()
        };

        let fabrica = arrancar_remotos(&a2a, &registro, &EventBus::new(16)).unwrap();

        assert!(fabrica.is_some());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let descargada = registro.snapshot().entries().into_iter().any(|e| {
                    matches!(&e.execution, loomcoding_core::registry::Execution::Remote(spec)
                        if e.name == "lejano" && spec.fetched.is_some())
                });
                if descargada {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("la tarjeta debería descargarse y anotarse en el registro");
        assert_eq!(
            registro.snapshot().approval("lejano"),
            Some(Approval::Pending),
            "descargar la tarjeta no aprueba nada"
        );
    }
}
