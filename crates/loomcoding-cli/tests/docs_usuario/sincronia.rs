//! Pruebas de sincronía entre la referencia de usuario (`docs/user/{en,es}/reference/`) y el código.
//!
//! Tres comprobaciones:
//!
//! - Cada entrada de `COMMANDS` (`loomcoding_tui::commands`) aparece en `commands.md`, en inglés y en español.
//! - `commands.md` no cita ningún `/comando` que ya no exista en `COMMANDS`.
//! - Cada campo de la lista `CAMPOS_DE_CONFIGURACION` existe en el código fuente y aparece en
//!   `configuration.md`, en inglés y en español.
//!
//! Limitación, documentada a propósito: la lista `CAMPOS_DE_CONFIGURACION` es explícita. Un campo nuevo
//! en las estructuras de configuración no hace fallar la prueba mientras nadie lo añada a la lista. La
//! comprobación contra el código solo verifica que cada entrada de la lista sea un campo declarado en
//! los ficheros indicados; no recorre las estructuras. Para cerrar el hueco, la lista debería derivarse
//! de las propias estructuras (por ejemplo, con una macro de derivación que emita los pares
//! `(sección, campo)` a partir de los `#[derive(Deserialize)]`, o con `schemars` y su esquema JSON).

use loomcoding_tui::commands::COMMANDS;
use std::fs;
use std::path::{Path, PathBuf};

/// Raíz del repositorio, subiendo desde `crates/loomcoding-cli`.
fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("la raíz del repositorio existe")
        .to_path_buf()
}

/// Lee un fichero del repositorio. Si falta, la prueba falla nombrando la ruta.
fn lee(relativa: &str) -> String {
    let ruta = raiz_repo().join(relativa);
    fs::read_to_string(&ruta)
        .unwrap_or_else(|error| panic!("no se puede leer {}: {error}", ruta.display()))
}

/// Ficheros de la referencia, en los dos idiomas: `(ruta relativa a la raíz, texto)`.
fn referencias() -> Vec<(&'static str, String)> {
    [
        "docs/user/en/reference/commands.md",
        "docs/user/es/reference/commands.md",
    ]
    .into_iter()
    .map(|ruta| (ruta, lee(ruta)))
    .collect()
}

/// Nombres de todos los comandos de `COMMANDS`, con su barra (`/help`).
fn nombres_de_comandos() -> Vec<&'static str> {
    COMMANDS.iter().map(|(_, nombre, _)| *nombre).collect()
}

