use crate::error::Result;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub mod artifact_publish;
pub mod artifact_read;
pub mod ask_user;
pub mod bash;
pub mod development_workflow;
pub mod edit;
pub mod gauntlet;
pub mod glob;
pub mod grep;
pub mod job_kill;
pub mod job_list;
pub mod job_output;
pub mod job_start;
pub mod ls;
pub mod lsp;
pub mod multiedit;
pub mod spill;
pub mod subagent;
pub mod subagent_control;
pub mod subagent_list;
pub mod todo_write;
pub mod view;
pub mod web;
pub mod write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Parallel,
    Exclusive,
}

#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Tope de lo que una herramienta puede devolver al modelo. Ciertamente, una salida
/// desmedida (un `glob **/*` que recorre `target/`) desborda el contexto del modelo y
/// el proveedor responde con un 400; de ahí que se recorte siempre, con aviso expreso.
pub const MAX_TOOL_OUTPUT_BYTES: usize = 20_000;

/// Tope del texto variable de un aviso (detalle de un job, resultado de un subagente) que
/// entra al historial como mensaje de usuario, por el que el tope de herramientas no pasa.
pub const MAX_NOTICE_BYTES: usize = 8_000;
const MAX_MCP_TOOLS_PER_AGENT: usize = 128;
const MAX_MCP_SCHEMAS_PER_AGENT: usize = 32_768;

/// Recorta `content` conservando `max / 2` bytes del principio y `max / 2` del final, y
/// sustituye lo omitido por un marcador con el número de bytes suprimidos. Ambos cortes
/// caen en frontera de carácter: el de cabeza retrocede y el de cola avanza, de modo que
/// nunca se conserva más de `max` bytes. Si `content` cabe en `max`, lo devuelve intacto.
pub fn limit_bytes(content: &str, max: usize) -> std::borrow::Cow<'_, str> {
    if content.len() <= max {
        return std::borrow::Cow::Borrowed(content);
    }
    let (head_end, tail_start) = cut_points(content, max);
    let omitted = tail_start - head_end;
    std::borrow::Cow::Owned(format!(
        "{}\n[... {omitted} bytes omitted ...]\n{}",
        &content[..head_end],
        &content[tail_start..]
    ))
}

/// Como `limit_bytes`, pero antepone una cabecera estructurada que declara el recorte, cuántos bytes
/// se muestran de cuántos y cuántos se omiten. Al ir la señal al principio, el modelo la lee antes
/// que el prefijo y no puede tomarlo por el documento completo. Si cabe en `max`, lo devuelve intacto.
pub fn limit_bytes_with_header(content: &str, max: usize) -> std::borrow::Cow<'_, str> {
    if content.len() <= max {
        return std::borrow::Cow::Borrowed(content);
    }
    let (head_end, tail_start) = cut_points(content, max);
    let omitted = tail_start - head_end;
    let shown = content.len() - omitted;
    std::borrow::Cow::Owned(format!(
        "[truncated: showing {shown} of {} bytes, head and tail; {omitted} bytes omitted from the middle]\n{}\n[... {omitted} bytes omitted ...]\n{}",
        content.len(),
        &content[..head_end],
        &content[tail_start..]
    ))
}

/// Puntos de corte de cabeza y de cola para conservar como mucho `max` bytes, ambos en frontera de carácter.
fn cut_points(content: &str, max: usize) -> (usize, usize) {
    let half = max / 2;
    let mut head_end = half;
    while !content.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = content.len() - half;
    while !content.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    (head_end, tail_start)
}

/// Recorta la salida de una herramienta a `MAX_TOOL_OUTPUT_BYTES`; ver `limit_bytes`.
pub fn limit_output(content: &str) -> std::borrow::Cow<'_, str> {
    limit_bytes(content, MAX_TOOL_OUTPUT_BYTES)
}

/// Adapta el texto recibido al formato de fin de línea del archivo destino.
pub(crate) fn match_line_endings(text: &str, file: &str) -> String {
    let crlf = file.matches("\r\n").count();
    let lf = file.matches('\n').count().saturating_sub(crlf);
    let newline = if crlf > lf { "\r\n" } else { "\n" };
    text.replace("\r\n", "\n").replace('\n', newline)
}

