# Plan: dashboard de capacidades runtime

> Estado (anotado en la higiene del 2026-11-01): sin cierre declarado: la spec gemela solo consta como «implementación solicitada»; no se ha verificado contra el código en esta higiene.

Spec: [`docs/specs/2026-10-29-codeloom-runtime-capabilities-dashboard.md`](../specs/2026-10-29-codeloom-runtime-capabilities-dashboard.md)

## Fases

1. Auditar los snapshots de MCP, registro de agentes, tools integradas y uso; no
   usar fixtures como fuente de la UI de producción.
2. Añadir un snapshot serializable y acotado para el dashboard: MCP por servidor,
   LSP detectado/registrado/invocado y skills anunciadas/declaradas.
3. Enviar el snapshot al cliente al abrir el agente y volver a publicar la parte
   de skills al cambiar el registro.
4. Renderizar el resumen MCP/LSP/skills en la cabecera persistente (también en
   terminales estrechos) y dejar el detalle por servidor en el rail, mostrando
   estados configurados, inicializados, fallidos y scopes declarados.
5. Revisar coherencia con contadores de uso y workflow; ejecutar `cargo fmt
   --check`, `cargo build` y `cargo clippy -- -D warnings`. La prueba manual debe
   cubrir MCP configurado y ausente, disponibilidad de rust-analyzer y skills de
   tarjeta/scope, sin enviar peticiones de prueba a providers reales.

## Restricciones

- No implementar el runtime `SKILL.md`, MCP hot-reload ni sesiones LSP persistentes.
- MCP continúa disponible solo en delegaciones locales autorizadas; el dashboard
  describe estado y alcance, no concede acceso.
- No modificar estados de workflow o política de permisos para hacerlos parecer
  sincronizados: la vista consume snapshots autoritativos.

## Estado

- [x] Inventariar fuentes reales y distinguir capacidades cargadas de scopes declarados.
- [x] Añadir y transportar el snapshot runtime por IPC y actualizar skills al cambiar el registro.
- [x] Renderizar widgets MCP, LSP y skills en el dashboard sin valores mock.
- [x] Hacer visible el resumen aunque el rail esté oculto o la terminal sea estrecha.
- [x] `cargo fmt --all --check`.
- [x] `cargo clippy -p codeloom-cli -- -D warnings`.
- [x] `cargo build -p codeloom-cli --bin loom` en `target/debug/loom.exe`.
- [ ] Prueba manual de la vista con MCP configurado/ausente, LSP y skills. Pendiente de abrir el binario recién compilado y revisar la cabecera/rail en el terminal real.
