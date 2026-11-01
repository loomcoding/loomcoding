# Inventario de specs y planes

Fotografía del 2026-10-30, con anexos y una higiene documental del 2026-11-01. Se obtuvo leyendo la línea `Estado:` de cada documento
y cotejándola con el repositorio: existencia de los ficheros que cada plan crea,
presencia de los símbolos que describe y existencia en `master` de los commits que
cita (117 de 118; el restante es una revisión de Crossterm, ajena a este repositorio).

Ubicación: lo hecho y sin cabos sueltos vive en `docs/specs/hechos/` y
`docs/plans/hechos/`. En `docs/specs/` y `docs/plans/` quedan solo lo pendiente,
lo parcialmente implementado y los documentos de referencia vivos (decálogo,
roadmaps, listas de pendientes). En «Hecho» (abajo) aparece todo lo que ya consta
como implementado; lo que sigue en la raíz es la sección «cabo suelto».

Salvedad (2026-11-01): los documentos de las tandas del 2026-10-29 en adelante, aunque estén hechos,
siguen en la raíz: moverlos obliga a reescribir los enlaces relativos entre documentos y no se hizo en
la higiene. Manda siempre la línea `Estado:` de cada cabecera, que en esa pasada se alineó entre
spec y plan en `mcp-schema-cost`, `cache-usage-ledger` e `image-attachments`, y se añadió a doce planes
que carecían de ella (los de `dashboard` y `interactive-reliability` constan como «sin verificar»).

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
  context-meter, context-summary; el decálogo está vigente. Se suma `cache-usage-ledger` (fase 1:
  registro de los tokens de caché comunicados por el proveedor; desglose y TUI quedan pendientes) y
  `mcp-schema-cost` (herramientas y tokens estimados por servidor MCP al conectar, con aviso por umbrales) y
  `prompt-cache-points` (dos puntos de caché en el adaptador de Anthropic, medido en real; configurable con `[provider] prompt_cache`, por debajo de `CODELOOM_PROMPT_CACHE`: `prompt-cache-config`) y
  `context-breakdown-ui` (desglose estimado de la petición y caché acumulada en el panel de uso, solo con datos) y
  `delegation-control` (aprobación por código de cada delegación con `confirm_delegation = always` y mando
  `[agents] delegation = off | on_request | proactive`; solo `off` es garantía de código) y
  `recursive-delegation` (fase 1: `[agents] max_depth` y perfil `max_depth` con dos candados; fase 2: los nietos se ven en la interfaz con sangría y camino; la regla de `delegation` llega a los hijos que delegan: `delegation-mode-for-nested-children`) y
  `external-catalog-profiles` (`[agents] catalog_roots`: agentes y skills de un catálogo externo como perfiles locales, siempre con aprobación por huella ligada a la raíz del catálogo) y
  `user-invoked-agents` (`/agent <nombre> [--skills a,b] <tarea>` y `@nombre …`: la persona usuaria lanza un agente directamente, con un subconjunto de sus skills, sin turno del modelo principal; Tab completa el nombre) y
  `reasoning-effort-benchmark` (medido con Haiku 4.5: sin efecto en ejecución; en revisión difícil `medium` halla más defectos pero añade falsos positivos y cuesta ~24 veces más; `high` pendiente; no se fijan valores).
- **Tanda del 2026-10-31 (implementada, con su plan):** `truncation-signal` (aviso de salida truncada al modelo),
  `budget-hard-stop` (corte duro por presupuesto, también en la familia de subagentes),
  `delegation-limits` (topes de la delegación) y `skill-size-policy` (límites de tamaño de las skills locales).
  `reasoning-effort-hard-battery` está ejecutada en parte: `none` y `medium` medidos, `high` sin datos válidos (HTTP 402 por falta de crédito).
- **Runtime R1 a R6** y sus anexos del 10-27.
- **Verificados el 2026-10-30 contra el código:** provider-timeouts, coste-real-y-estimado-por-adapter,
  parent-wakeup, chat-demo-streaming, cost-estimate; conversation-composer-review queda superado
  por transcript-activity.
- **Suite del workspace del 2026-10-30:** coste-informativo-mcp-por-agente y runtime-local-skills
  (cerrados con `fmt`, `clippy`, `build` y `test` completos en verde).

## Hecho con un cabo suelto