#[derive(Clone)]
pub struct ToolCtx {
    pub cwd: PathBuf,
    pub store: Option<Arc<dyn crate::session::SessionStore>>,
    pub permission: Arc<crate::permission::PermissionService>,
    pub session: crate::session::SessionId,
    pub jobs: Arc<dyn crate::job::JobRegistry>,
    pub bus: crate::event::EventBus,
    pub subagents: Option<Arc<dyn crate::subagent::SubagentLauncher>>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    fn mcp_server_id(&self) -> Option<&str> {
        None
    }
    fn execution_mode(&self, _input: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Parallel
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput>;
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }
    pub fn without(&self, name: &str) -> Self {
        let mut registry = self.clone();
        registry.tools.remove(name);
        registry
    }
    pub fn only(&self, names: &[&str]) -> Self {
        let mut registry = Self::new();
        for name in names {
            if let Some(tool) = self.tools.get(*name) {
                registry.tools.insert((*name).to_string(), tool.clone());
            }
        }
        registry
    }
    /// Keeps every built-in tool and only MCP tools allowed by the local profile.
    pub fn with_mcp_scope(&self, server_ids: &[String]) -> Self {
        let mut registry = Self::new();
        let mut schema_bytes = 0;
        let mut mcp_count = 0;
        for (name, tool) in &self.tools {
            if let Some(server_id) = tool.mcp_server_id() {
                if !server_ids.iter().any(|allowed| allowed == server_id)
                    || mcp_count >= MAX_MCP_TOOLS_PER_AGENT
                {
                    continue;
                }
                let size = name.len()
                    + tool.description().len()
                    + serde_json::to_vec(&tool.schema()).map_or(usize::MAX, |schema| schema.len());
                if size > MAX_MCP_SCHEMAS_PER_AGENT.saturating_sub(schema_bytes) {
                    continue;
                }
                schema_bytes += size;
                mcp_count += 1;
            }
            registry.tools.insert(name.clone(), tool.clone());
        }
        registry
    }
    pub fn has_mcp_server(&self, server_id: &str) -> bool {
        self.tools
            .values()
            .any(|tool| tool.mcp_server_id() == Some(server_id))
    }
    pub fn specs(&self) -> Vec<crate::provider::ToolSpec> {
        self.tools
            .values()
            .map(|t| crate::provider::ToolSpec {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.schema(),
                programmatic_allowed: matches!(t.name(), "view" | "glob" | "grep" | "ls")
                    && t.execution_mode(&serde_json::Value::Null) == ExecutionMode::Parallel
                    && t.mcp_server_id().is_none(),
            })
            .collect()
    }

    /// Estimación informativa de los esquemas MCP incluidos en una petición concreta.
    pub fn mcp_usage_components(
        &self,
        included: &[crate::provider::ToolSpec],
    ) -> Vec<crate::session::UsageComponent> {
        let mut by_server = BTreeMap::<String, u64>::new();
        for spec in included {
            let Some(tool) = self.tools.get(&spec.name) else {
                continue;
            };
            let Some(server_id) = tool.mcp_server_id() else {
                continue;
            };
            let tokens = tool_schema_tokens(&spec.name, &spec.description, &spec.input_schema);
            let entry = by_server.entry(server_id.to_owned()).or_default();
            *entry = entry.saturating_add(tokens);
        }
        by_server
            .into_iter()
            .map(|(id, input_tokens)| crate::session::UsageComponent {
                kind: crate::session::UsageComponentKind::Mcp,
                id,
                input_tokens,
            })
            .collect()
    }
}

/// Tokens estimados de la definición de una herramienta (nombre, descripción y esquema), con la misma
/// estimación local que el resto del núcleo.
pub fn tool_schema_tokens(name: &str, description: &str, schema: &serde_json::Value) -> u64 {
    use crate::agent::compact::estimate_text_tokens;
    (estimate_text_tokens(name) as u64)
        .saturating_add(estimate_text_tokens(description) as u64)
        .saturating_add(estimate_text_tokens(&schema.to_string()) as u64)
}

/// Tokens estimados de las definiciones de las herramientas MCP, agrupados por servidor. Las
/// herramientas que no pertenecen a un servidor MCP no cuentan.
pub fn mcp_schema_tokens_by_server(tools: &[std::sync::Arc<dyn Tool>]) -> BTreeMap<String, u64> {
    let mut by_server = BTreeMap::<String, u64>::new();
    for tool in tools {
        let Some(server_id) = tool.mcp_server_id() else {
            continue;
        };
        let entry = by_server.entry(server_id.to_owned()).or_default();
        *entry = entry.saturating_add(tool_schema_tokens(
            tool.name(),
            tool.description(),
            &tool.schema(),
        ));
    }
    by_server
}

