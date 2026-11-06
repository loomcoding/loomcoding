# Desarrollo orgánico (ODD) — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla (rojo visible), código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`), un commit por
> tarea. Cargo desde PowerShell. Trabajo en una rama propia (`odd`).

**Spec:** `docs/specs/2026-11-07-loomcoding-odd-desarrollo-organico.md`.

## Decisiones de la persona usuaria (2026-11-07)

- **Activo por defecto**, con interruptor para apagarlo (`[odd] enabled = true`). La medición del final de la spec pasa de
  «condición para activarlo» a «seguimiento tras activarlo»: se registran los números y, si salen mal, se desactiva.
- **Cuando se active un modo o flujo debe reflejarse en algún sitio visible.** Pasa a ser una tarea propia (T5) y vale
  también para `development_workflow` y `gauntlet`, que hoy no se ven en la cabecera.
- **Evidencia de una casilla** (explicada en la tarea T2): decisión provisional de quien redacta, a revisar.
- **Sección de prompt solo para el padre**, no para los subagentes (provisional).

## Restricciones globales

- **Ubicación (2026-11-07, tras una objeción de la persona usuaria):** ODD es un flujo, así que su lógica vive en el
  crate `crates/loomcoding-workflows` (módulo `odd/`), que depende de `loomcoding-core` y nunca al revés, como fija
  `2026-11-05-loomcoding-extraer-flujos-del-core.md` (el núcleo no nombra ningún flujo). Lo que ODD necesita del núcleo
  se pide como **punto de extensión genérico, sin la palabra «odd»**: política de cierre de turno (T3), contribución a
  la sección de prompt (T6) y evento genérico de flujo con `label` (T4/T5; es el `FlowChanged` de esa spec, que se
  introduce aquí con la misma forma para que la extracción del Gauntlet no lo rehaga). `loomcoding-cli` compone: llama a
  `loomcoding_workflows::register`.
- Núcleo sin `ratatui`; ficheros nuevos ≤ 400 líneas.
- Todo acotado (decálogo): bytes del documento, número de tareas, líneas del registro, reintentos del cierre, bytes de la
  sección de prompt.
- Las fases y el modo son **etiquetas propias de Loomcoding, no estados A2A**; así se rotulan en la TUI.
- Sin red real en las pruebas; tiempos con reloj simulado.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Documento enorme, vacío, con formato roto o editado a mano a medio trabajo: nunca bloquea el cierre ni provoca pánico.
- Dos features a la vez en el mismo proyecto: cada documento es independiente y el cierre mira solo el de la sesión.
- Sesión retomada: el documento se lee entero y se reconcilia con el código antes de seguir.
- El usuario cancela con el cierre bloqueando: la cancelación gana siempre.
- Presupuesto de turnos automáticos agotado con tareas pendientes: se avisa una vez y se deja de insistir.

## Tarea 1 — Documento de feature

**Ficheros:** crate nuevo `crates/loomcoding-workflows` (miembro del workspace, con su README); en él `src/odd/doc.rs`
(`FeatureDoc`, `Task { id, text, status, evidence }`, `TaskStatus::{Pending, Done, Blocked}`), `src/odd/render.rs`
(Markdown ↔ estructura), `src/odd/mod.rs`, `src/lib.rs`. El núcleo no se toca.
- Formato: Markdown con objetivo, motivo, alcance, restricciones, tareas (`- [ ] T1 texto` / `- [x] T1 texto — evidencia:
  …`), registro y siguiente paso. Identificadores estables `T<n>`.
- Cotas: documento ≤ 32 KiB, ≤ 60 tareas, texto de tarea ≤ 300 B, registro conserva las últimas 100 líneas.
- Firmas: `FeatureDoc::parse(&str) -> Result<FeatureDoc, OddError>`, `FeatureDoc::to_markdown(&self) -> String`,
  `FeatureDoc::pending(&self) -> Vec<&Task>`.
