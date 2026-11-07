# Mapa de código orientado a tareas

Cabe señalar que este mapa responde a «¿dónde toco para X?», no a «¿qué es cada módulo?»
(para eso, `docs/ARCHITECTURE.md`). Todas las rutas existen a fecha de 2026-11-05; si mueves
un archivo, corrígelo aquí en el mismo commit.

Antes de nada, para cualquier tarea: lee el decálogo de contexto y coste
(`docs/specs/2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`) y comprueba en
`docs/INDEX.md` que el spec que vas a seguir no está superado.

## Quiero añadir una herramienta (tool)

1. Crea `crates/loomcoding-core/src/tools/<nombre>.rs` e implementa `trait Tool`
   (`name`, `description`, `schema`, `execution_mode`, `run`), tomando como modelo
   `tools/glob.rs` (lectura) o `tools/write.rs` (escritura).
2. Decláralo con `mod <nombre>;` y regístralo en `project_registry`
   (`crates/loomcoding-core/src/tools/mod.rs`, `tools.register(Arc::new(...))`).
3. `execution_mode`: **`Exclusive` si escribe**, `Parallel` si solo lee.
4. **Pon cota a la salida** (decálogo); si puede ser grande, mira `tools/spill.rs`.
5. Test en el propio archivo (`#[cfg(test)]`), sin red real.
6. Si necesita render propio en la TUI: `crates/loomcoding-cloth/src/state/tool_atom/`.

Ojo: `crates/loomcoding-core/src/registry/` NO es el registro de herramientas, es el de agentes.

## Quiero añadir un provider

1. Variante en `ProviderKind` (`crates/loomcoding-core/src/config/mod.rs`).
2. Cliente en `crates/loomcoding-core/src/provider/<nombre>.rs` implementando `trait Provider`
   (`capabilities`, `stream`); `provider/sse.rs` tiene el análisis de SSE común.
3. Selección en `build_provider` (`crates/loomcoding-cli/src/main.rs`).
4. Tests con `wiremock`; nada de red real. Los `tests/*_live.rs` son opt-in.

### Proveedores locales globales (llama-server, Strata, PolyStrata)

- Registro: `crates/loomcoding-core/src/providers_registry.rs` (`providers.toml` en `<perfil>/.loomcoding/`, sembrado
  si falta, nunca pisado). El cli lo carga en `crates/loomcoding-cli/src/registro_local.rs`; un proyecto sin
  `loomcoding.toml` nace con su `default` (`load_or_create_with`), y un servidor de bucle sin clave usa la ficticia `local`.
- Sondeo: `providers_probe.rs` (un solo `GET <base_url>/models`, 1,5 s, 256 KiB, 200 modelos; jamás inferencia) y
  `providers_catalog.rs` (lo traduce a `CatalogProvider`, con el estado en el nombre). `crates/loomcoding-cli/src/sondeo_local.rs`
  lo lanza en segundo plano, fusiona con el catálogo (`Catalog::with_local_providers`) y enfría 5 s los refrescos.
- TUI: `app/providers_refresh.rs` pide el refresco al abrir `/model`, `/settings` y el selector; `/provider` vive en
  `app/provider_command.rs` con `render/provider_picker.rs` y `run/provider_picker_keys.rs`.

## Quiero añadir un evento al bus

1. Variante en `LiveEvent` (`crates/loomcoding-core/src/event/mod.rs`).
2. Clasifícala en `is_child_relevant()` (mismo archivo): es lo que decide qué ve el padre.
3. Publícala donde ocurre el hecho (`EventBus`).
4. Consumo en la TUI: `crates/loomcoding-tui/src/app/events.rs` (`App::apply`).
5. ¿Debe sobrevivir al reinicio? Entonces persiste en `crates/loomcoding-core/src/session/sqlite/` (el módulo del tema: `subagents.rs`, `sessions.rs`, `usage.rs`, `workflows.rs` o `gauntlets.rs`; el esquema, en `schema.rs`).
6. Si viaja por IPC al cliente, el reenvío está en `crates/loomcoding-cli/src/server/` (`worker.rs`: `forward_agent_events`; los tipos del cable, en `protocol.rs`).