/// Dice si `nombre` aparece en `texto` como palabra completa: `/agent` no cuenta dentro de `/agents`.
fn menciona(texto: &str, nombre: &str) -> bool {
    texto.match_indices(nombre).any(|(i, _)| {
        let siguiente = texto[i + nombre.len()..].chars().next();
        !siguiente.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// Comandos `/x` citados entre comillas invertidas en la prosa. Se ignoran los bloques de código con
/// valla, cuyas líneas no son prosa.
fn comandos_citados(texto: &str) -> Vec<String> {
    let mut en_codigo = false;
    let mut citados = Vec::new();
    for linea in texto.lines() {
        if linea.trim_start().starts_with("```") {
            en_codigo = !en_codigo;
            continue;
        }
        if en_codigo {
            continue;
        }
        for tramo in linea.split('`').skip(1).step_by(2) {
            let Some(resto) = tramo.strip_prefix('/') else {
                continue;
            };
            let nombre: String = resto
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            if !nombre.is_empty() {
                citados.push(format!("/{nombre}"));
            }
        }
    }
    citados
}

/// Campos de configuración que la referencia debe documentar, como `(sección, campo)`. La sección es
/// el texto que el título `##` de `configuration.md` debe llevar entre comillas invertidas, por ejemplo
/// `provider` para `` `[provider]` ``, o `providers.toml` para `` `providers.toml` ``. Vacía, el campo
/// está en el nivel superior de `loomcoding.toml`.
///
/// Mantener esta lista a mano es la limitación descrita al principio del módulo.
const CAMPOS_DE_CONFIGURACION: &[(&str, &str)] = &[
    // Nivel superior de loomcoding.toml (LoomcodingConfig).
    ("", "model"),
    ("", "max_steps"),
    ("", "effort"),
    // [provider] (ProviderConfig, ProviderCacheConfig, ProviderLimitsConfig).
    ("provider", "kind"),
    ("provider", "base_url"),
    ("provider", "api_key_env"),
    ("provider", "prompt_cache"),
    ("provider", "context_window"),
    ("provider", "request_timeout_secs"),
    ("provider", "efforts"),
    // [limits] (LimitsConfig).
    ("limits", "max_read_steps"),
    ("limits", "max_parallel_tools"),
    ("limits", "max_concurrent_requests"),
    ("limits", "max_output_tokens"),
    ("limits", "temperature"),
    // [agents] (AgentsConfig).
    ("agents", "global_dir"),
    ("agents", "project_dir"),
    ("agents", "trust_project_agents"),
    ("agents", "confirm_delegation"),
    ("agents", "routing"),
    ("agents", "routing_model"),
    ("agents", "delegation"),
    ("agents", "max_active_subagents"),
    ("agents", "max_depth"),
    ("agents", "stall_threshold_secs"),
    ("agents", "lost_threshold_secs"),
    ("agents", "lost_action"),
    ("agents", "catalog_roots"),
    // [a2a] (A2aConfig) y [a2a.serve] (ServeConfig).
    ("a2a", "role"),
    ("a2a", "request_timeout_secs"),
    ("a2a", "max_card_bytes"),
    ("a2a", "max_result_bytes"),
    ("a2a", "card_refresh_secs"),
    ("a2a", "ca_file"),
    ("a2a", "trusted_jwks"),
    ("a2a.serve", "listen"),
    ("a2a.serve", "agent"),
    ("a2a.serve", "auth"),
    ("a2a.serve", "public_url"),
    ("a2a.serve", "max_concurrent_tasks"),
    ("a2a.serve", "max_wait_secs"),
    ("a2a.serve", "max_task_secs"),
    ("a2a.serve.auth", "scheme"),
    ("a2a.serve.auth", "name"),
    ("a2a.serve.auth", "env"),
    // [ui], [compaction], [budget], [output_filter], [flows.<kind>].
    ("ui", "verbose_provider_notices"),
    ("compaction", "background"),
    ("budget", "session_tokens"),
    ("budget", "session_cost_usd"),
    ("budget", "hard_stop"),
    ("output_filter", "enabled"),
    ("flows.<kind>", "enabled"),
    // providers.toml (ProvidersRegistry y LocalProvider).
    ("providers.toml", "default"),
    ("providers.toml", "id"),
    ("providers.toml", "name"),
    ("providers.toml", "kind"),
    ("providers.toml", "base_url"),
    ("providers.toml", "api_key_env"),
    ("providers.toml", "model"),
    ("providers.toml", "context_window"),
    // runtime.toml (RuntimeUserConfig y SupervisorSettings).
    ("supervisor", "provider"),
    ("supervisor", "model"),
];

/// Ficheros del código donde se declaran las estructuras de `CAMPOS_DE_CONFIGURACION`.
const FUENTES_DE_CONFIGURACION: &[&str] = &[
    "crates/loomcoding-core/src/config/mod.rs",
    "crates/loomcoding-core/src/config/limits.rs",
    "crates/loomcoding-core/src/config/agent_limits.rs",
    "crates/loomcoding-core/src/config/flows.rs",
    "crates/loomcoding-core/src/budget.rs",
    "crates/loomcoding-core/src/agent/compact.rs",
    "crates/loomcoding-core/src/output_filter/mod.rs",
    "crates/loomcoding-core/src/providers_registry.rs",
    "crates/loomcoding-a2a/src/config.rs",
    "crates/loomcoding-a2a/src/serve/config.rs",
    "crates/loomcoding-cli/src/server/runtime_tasks.rs",
];

/// Quita la visibilidad de una declaración (`pub`, `pub(crate)`, `pub(super)`), si la hay.
fn quita_visibilidad(texto: &str) -> &str {
    let Some(resto) = texto.strip_prefix("pub") else {
        return texto;
    };
    if let Some(dentro) = resto.strip_prefix('(') {
        return dentro
            .split_once(')')
            .map_or(texto, |(_, tras)| tras.trim_start());
    }
    if resto.starts_with(char::is_whitespace) {
        resto.trim_start()
    } else {
        texto
    }
}

/// Dice si alguna línea de `fuente` declara el campo `campo` (`campo: tipo`, con o sin visibilidad).
fn declara_campo(fuente: &str, campo: &str) -> bool {
    fuente.lines().any(|linea| {
        let t = linea.trim_start();
        [t, quita_visibilidad(t)].iter().any(|candidato| {
            candidato
                .strip_prefix(campo)
                .is_some_and(|resto| resto.trim_start().starts_with(':'))
        })
    })
}

/// Dice si `texto` tiene un título `##` que lleva la sección entre comillas invertidas.
fn tiene_seccion(texto: &str, seccion: &str) -> bool {
    let corchetes = format!("`[{seccion}]`");
    let llana = format!("`{seccion}`");
    texto.lines().any(|linea| {
        linea.starts_with("## ") && (linea.contains(&corchetes) || linea.contains(&llana))
    })
}

/// Dice si `texto` tiene una fila de tabla cuya primera celda es el campo.
fn tiene_fila(texto: &str, campo: &str) -> bool {
    texto
        .lines()
        .any(|linea| linea.trim_start().starts_with(&format!("| `{campo}` |")))
}

#[test]
fn cada_comando_de_commands_aparece_en_la_referencia_en_y_es() {
    let mut faltan = Vec::new();
    for (ruta, texto) in referencias() {
        for nombre in nombres_de_comandos() {
            if !menciona(&texto, nombre) {
                faltan.push(format!("{ruta}: falta {nombre}"));
            }
        }
    }
    assert!(
        faltan.is_empty(),
        "la referencia no documenta todos los comandos de COMMANDS:\n{}",
        faltan.join("\n")
    );
}

#[test]
fn la_referencia_no_cita_comandos_que_ya_no_existen() {
    let existentes = nombres_de_comandos();
    let mut sobran = Vec::new();
    for (ruta, texto) in referencias() {
        for citado in comandos_citados(&texto) {
            if !existentes.contains(&citado.as_str()) {
                sobran.push(format!("{ruta}: cita {citado}, que no está en COMMANDS"));
            }
        }
    }
    assert!(
        sobran.is_empty(),
        "la referencia cita comandos que ya no existen:\n{}",
        sobran.join("\n")
    );
}

#[test]
fn cada_campo_de_configuracion_aparece_en_el_codigo_y_en_la_referencia() {
    let fuentes: String = FUENTES_DE_CONFIGURACION
        .iter()
        .map(|ruta| lee(ruta))
        .collect::<Vec<_>>()
        .join("\n");
    let paginas = [
        (
            "docs/user/en/reference/configuration.md",
            lee("docs/user/en/reference/configuration.md"),
        ),
        (
            "docs/user/es/reference/configuration.md",
            lee("docs/user/es/reference/configuration.md"),
        ),
    ];
    let mut fallos = Vec::new();
    for (seccion, campo) in CAMPOS_DE_CONFIGURACION {
        if !declara_campo(&fuentes, campo) {
            fallos.push(format!(
                "[{seccion}] {campo}: no está declarado en los ficheros de configuración"
            ));
        }
        for (ruta, texto) in &paginas {
            if !seccion.is_empty() && !tiene_seccion(texto, seccion) {
                fallos.push(format!("{ruta}: no hay título para la sección `{seccion}`"));
            }
            if !tiene_fila(texto, campo) {
                fallos.push(format!(
                    "{ruta}: falta la fila de `{campo}` (sección {seccion})"
                ));
            }
        }
    }
    assert!(
        fallos.is_empty(),
        "la referencia de configuración no está sincronizada con el código:\n{}",
        fallos.join("\n")
    );
}

#[test]
fn menciona_distingue_comandos_con_prefijo_comun() {
    assert!(menciona("usa `/agents` para ver", "/agents"));
    assert!(!menciona("usa `/agents` para ver", "/agent"));
    assert!(menciona("`/agent <name>`", "/agent"));
}

#[test]
fn comandos_citados_ignora_bloques_de_codigo() {
    let texto = "Usa `/model <id>` y `/refresh-agents`.\n\n```text\n`/inventado`\n```\n";
    assert_eq!(
        comandos_citados(texto),
        vec!["/model".to_string(), "/refresh-agents".to_string()]
    );
}

#[test]
fn declara_campo_acepta_visibilidades_y_rechaza_prefijos() {
    let fuente = "pub struct A {\n    pub(super) uno: u8,\n    dos: u8,\n    public_url: Option<String>,\n}\n";
    assert!(declara_campo(fuente, "uno"));
    assert!(declara_campo(fuente, "dos"));
    assert!(declara_campo(fuente, "public_url"));
    assert!(!declara_campo(fuente, "public"));
    assert!(!declara_campo(fuente, "tres"));
}
