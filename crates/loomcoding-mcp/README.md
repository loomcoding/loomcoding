# loomcoding-mcp

**Responsabilidad:** integración MCP: conecta con servidores MCP y expone sus herramientas al
motor como `Tool`.

**No va aquí:** el registro de herramientas ni el bucle del agente (`loomcoding-core`).

**Tipos clave:** `McpConfig` y `McpServerConfig` (configuración), `McpRuntime` (conexiones
vivas), `McpServerStatus` y `McpIssue` (estado y fallos), en `lib.rs`.

**Puntos de entrada:** `lib.rs` (un solo fichero). El motor filtra por alcance con
`ToolRegistry::with_mcp_scope`.

**Cómo se testea:** tests unitarios en el fichero; sin servidores MCP reales ni red.

**Trampas:** el coste en tokens de los esquemas MCP se mide al conectar y avisa por umbrales
(`docs/INDEX.md`, «mcp-schema-cost»); no añadas herramientas sin cota de salida.