## Quiero tocar el Gauntlet (constructor + crítico ciego)

1. Reglas puras (fases, orden A/B, veredicto, parseo acotado): `crates/loomcoding-core/src/gauntlet.rs`;
   `GauntletPhase::can_transition_to` es la única tabla de transiciones.
2. Acciones de la herramienta (`start`, `advance`, `accept`, `cancel`, `status`): `crates/loomcoding-core/src/tools/gauntlet/`
   (`start.rs`, `advance.rs`, `decide.rs`, `status.rs`; prompts y `save` en `helpers.rs`).
3. Todo cambio de fase pasa por `helpers::save`, que persiste y publica `LiveEvent::GauntletChanged`: no llames a
   `update_gauntlet` por otro camino o la conversación no lo verá.
4. Persistencia: `session/sqlite/gauntlets.rs` y los métodos de `SessionStore` en `session/mod.rs`.
5. Línea del transcript y reenvío al abrir sesión: `crates/loomcoding-tui/src/app/events.rs` (`GauntletChanged`),
   `run/mod.rs` (`SessionUpdate::Gauntlet`) y `crates/loomcoding-cli/src/server/snapshot.rs`.
6. Puerta de escritura del padre y fin de turno tras `start`: `agent/dispatch.rs` y `agent/turn.rs`.
7. Prueba de punta a punta: `crates/loomcoding-core/src/subagent/tests/gauntlet_e2e.rs`.
8. Guardas de integridad (lectura del crítico, `check`, `hide_bar`): `gauntlet.rs::unread_candidates`,
   `tools/gauntlet/advance.rs` (`apply_next`) y `tools/gauntlet/helpers.rs` (`run_check`); sus pruebas, en
   `tools/gauntlet/tests/guards.rs`.
9. Prueba real contra un servidor local (ignorada): `crates/loomcoding-core/tests/gauntlet_local_live.rs`.

## Quiero añadir una fase de subagente

1. Variante en `SubagentPhase` y en `ALL` (`crates/loomcoding-core/src/subagent/phase.rs`).
2. Completa **todos** los `match` del mismo archivo: `to_task_state`, `from_remote_state`,
   `as_str`, y la tabla de `can_transition_to` (única fuente de verdad de transiciones).
3. Decide si el vigilante debe ignorarla (esperas al humano): `subagent/watchdog.rs`.
4. La fase se persiste por su `as_str` (`session/sqlite/codec.rs` y `subagents.rs`); una fase nueva no rompe filas viejas.
5. Si la UI agrupa estados, nombra entre paréntesis los A2A originales
   (`crates/loomcoding-tui/src/rail/`, sección `subagents.rs`).
6. Tests: `todas_las_fases_proyectan` y compañía ya recorren `ALL`.

## Quiero añadir un widget de TUI

1. Estado puro y testeable en `crates/loomcoding-cloth/src/state/<nombre>.rs` (sin `ratatui`).
2. Widget en `crates/loomcoding-cloth/src/widgets/<nombre>.rs`; decláralo en `widgets/mod.rs`.
3. Colores del tema en `crates/loomcoding-cloth/src/widgets/theme/`; no uses colores sueltos.
4. Úsalo desde la zona que corresponda en `crates/loomcoding-tui/src/render/` (`chrome.rs` cabecera y pie, `transcript.rs`, `sidebar.rs`, `settings.rs`, `overlays.rs`, `a2a.rs`); `render/mod.rs` solo orquesta.
5. Si es específico de Loomcoding y no genérico, va en `crates/loomcoding-tui/src/widgets/`.

## Quiero añadir un comando de composer (`/algo`)

1. Variante en `Command` y fila en la tabla de `crates/loomcoding-tui/src/commands.rs`.
2. Su ejecución, en el `match` de comandos de `crates/loomcoding-tui/src/run/slash.rs`
   (ejemplo: `Command::Verbose`).
3. Estado que necesite: `crates/loomcoding-tui/src/app/` (`mod.rs` define `App`; ver el índice de módulos en el README del crate).
4. El composer no puede parpadear ni ir con retraso: comprueba con `tui_pty` si tocas teclas.
5. Si el comando lleva argumento, añádelo a la lista de `slash_query` (`app/prefs.rs`) para que el popup no se
   cierre al teclearlo, y resuelve los rechazos **antes** de borrar el compositor (así se conserva el texto).