- [ ] Pruebas: ida y vuelta sin pérdida; documento roto → error claro, no pánico; exceso de tamaño o de tareas → se
  rechaza o se recorta con aviso; identificadores duplicados; casilla marcada a mano en el fichero se respeta.

## Tarea 2 — Herramienta `odd_track` y evidencia

**Ficheros:** en `loomcoding-workflows`: `src/odd/tool.rs` (herramienta `odd_track`, `Exclusive`, implementa el `Tool` del
núcleo), `src/odd/store.rs` (ruta del documento y localizador guardado en la sesión) y `pub fn register(&mut
ToolRegistry)` en `lib.rs`; en `loomcoding-cli`, la llamada a esa función al componer el registro. El núcleo solo gana,
si hace falta, un acceso genérico al historial de la sesión para comprobar la evidencia.
- Operaciones: `create {feature, objective, tasks[]}`, `done {id, evidence}`, `reopen {id, reason}`, `block {id, reason}`,
  `log {text}`, `read`. El documento vive en `.loomcoding/odd/<feature>.md` del proyecto; el nombre se normaliza (sin
  rutas, sin `..`).
- **Evidencia de una casilla (decisión provisional):** marcar `done` exige `evidence`: una cita corta de lo observado
  (por ejemplo «cargo test: 12 passed») **y** el identificador de una llamada de herramienta de esta sesión que la
  respalde (`bash`, un test, un `diff`). El runtime comprueba que esa llamada **existe** en el historial y que el texto
  cabe en 500 bytes; **no** comprueba que el modelo diga la verdad. Sin evidencia válida, la casilla no se marca y la
  herramienta explica qué falta. Es el equivalente de «enseña el recibo».
- **Heredado de la T1:** el formato separa texto y evidencia por ` — evidencia:` (y el motivo por ` — bloqueada:`), así
  que un texto de tarea que contenga esos separadores no hace ida y vuelta exacta. `odd_track` debe **rechazar** (o
  neutralizar) los separadores en cualquier texto que venga del modelo, con su prueba.
- [ ] Pruebas: `done` sin evidencia, con llamada inexistente o con texto desmesurado → rechazo con motivo; `done` válido
  → documento actualizado y avisado en el transcript; nombre de feature con `..`/rutas → rechazo; `create` duplicado;
  `read` acotado; dos sesiones no se pisan.

## Tarea 3 — Cierre bloqueante en el runtime

**Ficheros:** en el núcleo, un **punto de extensión genérico** sin la palabra «odd»: `trait TurnCloseGate` (consultado
al cerrar el turno del padre; devuelve cerrar o reinyectar un texto) cableado en `agent/settle.rs` y `agent/turn.rs`;
en `loomcoding-workflows`, `src/odd/gate.rs` (`fn closing_gate(doc: Option<&FeatureDoc>, state: &GateState) ->
GateDecision`) que lo implementa.
- Si hay tareas pendientes, el cierre devuelve el control al modelo con la lista de pendientes. **Reutiliza** el
  presupuesto de turnos automáticos (`MAX_AUTO_TURNS` en la TUI, `MAX_WAKE_TURNS` sin ella): al agotarse, avisa una vez y
  cierra. Se levanta con la cancelación del usuario o con un resultado «Necesita tu decisión» (marca de bloqueo en el
  documento).
- Un documento ilegible o ausente **no bloquea** (se avisa y se cierra).
- [ ] Pruebas: pendiente → no cierra y reinyecta la lista; sin pendientes → cierra; presupuesto agotado → avisa y cierra;
  cancelación gana; documento roto → no bloquea; todo `Blocked` con motivo → cierra con «Necesita tu decisión»; `--print`
  respeta `MAX_WAKE_TURNS`.

## Tarea 4 — Fase inferida de las herramientas

