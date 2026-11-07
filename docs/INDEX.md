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

- **Fundamentos (10-13 a 10-20):** MVP, `cloth` (v0, primitivas, showcase, sidebar-logo,
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
  `prompt-cache-points` (dos puntos de caché en el adaptador de Anthropic, medido en real; configurable con `[provider] prompt_cache`, por debajo de `LOOMCODING_PROMPT_CACHE`: `prompt-cache-config`) y
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
| loomcoding-m3 (herdr) | validar el pane en una sesión herdr no elevada |

## Sin hacer o sin cierre claro

- **Ideas en exploración (no implementar):** decision-hub-jevalan, gauntlet-loop (recortada en gauntlet-minimo),
  kpis-prompt-compression, memory, static-decision-signals,
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
| estados-de-subagente-y-composer-compartido (11-04) | hecho | Fases 0 a 6 hechas, con los matices que el plan declara; supervisor probado con un remoto en `loomcoding-a2a/tests/supervised_remote.rs` (cotejado 2026-11-05) |
| watchdog-de-subagentes (11-05) | hecho | Fase 3 del anterior, commit `70a21f9`; el supervisor y `lost_action` (defecto `notify`) ya están implementados |
| organizacion-y-navegabilidad (11-05) | hecho | Fases 0 a 8: navegación, guardarraíles y todos los monolitos partidos; lista blanca vacía |
| avisos-proveedor-spinner (11-03) | hecho | `/verbose` (`commands.rs`) y `provider_in_flight`; el plan lo declara hecho. Abierto en el spec: si `/verbose` debe persistir en `loomcoding.toml` |
| temas-efectos-matrix-rain (11-02) | hecho | `MatrixConfig`, `/temas`, `draw_temas`, lluvia en `App` y `[effects.matrix]` documentado en `crates/loomcoding-cloth/README.md` |
| loomcoding-renombrado (11-02) | hecho | renombrado mecánico ejecutado el 2026-11-05 (CodeLoom → Loomcoding, `batten` → `loomcoding-cloth`); pendiente solo la búsqueda de marca y el anuncio público |
| gauntlet-minimo (11-04) | hecho | recorte de `gauntlet-loop`: herramienta `gauntlet` (constructor + crítico ciego), persistencia, observabilidad y E2E (pasos 1 a 10 del plan). Revisión visual hecha con el renderizador real (las líneas se parten en el transcript estrecho; cosmético); sin E2E por PTY, que exigiría un guion de padre que llame a `advance` |
| autonomia-y-limite-de-pasos (11-04) | hecho | rondas de lectura aparte (`[limits]`, `read_step_cap`), pregunta al llegar al tope, traspaso del subagente (`agent/handoff.rs`), topes de herramientas y de peticiones (`LimitedProvider`), guía en el prompt y modo Dios (`Ctrl+X`, `kill_everything`); plan de 10 tareas en `docs/plans/2026-11-04-loomcoding-autonomia-y-limite-de-pasos.md`. Piloto con el 9B local: T3 interrumpida sin resultado (proceso muerto, trabajo parcial y fuera de encargo); T4 a T10 las hizo Claude directamente |
| servidores-locales-ventana-contexto (11-04) | hecho | `[provider] context_window` y `request_timeout_secs`, reconocimiento del desborde de llama.cpp y cuerpo del error en el adaptador OpenAI-compatible; cabo suelto: criterio 8 (comprobación manual con `LOOMCODING_HOME` aislado) sin verificar |
| help-site-mdbook (11-01) | propuesta | pendiente de aprobación; sin código |
| agents-picker-filters (11-01) | hecho | filtros conectados al teclado y al popup (`app/agents.rs`, `run/keys.rs`, `render/a2a.rs`); cotejado 2026-11-05 |
| edit-line-endings (11-01) | hecho | `match_line_endings` (`tools/mod.rs`) usado por `edit` y `multiedit` |
| session-picker-rows, session-project-identity, tool-loop-turn-scope (11-01) | hecho | lista sin ajuste de línea (`render/settings.rs`); clave `cwd#loomcoding-agent` con lectura de la anterior por prefijo (`session/sqlite/sessions.rs`); detección de bucles desde el último mensaje de usuario (`agent/stop.rs`) |
| deuda-tecnica (10-16) | referencia | registro vivo de limitaciones |
| development-workflow-interactive-reliability, runtime-capabilities-dashboard (10-29) | sin cierre declarado | anotado en la higiene del 11-01 |
| local-server-client-runtime (10-26) | hecho | es el «Runtime R1 a R6» de más arriba |
| despertar-sin-tui (11-05) | hecho | `AgentLoop::run_until_settled` (`agent/settle.rs`) usado por `--print` mediante `run_print_turn`; incluye informe de turno sin respuesta, cancelación al desconectar y `StopReason::OutputLimit` |
| mejorar-el-sistema-del-modelo-pequeno (11-05) | propuesta | método de experimentos (una variable cada vez) para ver cuánto rinde un modelo pequeño con mejor ciclo de verificación, edición y contexto estructural; sin plan ni código |
| verificar-tras-editar (11-05) | propuesta | herramienta `check` y puerta de cierre: un turno que edita Rust no termina sin compilar; línea 1 del anterior; sin plan ni código |
| extraer-flujos-del-core (11-05) | propuesta (con plan de 9 tareas, 0 a 8; la 0 endurece el guardarraíl de tamaño existente a 400 líneas) | sacar el Gauntlet de `loomcoding-core` a un crate `loomcoding-workflows` mediante tres puntos de extensión genéricos (almacén de flujos, evento `FlowChanged`, `Tool::is_orchestrator`) y una regla de tamaño de 400 líneas con lista que solo encoge; `development_workflow` irá después con el mismo método |
| ajustes-modelo-local (11-05) | propuesta (plan de 4 tareas) | `[limits] temperature`, `[provider] efforts` declarables y detección de salida repetida (`RepeatedOutput`); los subagentes heredan los límites del padre |
| dashboard-limpio (11-05) | propuesta (plan de 9 tareas) | pantalla más limpia: rail adelgazado por defecto, disposiciones `Minimal` y `Cards` seleccionables, cabecera de una línea, pie de cuatro atajos y muñeco braille cuya postura sigue el estado del agente |
| menu-izquierdo (11-06) | propuesta (plan de 7 tareas) | menú izquierdo del `Workspace`: logo «LOOM» con un efecto al azar cada cierto tiempo (lluvia binaria, reflejo, tipografía, glitch; `tachyonfx` y `MatrixRain`, sin reloj en reposo), secciones sin mayúsculas ni contadores a cero y diff de `git` desplegable por fichero tocado en la sesión |
| aspecto-unificado (11-06) | propuesta (sin plan propio) | tres capas de aspecto —paleta, estilo y distribución— con un solo sitio de elección cada una; la regla de que un widget nuevo derive de la capa existente; preguntas resueltas en panel-ui |
| panel-ui (11-06) | hecho (T1–T6) | `/ui`: un único comando que amplía el árbol de `/temas` con tema, estilo, distribución, efectos, spinner, muñeco, menú lateral y ratón; los comandos visuales antiguos quedan como alias ocultos y los temas de usuario pueden declarar `style` y `layout` |
| model-effort (11-06) | propuesta (plan de 3 tareas) | comandos `/model` y `/effort`: atajos directos al flujo de ajustes del proveedor para cambiar el modelo y el esfuerzo de razonamiento sin abrir `/settings`, con selector cuando se teclean sin argumento |
| carril-transcript (11-06) | hecho (T1–T3; seis estilos) | fila «Carril» de `/ui` con seis valores (`tree`, `dots`, `bar`, `box`, `tint`, `none`) para el trazo que acompaña a cada bloque del transcript, a la manera de Crush, MastraCode, Pi y Codex |
| job-watch (11-06) | hecho (T1–T5) | parámetro `watch` de `job_start` y herramienta `job_watch` para un job ya lanzado: avisos a la conversación, acotados y con despertar del padre, cuando una línea de la salida de un job en marcha casa con un patrón (la contrapartida de `Monitor`); sin planificador de DAG |
| providers-locales (11-06) | hecho (T1–T5; falta el volcado e2e) | registro global `providers.toml` con llama-server, Strata y PolyStrata como proveedores por defecto, sondeo `GET /models` para descubrir modelos y comando `/provider` |
| compresion-de-salida (11-06) | implementada salvo la tarea 4 (dedup de lecturas, aplazada hasta medir); apagada por defecto; guía `docs/output-filter.md` | capa determinista que reduce con criterio la salida de las herramientas antes de acotarla (ANSI, repeticiones, reglas por comando como `cargo test`), con diez reglas para no perder señal (cabecera obligatoria, original recuperable, lista blanca de señal, `raw`) y medición antes de activarla; sin proxy tipo `headroom` |
| tareas-para-modelo-local (11-06) | propuesta (borrador de experimento, sin plan) | cómo dividir el trabajo para un modelo local de 3B a 8B: un modelo fuerte descompone el plan en tarjetas pequeñas, el local las ejecuta y un `check` objetivo con reintentos decide; cuatro brazos (monolítico, tarjetas, tarjetas con `check`, tarjetas del propio local), tareas fuera del techo y criterio de adopción fijado antes de correr |
| odd-desarrollo-organico (11-07) | hecho (T1–T8; activo por defecto; guía en `docs/odd.md`) | modo ligero por defecto inspirado en el *Organic Driven Development* de `gentle-shell`: prueba del resumen para clasificar, documento de feature vivo solo para lo grande, casillas con evidencia, cierre bloqueante impuesto por el runtime (con techo de turnos) y fase inferida de las herramientas (etiqueta propia, no estado A2A); `development_workflow` se conserva como el flujo pesado |
| contexto-estructural (11-05) | en curso (tareas 0, 1, 4 y 5 hechas: índice, paquete y herramienta `context`; faltan PageRank, caché en disco, orientación previa y medición) | herramienta `context` (símbolos, usos y pruebas en una llamada, índice sintáctico) y orientación previa en la oficina; línea 3 del método; sin plan ni código |

## Dependencia vigilada: crossterm

Cabe señalar que el `Cargo.toml` raíz sustituye el crossterm de crates.io por una revisión fijada
del fork de eitsupi (rama `feature/windows-vt-input`), porque la 0.29 publicada no entrega
`Event::Paste` en la consola de Windows. El PR correspondiente, crossterm-rs/crossterm#1030,
sigue abierto (comprobado el 2026-10-31). Cuando se fusione y se publique una versión que lo
incluya, ha de borrarse la sección `[patch.crates-io]` y comprobar el pegado con `tui_pty`.

## Qué puede verificar ya el E2E en PTY

Con `crates/loomcoding-cli/tests/tui_pty.rs` (véase `docs/visual-e2e.md`) se pueden añadir escenarios
automáticos para: multiline-chat-composer, image-attachments, atribución `You`
(user-attribution-internal-messages) y subagent-empty-final-response. Cada cabo suelto se cierra
al añadir su escenario; hoy hay cinco cubiertos: arranque, turno con `You`, pegado grande, composer
multilínea y subagente sin respuesta final. Pendiente: image-attachments (Ctrl+V lee el portapapeles
real de la persona usuaria, así que no se automatiza sin aislarlo).