### `/ui` y los alias de aspecto

El panel «Interfaz» (raíz de `/temas`) y su comando `/ui [fila] [valor]` viven en `crates/loomcoding-tui/src/app/ui_command.rs`
(`App::ui_command`, `App::ui_row_set`, sobre `UiRow`). Los comandos de aspecto antiguos (`/theme`, `/layout`, `/temas`,
`/crush`, `/classic`, `/mascot`, `/sidebar`, `/mouse`) siguen funcionando como alias ocultos: están listados en
`crates/loomcoding-tui/src/commands_alias.rs` (`HIDDEN_ALIASES`), no salen en el popup de `/` (lo filtra
`slash_matches` en `app/prefs.rs`) y se ejecutan al escribirlos enteros y pulsar Enter (`run/keys.rs`). Un comando de
aspecto nuevo entra como fila de `UiRow`, no como comando suelto.

### Carril del transcript (`/ui carril`)

El trazo que acompaña a cada bloque de la conversación lo elige `RailStyle` (`crates/loomcoding-tui/src/render/rail.rs`:
`tree`, `dots`, `bar`, `box`, `tint`, `none`; ancho y símbolos por estilo, funciones puras). Se pinta en
`render/transcript.rs::draw_tree_gutter`; la caja de `box` vive en `render/rail_box.rs` y exige que el widget de
`loomcoding-cloth` mida una fila extra por mensaje de usuario (`Theme::rail_box`, `Layout::invalidate`). La preferencia se
guarda como `rail` en `ui.toml` (`prefs/rail.rs`, `app/rail_prefs.rs`) y es una fila de `/ui`. Un estilo nuevo se añade en
`RailStyle` y en `rail_cells`/`rail_width`, no con un comando suelto.

### `/model` y `/effort`

Atajos al flujo de ajustes del proveedor, sin abrir `/settings`: ambos terminan en `settings_save_requested`, que
recoge `run/mod.rs::send_provider_settings`. La lógica vive en `crates/loomcoding-tui/src/app/model_effort.rs`
(`set_model_now`, `open_model_picker_now`, `set_effort_now`, `open_effort_picker_now`); los selectores rápidos se
dibujan en `render/model_picker.rs` y `render/effort_picker.rs`, y sus teclas, en `run/model_picker_keys.rs` y
`run/effort_picker_keys.rs`. Los niveles de esfuerzo salen de `App::effort_options()` (catálogo y, si no
publica niveles, `[provider] efforts`). Un selector abierto debe excluirse del guard del pegado en `run/events.rs`.

## Quiero tocar el menú izquierdo del `Workspace`

Cabe señalar que el menú izquierdo solo se pinta con el layout `Workspace` (≥105 columnas).

- **Qué contiene** (logo, estado, `Actividad`, `Sesión`): `App::sidebar` en
  `crates/loomcoding-tui/src/app/composer.rs`; el dibujo, en
  `crates/loomcoding-cloth/src/widgets/sidebar.rs` y la marca en `widgets/loom_mark.rs`.
- **Diff de la sesión** (bloque `Ficheros`): el analizador y la obtención acotada de `git`, en
  `crates/loomcoding-tui/src/session_diff/` (`parse.rs`, `fetch.rs`); el estado y el hilo de `git`, en
  `app/session_diff_state.rs`; el dibujo, en `render/sidebar_diff.rs`; las teclas (`↑`/`↓`/`Enter` con el foco
  en el menú, `Shift+Tab` desde el compositor), en `run/sidebar_keys.rs`.
- **Efectos del logo**: el planificador y el catálogo de siete efectos, en
  `crates/loomcoding-cloth/src/widgets/logo_fx/` (`scheduler.rs`, `effects.rs`, `draw.rs`, `rng.rs`); el
  estado y las condiciones (solo `Workspace`, menú pintado, sin overlays, con color), en
  `crates/loomcoding-tui/src/app/logo_fx_state.rs`; el dibujo sobre el rectángulo del logo, en
  `render/logo_fx.rs`; el reloj, en `run/mod.rs` (`wants_clock` y el plazo `logo_fx_deadline`).