/// Herramientas que solo ven los subagentes: la sesión raíz no tiene tarea a la que adjuntarlas.
/// Se registran al lanzar cada hijo, no en `project_registry`.
pub const CHILD_ONLY_TOOLS: &[&str] = &["artifact_publish"];

/// Registro de herramientas de la sesión raíz de un proyecto. `project_env` es el entorno que ve `bash`.
pub fn project_registry(project_env: std::collections::HashMap<String, String>) -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(view::ViewTool));
    tools.register(Arc::new(ls::LsTool));
    tools.register(Arc::new(glob::GlobTool));
    tools.register(Arc::new(grep::GrepTool));
    tools.register(Arc::new(edit::EditTool));
    tools.register(Arc::new(write::WriteTool));
    tools.register(Arc::new(multiedit::MultiEditTool));
    tools.register(Arc::new(lsp::LspTool));
    tools.register(Arc::new(web::WebTool));
    tools.register(Arc::new(bash::ProjectBashTool::new(project_env)));
    tools.register(Arc::new(job_list::JobListTool));
    tools.register(Arc::new(job_output::JobOutputTool));
    tools.register(Arc::new(job_kill::JobKillTool));
    tools.register(Arc::new(job_start::JobStartTool));
    tools.register(Arc::new(subagent::SubagentTool));
    tools.register(Arc::new(subagent_control::SubagentControlTool));
    tools.register(Arc::new(subagent_list::SubagentListTool));
    tools.register(Arc::new(artifact_read::ArtifactReadTool));
    tools.register(Arc::new(todo_write::TodoWriteTool));
    tools.register(Arc::new(ask_user::AskUserTool));
    tools.register(Arc::new(development_workflow::DevelopmentWorkflowTool));
    tools.register(Arc::new(gauntlet::GauntletTool));
    tools
}

#[cfg(test)]
mod tests {
    #[test]
    fn project_registry_exposes_the_expected_tools() {
        let registro = super::project_registry(std::collections::HashMap::new());
        let nombres: Vec<String> = registro.specs().into_iter().map(|s| s.name).collect();
        assert_eq!(nombres.len(), 22, "{nombres:?}");
        for esperada in [
            "bash",
            "edit",
            "subagent",
            "artifact_read",
            "development_workflow",
            "gauntlet",
        ] {
            assert!(nombres.iter().any(|n| n == esperada), "falta {esperada}");
        }
        assert!(!nombres.iter().any(|n| n == "artifact_publish"));
    }

    #[test]
    fn child_only_tools_are_registered_for_children() {
        assert!(super::CHILD_ONLY_TOOLS.contains(&"artifact_publish"));
    }

    use super::*;
    use async_trait::async_trait;

    struct Dummy;
    #[async_trait]
    impl Tool for Dummy {
        fn name(&self) -> &str {
            "dummy"
        }
        fn description(&self) -> &str {
            "d"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn run(
            &self,
            _c: ToolCtx,
            _i: serde_json::Value,
            _x: CancellationToken,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput::ok("ok"))
        }
    }

