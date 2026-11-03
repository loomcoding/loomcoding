# Índice de specs y planes

Cabe señalar que este documento es el **índice** de `docs/specs/` y `docs/plans/` (antes se
llamaba `inventario.md`). **Regla para quien vaya a implementar algo:** consulta primero este
índice; si el documento que ibas a leer figura como superado, lee el que lo sustituye. Los
estados admitidos son `hecho`, `en curso`, `propuesta`, `idea` (no implementar), `referencia`
y `superado`. Un documento sin línea `Estado:` en su cabecera consta aquí como «sin estado
declarado» y no debe darse por vigente sin cotejarlo con el código.

Fotografía del 2026-10-30, con anexos y una higiene documental del 2026-11-01. Se obtuvo leyendo la línea `Estado:` de cada documento
y cotejándola con el repositorio: existencia de los ficheros que cada plan crea,
presencia de los símbolos que describe y existencia en `master` de los commits que
cita (117 de 118; el restante es una revisión de Crossterm, ajena a este repositorio).

Ubicación (regla desde el 2026-11-01): `docs/specs/` y `docs/plans/` guardan solo lo que
está por hacer o en curso: ideas y borradores, lo parcial, lo hecho con un cabo suelto
declarado (revisión visual, prueba con proveedor real…) y los documentos de referencia vivos
(decálogo, roadmaps, listas de pendientes). Lo terminado y sin trabajo pendiente vive en
`docs/specs/hechos/` y `docs/plans/hechos/`; al cerrarse el último cabo, el par se mueve allí.
En «Hecho» (abajo) aparece todo lo que ya consta como implementado, esté donde esté.

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
  `child-progress-artifacts` (el hijo publica avances con nombre con `artifact_publish`; el padre los lee con `artifact_read`; el nodo del hijo muestra de 1 a 3 líneas del último; A2A los transmite) y
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
- **`agent-autonomy`, verificado el 2026-11-01:** A, B, C, D y `follow_up_of` están hechos (la B con `child-progress-artifacts`). La restricción de `send_message`
  se descartó a propósito (`answer` ya cubre `input-required`). Detalle en la cabecera de la spec.
- **Referencia, no ejecutables:** a2a-roadmap, jevalan-roadmap, pendientes,
  agent-first-development-skills.

## Tanda del 2026-11-01 al 2026-11-05 (añadida al convertir esto en índice)

| Documento | Estado | Nota |
|---|---|---|
| estados-de-subagente-y-composer-compartido (11-04) | en curso | Fases 1 y 3 hechas (`subagent/phase.rs`, `subagent/watchdog.rs`); restan las demás |
| watchdog-de-subagentes (11-05) | hecho | Fase 3 del anterior, commit `70a21f9`; el supervisor (Fase 5) tiene precondiciones en el spec §8 |
| organizacion-y-navegabilidad (11-05) | en curso | Fase 0: hecho `ARCHITECTURE.md` y este índice |
| avisos-proveedor-spinner (11-03) | sin estado declarado | `/verbose` existe en `commands.rs`; el resto sin cotejar |
| temas-efectos-matrix-rain (11-02) | propuesta | ya hay `widgets/matrix_rain.rs` en `batten`; sin cotejar el alcance |
| loomcoding-renombrado (11-02) | propuesta | |
| help-site-mdbook (11-01) | propuesta | pendiente de aprobación; sin código |
| agents-picker-filters (11-01) | sin estado declarado | hay código (`agents_filter_edit`, `agents_tag_filter` en `app/`) |
| edit-line-endings (11-01) | sin estado declarado | hay código (`match_line_endings` en `tools/edit.rs`) |
| session-picker-rows, session-project-identity, tool-loop-turn-scope (11-01) | sin estado declarado | sin cotejar con el código |
| deuda-tecnica (10-16) | referencia | registro vivo de limitaciones |
| development-workflow-interactive-reliability, runtime-capabilities-dashboard (10-29) | sin cierre declarado | anotado en la higiene del 11-01 |
| local-server-client-runtime (10-26) | hecho | es el «Runtime R1 a R6» de más arriba |

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