- **Preferencias**: `logo_fx` y `logo_fx_interval` (5 a 600 s, por defecto 20) en `ui.toml`, vía
  `crates/loomcoding-tui/src/prefs.rs`; el comando `/logofx [on|off|segundos]`, en `commands.rs` y `run/slash.rs`.
- Sin reloj en reposo: el bucle solo despierta en el próximo disparo; no añadas ticks periódicos.

## Quiero cambiar el flujo de permisos

1. Política y servicio: `crates/loomcoding-core/src/permission/mod.rs` (`PermissionService`,
   `PermissionRequest`, `Decision`).
2. Llamada desde la ejecución de tools: `crates/loomcoding-core/src/agent/dispatch.rs`.
3. Diálogo: `crates/loomcoding-tui/src/permission_flow.rs`,
   `crates/loomcoding-tui/src/widgets/permission.rs` y `crates/loomcoding-cloth/src/widgets/permission.rs`.

## Quiero tocar A2A (cliente o servidor)

1. Nombres del cable y tarjeta: `crates/loomcoding-a2a/src/card.rs`, `wire.rs`.
2. Cliente (consumir agentes remotos): `crates/loomcoding-a2a/src/client/` (`mod.rs` llama; `stream.rs` sigue el SSE; `factory.rs` construye clientes), `remote.rs`,
   `oauth.rs`, `security/`, `signing.rs`.
3. Servidor (servir Loomcoding): `crates/loomcoding-a2a/src/serve/` (`rpc/`, `auth.rs`, `host/`).
4. Streaming: `crates/loomcoding-a2a/src/sse.rs`.
5. Modelo interno de tarea: `crates/loomcoding-core/src/task.rs`, `task_events.rs`.
6. **No inventes estados**: contrasta con el proto normativo y rotula lo propio como propio.
7. Anota el avance en `docs/plans/2026-10-22-codeloom-a2a-roadmap.md`.

## Quiero añadir una columna persistida

1. Campo en la estructura (p. ej. `SubagentDescriptor` en `crates/loomcoding-core/src/session/mod.rs`).
2. Migración en `crates/loomcoding-core/src/session/sqlite/schema.rs`: añade la columna a la lista
   `(columna, "ALTER TABLE ... ADD COLUMN ...")` del bucle de migración (idempotente).
3. Lectura y escritura en el mismo archivo; un valor desconocido debe leerse sin romper.
4. Test de ida y vuelta con `SqliteStore`.

## Quiero tocar los límites del agente

1. Topes de pasos: las rondas de solo lectura no gastan `max_steps` y tienen su propio tope
   (`LimitsConfig::read_step_cap`, `4 × max_steps` por defecto). La clasificación vive en
   `crates/loomcoding-core/src/agent/stop.rs` (`is_read_round`, `is_write_tool`, `stop_reason_budgets`) y
   los contadores, en `agent/turn.rs` (`steps`, `read_steps`).
2. Pregunta al llegar al tope (`"Continue"` / `"Stop"`, hasta `MAX_STEP_EXTENSIONS`): `agent/step_limit.rs`.
   Los subagentes nunca preguntan (`budget_family.is_some()`); su fallo nombra el límite en
   `subagent/local_body.rs` (`step limit` o `read limit`).
3. Ajustes `[limits]` (`max_read_steps`, `max_parallel_tools`, `max_concurrent_requests`,
   `max_output_tokens`: tope de salida por petición, razonamiento incluido; 4096 por defecto y techo duro
   `MAX_OUTPUT_TOKENS_HARD`; una respuesta cortada por él termina el turno con `StopReason::OutputLimit`):
   `crates/loomcoding-core/src/config/agent_limits.rs`, con cargador propio; no se añaden campos a
   `LoomcodingConfig`. Un cero equivale al valor por defecto.
4. Paralelismo de herramientas: `execute_tools(.., max_parallel, ..)` en `agent/dispatch.rs`. De peticiones al
   proveedor: `provider/limited.rs` (`LimitedProvider`: semáforo y reintento ante `http 429`), aplicado en
   `loomcoding-cli/src/main.rs::limit_provider`.
