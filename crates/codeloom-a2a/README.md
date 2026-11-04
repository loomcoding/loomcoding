# codeloom-a2a

**Responsabilidad:** el borde de red de A2A: traduce los tipos del núcleo a los nombres del
cable v1.0, consume agentes remotos (cliente) y sirve CodeLoom como agente A2A de solo lectura.

**No va aquí:** lógica del motor (bucle, herramientas, sesiones): vive en `codeloom-core`. Este
crate depende de `core`, nunca al revés.

**Tipos clave:** `WireAgentCard` y `WireInterface` (`card.rs`), `JsonRpcClient` y
`A2aClientFactory` (`client/factory.rs`), `LauncherHost` y `HostLimits` (`serve/host.rs`).

**Puntos de entrada:** `client/` (llamar a un agente remoto), `serve/` (`mod.rs`, `rpc.rs`,
`auth.rs`, `launch.rs`: servidor), `sse.rs` (streaming), `oauth.rs`/`security/`/`signing.rs`
(autenticación y verificación de tarjetas).

**Cómo se testea:** `wiremock` y `tower` para simular el par remoto; `tempfile` para
configuración. Sin red real.

**Trampas:**
- El servidor solo escucha en bucle local y siempre con autenticación.
- No inventes estados ni semántica de agentes: se contrasta con el proto normativo y lo propio
  se rotula como propio.
- Estado de cada tarea A2A: `docs/plans/2026-10-22-codeloom-a2a-roadmap.md`.
