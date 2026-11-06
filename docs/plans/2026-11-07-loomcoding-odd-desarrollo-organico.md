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

- Núcleo sin `ratatui`; módulo nuevo `crates/loomcoding-core/src/odd/`; ficheros nuevos ≤ 400 líneas.
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

**Ficheros:** nuevo `odd/doc.rs` (`FeatureDoc`, `Task { id, text, status, evidence }`, `TaskStatus::{Pending, Done,
Blocked}`), `odd/render.rs` (Markdown ↔ estructura), `odd/mod.rs`, `lib.rs`.
- Formato: Markdown con objetivo, motivo, alcance, restricciones, tareas (`- [ ] T1 texto` / `- [x] T1 texto — evidencia:
  …`), registro y siguiente paso. Identificadores estables `T<n>`.
- Cotas: documento ≤ 32 KiB, ≤ 60 tareas, texto de tarea ≤ 300 B, registro conserva las últimas 100 líneas.
- Firmas: `FeatureDoc::parse(&str) -> Result<FeatureDoc, OddError>`, `FeatureDoc::to_markdown(&self) -> String`,
  `FeatureDoc::pending(&self) -> Vec<&Task>`.
- [ ] Pruebas: ida y vuelta sin pérdida; documento roto → error claro, no pánico; exceso de tamaño o de tareas → se
  rechaza o se recorta con aviso; identificadores duplicados; casilla marcada a mano en el fichero se respeta.

## Tarea 2 — Herramienta `odd_track` y evidencia

**Ficheros:** `tools/odd_track.rs` (`Exclusive`), registro junto a las demás herramientas; `odd/store.rs` (ruta del
documento y localizador guardado en la sesión).
- Operaciones: `create {feature, objective, tasks[]}`, `done {id, evidence}`, `reopen {id, reason}`, `block {id, reason}`,
  `log {text}`, `read`. El documento vive en `.loomcoding/odd/<feature>.md` del proyecto; el nombre se normaliza (sin
  rutas, sin `..`).
- **Evidencia de una casilla (decisión provisional):** marcar `done` exige `evidence`: una cita corta de lo observado
  (por ejemplo «cargo test: 12 passed») **y** el identificador de una llamada de herramienta de esta sesión que la
  respalde (`bash`, un test, un `diff`). El runtime comprueba que esa llamada **existe** en el historial y que el texto
  cabe en 500 bytes; **no** comprueba que el modelo diga la verdad. Sin evidencia válida, la casilla no se marca y la
  herramienta explica qué falta. Es el equivalente de «enseña el recibo».
- [ ] Pruebas: `done` sin evidencia, con llamada inexistente o con texto desmesurado → rechazo con motivo; `done` válido
  → documento actualizado y avisado en el transcript; nombre de feature con `..`/rutas → rechazo; `create` duplicado;
  `read` acotado; dos sesiones no se pisan.

## Tarea 3 — Cierre bloqueante en el runtime

**Ficheros:** `agent/settle.rs` y `agent/turn.rs` (punto de cierre del turno del padre), `odd/gate.rs`
(`fn closing_gate(doc: Option<&FeatureDoc>, state: &GateState) -> GateDecision`).
- Si hay tareas pendientes, el cierre devuelve el control al modelo con la lista de pendientes. **Reutiliza** el
  presupuesto de turnos automáticos (`MAX_AUTO_TURNS` en la TUI, `MAX_WAKE_TURNS` sin ella): al agotarse, avisa una vez y
  cierra. Se levanta con la cancelación del usuario o con un resultado «Necesita tu decisión» (marca de bloqueo en el
  documento).
- Un documento ilegible o ausente **no bloquea** (se avisa y se cierra).
- [ ] Pruebas: pendiente → no cierra y reinyecta la lista; sin pendientes → cierra; presupuesto agotado → avisa y cierra;
  cancelación gana; documento roto → no bloquea; todo `Blocked` con motivo → cierra con «Necesita tu decisión»; `--print`
  respeta `MAX_WAKE_TURNS`.

## Tarea 4 — Fase inferida de las herramientas

**Ficheros:** `odd/phase.rs` (`enum OddPhase { Exploring, Implementing, Checking, Deciding, Planning, Idle }`,
`fn phase_of(tool: &str, args: &Value) -> Option<OddPhase>`), `event/mod.rs` (`LiveEvent::OddPhaseChanged { session,
phase }`, no relevante para hijos).
- Lectura = explorando; escritura = implementando; `bash` con test/build/lint conocido = comprobando; pregunta =
  decidiendo; `odd_track` = planificando. Función pura, sin llamar al modelo; el comando ambiguo deja la fase como está.
  Se publica solo al cambiar.
- [ ] Pruebas: tabla de herramientas → fase; `bash` ambiguo no cambia; no se publica dos veces la misma; el evento no se
  reenvía a los hijos.

## Tarea 5 — El modo y el flujo activo se ven

**Ficheros:** `crates/loomcoding-tui/src/render/chrome.rs` (cabecera), `app/events.rs`, `prefs`/configuración
(`[odd] enabled`), comando `/odd` (ver el estado y apagar/encender para la sesión).
- La cabecera muestra una etiqueta del **modo/flujo activo** y, en ODD, la fase: `ODD · exploring`, `ODD · implementing
  (T2/5)`. Cuando corre `development_workflow` o `gauntlet`, la etiqueta pasa a ese flujo y su fase propia
  (`gauntlet · ronda 2/3`). Si no hay modo, no se pinta nada. Se rotula como «propio de Loom», no como estado A2A.
- Con la cabecera estrecha, la etiqueta se acorta antes que desaparecer.
- [ ] Pruebas de render: ODD activo con y sin fase; flujo pesado activo; ancho estrecho; ODD apagado → sin etiqueta;
  `/odd` informa y conmuta; volcado e2e con perfil aislado (`LOOMCODING_HOME` temporal) y mock con guion.

## Tarea 6 — Sección de prompt y clasificación

**Ficheros:** `agent/entry.rs` (`system_prompt`), `odd/prompt.rs` (texto de la sección, con tope de 2 000 bytes).
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