5. Parada de emergencia (`Ctrl+X` fuera del panel de la cola): `agent/kill.rs` (`kill_everything`),
   `SessionCommand::KillAll` en `loomcoding-tui/src/run/` y el atendedor inmediato en
   `loomcoding-cli/src/server/client.rs`. Matar procesos: `tools/bash.rs::kill_tree` (todo el árbol).
6. Tests: `agent/tests/{steps,step_limit,kill}.rs`, `provider/limited.rs` (módulo `tests`).

## Quiero tocar el despertar del padre

1. Quien decide despertar al padre en la TUI: `App::take_wake` en `crates/loomcoding-tui/src/app/turn.rs`
   (causas: hijo `Completed`, `Failed` o que pide entrada; `Canceled` y `Rejected` no despiertan; presupuesto
   `MAX_AUTO_TURNS` en `crates/loomcoding-tui/src/app/child_types.rs`). El envío al servidor es `dispatch_wake` en
   `crates/loomcoding-tui/src/run/events.rs`.
2. Quien ejecuta el despertar: `AgentLoop::run_woken` en `crates/loomcoding-core/src/agent/entry.rs`, invocado por
   `AgentCommand::Wake` en `crates/loomcoding-cli/src/server/worker.rs`.
3. Sin TUI (`loomcoding --print`) el bucle es `AgentLoop::run_until_settled` en
   `crates/loomcoding-core/src/agent/settle.rs` (cotas `MAX_WAKE_TURNS` y `MAX_SETTLE_WAIT`, mismas causas que la TUI);
   lo llama `run_print_turn` en `crates/loomcoding-cli/src/server/daemon.rs`, que además cancela el turno si el
   cliente se desconecta.
4. Un turno cortado por un tope sin respuesta final se informa con `TurnOutcome::unfinished_report`
   (`crates/loomcoding-core/src/agent/mod.rs`; incluye `StopReason::OutputLimit`); `--print` lo devuelve como
   error mediante `print_text` en `daemon.rs`.

## Quiero tocar los jobs y su vigilancia (`watch`)

Cabe señalar que la vigilancia es información propia de Loomcoding: nunca se rotula como estado A2A.

1. Troceo, emparejado de líneas y cola de avisos (`WatchSpec`, `Watch`, `EventQueue`, `WatchBatch`, `WatchInfo`):
   `crates/loomcoding-core/src/job_watch.rs`. Ahí vive el decálogo de la vigilancia (patrón 256 B, línea 512 B,
   10 líneas por aviso, un aviso cada 5 s, `max_events` ≤ 100); no meta esa lógica en `job.rs`.
2. Ranura de vigilancia de cada job y registro: `crates/loomcoding-core/src/job.rs` (`WatchSlot`, `JobHandle::append`,
   `set_watch`, `take_watch_batches`, `watch_ready`). `JobSnapshot.watch` lleva el `WatchInfo` que ven la TUI y `job_list`.
3. Parámetro `watch` de `job_start`: `crates/loomcoding-core/src/tools/job_start.rs` (`watch_spec`, y `track_job`, que
   sondea la vigilancia en cada tick y publica `JobWatchReady`). Herramienta `job_watch` para un job ya lanzado:
   `crates/loomcoding-core/src/tools/job_watch.rs` (`JobWatchTool`), registrada en `tools/mod.rs`.
4. Evento `LiveEvent::JobWatchReady { session }` (`crates/loomcoding-core/src/event/mod.rs`): despierta al padre ocioso.
   Quien lo escucha en el núcleo es `agent/settle.rs`; en la TUI, `app/events.rs` (la causa «job watch» del turno
   despertado, con su presupuesto `MAX_AUTO_TURNS`, en `app/turn.rs`).
5. Aviso `[Job <id> watch] …` que llega a la conversación: `aviso_de_vigilancia` en
   `crates/loomcoding-core/src/agent/subagents.rs`, dentro de `flush_subagent_notices`. Entrega cada lote una sola vez,
   lo persiste y publica `SessionMessageAdded`; tras entregarlo republica `JobChanged` para que el recuento de un job
   ya terminado no se quede a cero. Pruebas: `agent/tests/watch_notices.rs`.