| Documento | Falta |
|---|---|
| agent-development-workflow | revisión visual |
| programmatic-tool-calling | evaluación con modelos reales |
| precision-usage-live-subagent-supervision | prueba interactiva con provider |
| anthropic-400-subagent-result-duplication | repetir la llamada real |
| subagent-empty-final-response | cubierto por `tui_pty::subagent_without_final_text_fails_instead_of_completing_empty` (proveedor simulado con guion, límite 3/3); queda la revisión de aspecto |
| image-attachments | revisión visual en la TUI (el build y los tests del workspace ya pasan) |
| large-paste-placeholder | revisión de aspecto por una persona (el marcador y la edición posterior los cubre `tui_pty`) |
| multiline-chat-composer | cubierto por `tui_pty::ctrl_j_and_paste_add_lines_and_only_enter_sends` (Ctrl+J, pegado y envío con Enter en ConPTY); queda la revisión de aspecto |
| agent-card-editor, mcp-agent-scope | revisión visual |
| tool-atoms, context-meter, budget-warning, context-summary | verlos en terminal real |
| scrollable-capability-widgets | revisión visual en terminal real (la suite del workspace ya pasa) |
| e2e-visual (PTY y ttyd) | captura de píxeles con Chrome: `Page.captureScreenshot` agotó el tiempo |
| codeloom-m3 (herdr) | validar el pane en una sesión herdr no elevada |

## Sin hacer o sin cierre claro

- **Parciales:** capacidades-tools-provider-modelo (registro y clasificador hechos, cableado solo en
  Anthropic; faltan Chat Completions, Responses, Gemini y metadata real), agent-autonomy (hechas la Fase A y la O1; faltan B, C y D),
  sesion-vacia-runtime (falta la prueba de tool en la TUI con OpenRouter) y
  openrouter-anthropic-ptc (compatibilidad PTC sin confirmar).
- **Ideas en exploración (no implementar):** decision-hub-jevalan, gauntlet-loop,
  kpis-prompt-compression, memory, skills-ponytail-superpowers, static-decision-signals,
  tool-skill-discovery, turn-summary, voice-prompt-input, batten-selection,
  agent-profile-a2a, agentic-fetch, turn-status-line.
- **Borradores y estudios sin aprobar (2026-10-31):** `decalogo-agent-first` (principios candidatos, a discusión) y
  `ecc-agent-workforce-research` (investigación de la persona usuaria; no autoriza cambios).
- **Fuera de este inventario hasta revisarlas:** `self-hosting-bootstrap` (evaluación), `mcp-skills-hot-reload`,
  `decision-experiments` (borradores), `composer-tool-visual-references` y `batten-reference-findings-showcase`
  (análisis ya completados, sin código asociado).
- **A verificar contra el código antes de planificar:** `agent-autonomy` fases B (artefactos) y C (tareas enlazadas
  y contexto): hay rastro de artefactos y de contexto en `a2a.rs`, `remote_task.rs` y `event/mod.rs`, pero la spec las da por pendientes.
- **Referencia, no ejecutables:** a2a-roadmap, jevalan-roadmap, pendientes,
  agent-first-development-skills.

## Dependencia vigilada: crossterm

Cabe señalar que el `Cargo.toml` raíz sustituye el crossterm de crates.io por una revisión fijada
del fork de eitsupi (rama `feature/windows-vt-input`), porque la 0.29 publicada no entrega
`Event::Paste` en la consola de Windows. El PR correspondiente, crossterm-rs/crossterm#1030,
sigue abierto (comprobado el 2026-10-31). Cuando se fusione y se publique una versión que lo
incluya, ha de borrarse la sección `[patch.crates-io]` y comprobar el pegado con `tui_pty`.

## Qué puede verificar ya el E2E en PTY

Con `crates/codeloom-cli/tests/tui_pty.rs` (véase `docs/visual-e2e.md`) se pueden añadir escenarios
automáticos para: multiline-chat-composer, image-attachments, atribución `You`
(user-attribution-internal-messages) y subagent-empty-final-response. Cada cabo suelto se cierra
al añadir su escenario; hoy hay cinco cubiertos: arranque, turno con `You`, pegado grande, composer
multilínea y subagente sin respuesta final. Pendiente: image-attachments (Ctrl+V lee el portapapeles
real de la persona usuaria, así que no se automatiza sin aislarlo).