    struct DummyMcp {
        server: &'static str,
        name: &'static str,
    }
    #[async_trait]
    impl Tool for DummyMcp {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "mcp test"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        fn mcp_server_id(&self) -> Option<&str> {
            Some(self.server)
        }
        async fn run(
            &self,
            _c: ToolCtx,
            _i: serde_json::Value,
            _x: CancellationToken,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput::ok("ok"))
        }
    }

    #[test]
    fn schema_tokens_by_server_counts_only_mcp_tools_and_matches_usage_components() {
        let mut r = ToolRegistry::new();
        r.register(std::sync::Arc::new(Dummy));
        r.register(std::sync::Arc::new(DummyMcp {
            server: "fs",
            name: "fs_read",
        }));
        r.register(std::sync::Arc::new(DummyMcp {
            server: "fs",
            name: "fs_write",
        }));
        r.register(std::sync::Arc::new(DummyMcp {
            server: "git",
            name: "git_log",
        }));
        let tools: Vec<std::sync::Arc<dyn Tool>> = ["dummy", "fs_read", "fs_write", "git_log"]
            .iter()
            .map(|name| r.get(name).unwrap())
            .collect();
        let by_server = mcp_schema_tokens_by_server(&tools);
        assert_eq!(by_server.keys().collect::<Vec<_>>(), ["fs", "git"]);
        assert!(by_server["fs"] > by_server["git"]);
        // Debe coincidir con lo que se apunta por petición para las mismas herramientas.
        for component in r.mcp_usage_components(&r.specs()) {
            assert_eq!(component.input_tokens, by_server[&component.id]);
        }
    }

    #[test]
    fn tool_schema_tokens_grows_with_the_definition() {
        let small = tool_schema_tokens("a", "d", &serde_json::json!({"type":"object"}));
        let large = tool_schema_tokens(
            "a",
            &"d".repeat(400),
            &serde_json::json!({"type":"object","properties":{"x":{"type":"string"}}}),
        );
        assert!(small > 0);
        assert!(large > small + 99);
    }

    #[test]
    fn registers_and_lists_specs() {
        let mut r = ToolRegistry::new();
        r.register(std::sync::Arc::new(Dummy));
        assert!(r.get("dummy").is_some());
        assert_eq!(r.specs()[0].name, "dummy");
    }

    #[test]
    fn mcp_scope_is_default_deny_and_keeps_only_allowed_servers() {
        let mut tools = ToolRegistry::new();
        tools.register(std::sync::Arc::new(Dummy));
        tools.register(std::sync::Arc::new(DummyMcp {
            server: "files",
            name: "mcp__files__read",
        }));
        tools.register(std::sync::Arc::new(DummyMcp {
            server: "calendar",
            name: "mcp__calendar__read",
        }));

        let denied = tools.with_mcp_scope(&[]);
        assert!(denied.get("dummy").is_some());
        assert!(denied.get("mcp__files__read").is_none());

        let scoped = tools.with_mcp_scope(&["files".into()]);
        assert!(scoped.get("mcp__files__read").is_some());
        assert!(scoped.get("mcp__calendar__read").is_none());
        assert!(scoped
            .specs()
            .iter()
            .find(|spec| spec.name == "mcp__files__read")
            .is_some_and(|spec| !spec.programmatic_allowed));
    }

    #[test]
    fn limit_output_keeps_small_and_truncates_large_on_a_char_boundary() {
        assert_eq!(limit_output("hola"), "hola");
        let big = "ñ".repeat(MAX_TOOL_OUTPUT_BYTES);
        let out = limit_output(&big);
        assert!(out.len() < big.len());
        // ADAPTADO: el marcador es ahora «[... N bytes omitted ...]» (cabeza y cola), no el antiguo
        // «[output truncated: ...]». La cota de tamaño se mantiene: el marcador es corto.
        assert!(out.len() < MAX_TOOL_OUTPUT_BYTES + 100);
        assert!(out.contains("\n[... 20000 bytes omitted ...]\n"));
    }

    #[test]
    fn limit_bytes_respects_the_given_cap() {
        assert_eq!(limit_bytes("corto", 10), "corto");
        // Cada «ñ» ocupa 2 bytes. ADAPTADO: con cabeza y cola, cada extremo recibe 25 bytes, que
        // los cortes ajustan a 24 (12 «ñ») y se omiten 152 de 200. El tope sigue verificándose:
        // lo conservado (sin el marcador) no supera los 51 bytes pedidos.
        let big = "ñ".repeat(100);
        let out = limit_bytes(&big, 51);
        let marcador = "\n[... 152 bytes omitted ...]\n";
        assert!(out.starts_with(&"ñ".repeat(12)));
        assert!(!out.starts_with(&"ñ".repeat(13)));
        assert!(out.contains(marcador));
        assert!(out.len() - marcador.len() <= 51);
    }

    #[test]
    fn limit_bytes_keeps_head_and_tail() {
        let original = format!("{}{}{}", "a".repeat(100), "m".repeat(800), "z".repeat(100));
        let out = limit_bytes(&original, 200);
        // Se conservan 100 bytes de cada extremo; omitidos = 1 000 − 200 = 800.
        assert!(out.starts_with(&"a".repeat(100)));
        assert!(out.ends_with(&"z".repeat(100)));
        assert!(out.contains("\n[... 800 bytes omitted ...]\n"));
        assert!(out.len() < original.len());
    }

    #[test]
    fn limit_bytes_cuts_on_char_boundaries_at_both_ends() {
        // Cada «ñ» ocupa 2 bytes. Con max 51 el corte de cabeza (25) cae a mitad de carácter y
        // retrocede a 24; el de cola (175) también y avanza a 176. Se conservan 48 bytes.
        let original = "ñ".repeat(100);
        let out = limit_bytes(&original, 51);
        assert!(out.starts_with(&"ñ".repeat(12)));
        assert!(!out.starts_with(&"ñ".repeat(13)));
        assert!(out.ends_with(&"ñ".repeat(12)));
        assert!(out.contains("\n[... 152 bytes omitted ...]\n"));
    }
}