6. Clasificación de la herramienta: `job_watch` está en `WRITE_TOOLS` (`crates/loomcoding-core/src/agent/stop.rs`); una
   ronda con ella no cuenta como lectura y repetirla idéntica corta el turno como cualquier escritura.
7. Dónde se pinta el recuento (`watching: N alerts`, con `· +M suppressed` y `· limit reached`):
   - Formato único: `format_watch` en `crates/loomcoding-tui/src/rail/format.rs`.
   - Sección «Jobs» del rail: `push_jobs` en `crates/loomcoding-tui/src/rail/lists.rs`, que parte la nota por palabras.
   - Actividad del job en el transcript: `sync_job_activity` en `crates/loomcoding-tui/src/app/activity.rs`, que pasa
     la nota a `ActivityView.note`; el dibujo de esa línea está en `crates/loomcoding-cloth/src/state/activity_lines.rs`.
   - La tarjeta de agentes (`render/agents_card.rs`) lista subagentes, no jobs: no pinta la vigilancia.
8. Pruebas: `render/tests/vigilancia_de_jobs.rs` (rail y transcript) y el E2E PTY `crates/loomcoding-cli/tests/tui_pty_vigilancia.rs`
   (guion `mock_provider_watch` en `crates/loomcoding-e2e/src/mock.rs`; volcado en `docs/mockups/vigilancia-jobs-e2e.md`).

## Quiero tocar ODD (desarrollo orgánico)

Guía de uso: `docs/odd.md`. La lógica vive en `crates/loomcoding-workflows/src/odd/`; el núcleo solo ofrece puntos de
extensión genéricos y la TUI solo pinta.

1. Documento de feature (formato, cotas): `odd/doc.rs`, `odd/render.rs`; operaciones puras: `odd/ops.rs`; disco: `odd/store.rs`
   (`.loomcoding/odd/<feature>.md`). Herramienta `odd_track` (`odd/tool.rs`, `parent_only`); evidencia de una casilla:
   `odd/evidence.rs` (`VENTANA_EVIDENCIA` = 500 mensajes). Estado compartido y contadores: `odd/state.rs`, `odd/medicion.rs`.
2. Cierre bloqueante: punto de extensión `TurnCloseGate` en `crates/loomcoding-core/src/agent/close_gate.rs` (tope de 3
   reinyecciones, plazo de 2 s, pánico aislado; se consulta en `agent/turn.rs`) y política en `odd/gate.rs`.
3. Fase y avance: `ToolObserver` en `agent/tool_observer.rs` (publica `LiveEvent::FlowChanged`, tipo `FlowSummary` en
   `crates/loomcoding-core/src/flow.rs`); política en `odd/phase.rs` (`phase_of`) y `odd/observer.rs`
   (`resumen_de_flujo`: el formato de la etiqueta vive solo ahí).
4. Lista de tareas de la TUI: proyección en `odd/todos.rs` (`TodosChanged` tras cada operación que modifica el documento).
5. Sección de prompt: `PromptContributor` en `agent/prompt_contributor.rs` (2 000 B por sección) y texto estático en
   `odd/prompt.rs` (estático a propósito: un texto cambiante rompería la caché del prompt).
6. Solo el padre: `Tool::parent_only` y `ToolRegistry::without_parent_only` (`tools/mod.rs`, aplicado en
   `subagent/launcher.rs`). El cableado del padre está en `crates/loomcoding-cli/src/server/agent_build.rs`
   (`with_close_gate`, `with_tool_observer`, `with_prompt_contributor`); los hijos no los reciben.
7. Interruptor: `[flows.<kind>] enabled` (`crates/loomcoding-core/src/config/flows.rs`, `FlowsConfig`, genérico; el valor por defecto de cada modo lo publica `loomcoding_workflows::flow_modes()`; se aplica al arrancar el daemon);
   TUI (genérica, sin nombrar ODD): lista de modos `FlowMode` y etiqueta en `app/flow_label.rs` y `render/chrome.rs`, comando `/flow` en `app/flow_command.rs`.
