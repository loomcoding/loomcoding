# Inventario de specs y planes

Fotografía del 2026-10-30. Se obtuvo leyendo la línea `Estado:` de cada documento
y cotejándola con el repositorio: existencia de los ficheros que cada plan crea,
presencia de los símbolos que describe y existencia en `master` de los commits que
cita (117 de 118; el restante es una revisión de Crossterm, ajena a este repositorio).

Las casillas `- [x]` de los planes anteriores al 2026-10-21 nunca se marcaron; no
son fiables. Manda la línea `Estado:` de la cabecera.

## Hecho

- **Fundamentos (10-13 a 10-20):** MVP, `batten` (v0, primitivas, showcase, sidebar-logo,
  composer-editing, spinner-styles), modified-files, permission-dialog, input-loop,
  markdown, ansi-sanitize, color-depth, osc8-links, transcript (focus, geometry,
  selection, virtual, activity), look-layout, look-presets, todos-and-questions.
- **A2A (A4 a A13):** wire-names, protocol-types, agent-card, in-process, remote-transport,
  remote-auth, serve, streaming, remote-closure, task-events, cancel, follow-up,
  delegation-choice, router, registry, task-model, states-ui, subagent-tree.
- **Observabilidad:** O1, O2, O3 y child-cost.
- **Núcleo y TUI (10-21):** models-catalog, reasoning-effort, usage-ledger,
  subagent-input-required, mouse-focus-selection, transcript-selection-blocks, tool-atoms.
- **Contexto y coste:** context-compaction, background-compaction, budget-warning,
  context-meter, context-summary; el decálogo está vigente.
- **Runtime R1 a R6** y sus anexos del 10-27.

## Hecho con un cabo suelto

| Documento | Falta |
|---|---|
| agent-development-workflow | revisión visual |
| programmatic-tool-calling | evaluación con modelos reales |
| precision-usage-live-subagent-supervision | prueba interactiva con provider |
| anthropic-400-subagent-result-duplication | repetir la llamada real |
| subagent-empty-final-response | probarlo en real |
| image-attachments, large-paste-placeholder | build del workspace y TUI real |
| multiline-chat-composer | pegado en terminal Windows |
| agent-card-editor, mcp-agent-scope | revisión visual |
| tool-atoms, context-meter, budget-warning, context-summary | verlos en terminal real |
| coste-informativo-mcp-por-agente, runtime-local-skills, scrollable-capability-widgets | suite del workspace |
| codeloom-m3 (herdr) | validar el pane en una sesión herdr no elevada |

## Sin hacer o sin cierre claro

- **Propuestas con plan:** capacidades-tools-provider-modelo, provider-timeouts,
  sesion-vacia-runtime, coste-real-y-estimado-por-adapter, agent-autonomy, cost-estimate.
- **Sin línea de estado:** a2a-observatory, default-ui, development-agents,
  readme-branding, peer-delegation, agent-e2e, expanded-tools.
- **Ideas en exploración (no implementar):** decision-hub-jevalan, gauntlet-loop,
  kpis-prompt-compression, memory, skills-ponytail-superpowers, static-decision-signals,
  tool-skill-discovery, turn-summary, voice-prompt-input, batten-selection,
  agent-profile-a2a, agentic-fetch, turn-status-line.
- **Referencia, no ejecutables:** a2a-roadmap, jevalan-roadmap, pendientes,
  agent-first-development-skills.