**Ficheros:** en `loomcoding-workflows`, `src/odd/phase.rs` (`enum OddPhase { Exploring, Implementing, Checking,
Deciding, Planning, Idle }`, `fn phase_of(tool: &str, args: &Value) -> Option<OddPhase>`); en el núcleo, el evento
genérico `LiveEvent::FlowChanged { summary: FlowSummary, actor, detail }` con la forma de la spec de extracción
(`FlowSummary { kind, id, phase, round, max_rounds, terminal, label }`), no relevante para hijos. Si al llegar aquí la
extracción ya lo hubiera introducido, se reutiliza tal cual.
- Lectura = explorando; escritura = implementando; `bash` con test/build/lint conocido = comprobando; pregunta =
  decidiendo; `odd_track` = planificando. Función pura, sin llamar al modelo; el comando ambiguo deja la fase como está.
  Se publica solo al cambiar.
- [ ] Pruebas: tabla de herramientas → fase; `bash` ambiguo no cambia; no se publica dos veces la misma; el evento no se
  reenvía a los hijos.

## Tarea 5 — El modo y el flujo activo se ven

**Ficheros:** `crates/loomcoding-tui/src/render/chrome.rs` (cabecera), `app/events.rs`, `prefs`/configuración
(`[odd] enabled`), comando `/odd` (ver el estado y apagar/encender para la sesión). La TUI pinta el `label` del evento
genérico `FlowChanged` tal cual, sin conocer ODD (regla de la spec de extracción: la TUI no depende de
`loomcoding-workflows`).
- La cabecera muestra una etiqueta del **modo/flujo activo** y, en ODD, la fase: `ODD · exploring`, `ODD · implementing
  (T2/5)`. Cuando corre `development_workflow` o `gauntlet`, la etiqueta pasa a ese flujo y su fase propia
  (`gauntlet · ronda 2/3`). Si no hay modo, no se pinta nada. Se rotula como «propio de Loom», no como estado A2A.
- Con la cabecera estrecha, la etiqueta se acorta antes que desaparecer.
- [ ] Pruebas de render: ODD activo con y sin fase; flujo pesado activo; ancho estrecho; ODD apagado → sin etiqueta;
  `/odd` informa y conmuta; volcado e2e con perfil aislado (`LOOMCODING_HOME` temporal) y mock con guion.

## Tarea 6 — Sección de prompt y clasificación

**Ficheros:** en el núcleo, un punto de extensión genérico (`trait PromptContributor`, que `system_prompt` en
`agent/entry.rs` consulta, solo para el padre y con tope de bytes por sección); en `loomcoding-workflows`,
`src/odd/prompt.rs` (texto de la sección, con tope de 2 000 bytes) que lo implementa.
- Solo para el padre (quien puede delegar). Contenido: autorizar y explorar primero, la prueba del resumen, crear el
  documento solo si es grande, casillas con evidencia, cierre. El texto se cuenta y se prueba contra el tope.
- [ ] Pruebas: el padre la recibe; un subagente no; apagado con `[odd] enabled = false` → no aparece; respeta el tope;
  el prompt total sigue acotado.

## Tarea 7 — Proyección a la lista de tareas de la TUI

**Ficheros:** `odd/todos.rs`, publicación de `LiveEvent::TodosChanged` desde los cambios del documento.
- La lista `TodosChanged` es proyección del documento, no otra fuente: al cambiar una casilla se republica.
- [ ] Pruebas: marcar/reabrir una tarea actualiza la lista; sin documento, la lista queda como estaba; editar la lista
  desde otra vía no desincroniza el documento (o se documenta que no se puede).

## Tarea 8 — Medición, documentación y comprobación

- Contadores en el bus (sin datos personales): documentos creados, tareas marcadas, cierres bloqueados, cierres que
  terminaron por techo de turnos, rechazos de evidencia.
- `docs/INDEX.md`, `docs/CODE-MAP.md` (sección ODD), `CHANGELOG.md`; guía breve de uso en `docs/`; volcado e2e con perfil
  aislado: una petición grande crea el documento, el cierre bloquea con una tarea pendiente, la fase y la etiqueta salen
  en pantalla.
- [ ] Revisión final de las cinco preguntas abiertas de la spec y actualización de su estado.