8. Pruebas: `odd/tests/` (documento, disco, evidencia, cierre, fase, prompt, lista, medición); garantía A2A de no herencia
   en `crates/loomcoding-core/src/subagent/tests/no_herencia.rs`; E2E PTY `crates/loomcoding-cli/tests/tui_pty_odd.rs`
   (volcado en `docs/mockups/odd-e2e.md`).

## Quiero tocar el contexto estructural

1. Índice de símbolos de Rust (definiciones y llamadas, con `tree-sitter` y `context/tags.scm`):
   `SymbolIndex::build` en `crates/loomcoding-core/src/context/index.rs`. El analizador y la consulta se crean
   una vez; cada método se indexa una sola vez (`tags.scm` lo registra también como función).
2. Caché por raíz con huella del árbol (nº de ficheros, última fecha, tamaño): `index_for` en
   `context/cache.rs`; una edición durante el turno reconstruye el índice.
3. Paquete acotado de un símbolo: `compose` y `PackOptions` en `context/pack.rs` (`MAX_CHARS_HARD`, usos,
   pruebas, sección de `docs/CODE-MAP.md` que lo nombra).
4. Herramienta del modelo: `ContextTool` en `crates/loomcoding-core/src/tools/context.rs`, registrada en
   `project_registry` (`tools/mod.rs`). Pendiente del plan `docs/plans/2026-11-05-loomcoding-contexto-estructural.md`:
   ordenación por PageRank (hoy: más usado primero), caché en disco y orientación previa en la oficina.

## Quiero tocar la compresión de salida

Guía de uso: `docs/output-filter.md`. Spec: `docs/specs/2026-11-06-loomcoding-compresion-de-salida.md`. Solo
herramientas `bash` y `job_output`; nunca subagentes ni A2A.

1. Enganche único en el bucle: `contenido_para_modelo` en `crates/loomcoding-core/src/output_filter/enviar.rs`,
   llamado desde `agent/turn.rs`. Devuelve `Enviado` (contenido, ahorro y reglas desactivadas) y el turno publica
   `LiveEvent::OutputFiltered` y, una sola vez por regla, un `RuntimeNotice`.
2. Filtro puro: `apply` y `filtrar` en `output_filter/mod.rs` (`filtrar` admite el veto de reglas desactivadas); la lista
   de reglas por comando es `REGLAS` en el mismo fichero. Reglas: `general.rs` (generales), `cargo.rs`, `git.rs`,
   `packages.rs`. Cabecera R1: `header.rs`.
3. Original y `raw`: `ToolSpill` (`tools/spill.rs`); `raw` lo leen `bash` y `job_output` antes de llegar al filtro.
4. Medición y desactivación: `output_filter/stats.rs` (`USOS_MINIMOS`, `UMBRAL_REPETICION_PCT`, `evalua`,
   `aviso_desactivacion` y `global()`, el estado del proceso). La huella del comando es un número, nunca el texto.
5. Evento: `LiveEvent::OutputFiltered` en `event/mod.rs` (`is_child_relevant` falso). La TUI lo ignora en
   `crates/loomcoding-tui/src/app/events.rs`.
6. Configuración: `FilterConfig` en `output_filter/mod.rs` (`[output_filter] enabled`, apagado por defecto;
   `LOOMCODING_NO_FILTER=1` lo apaga). Se carga en `crates/loomcoding-cli/src/server/agent_build.rs`.
7. Pruebas: `output_filter/tests.rs`, `cargo_tests.rs`, `git_tests.rs`, `packages_tests.rs`, `stats_tests.rs`; enganche
   y evento en `agent/tests/output_filter.rs`. Capturas reales en `output_filter/fixtures/`.

## Quiero añadir un umbral o ajuste en `[agents]`

1. Campo `Option<...>` en `AgentsConfig` (`crates/loomcoding-core/src/config/mod.rs`); la clave del
   TOML **es el nombre del campo**.
2. Constantes `DEFAULT_*`/`MIN_*`/`MAX_*` y un método que acote con `clamp` (patrón de
   `active_subagents` y `stall_threshold_ms`).
3. Test con `AgentsConfig::from_toml`.
