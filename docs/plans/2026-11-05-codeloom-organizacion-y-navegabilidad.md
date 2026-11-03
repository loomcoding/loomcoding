# Plan: organización y navegabilidad del código

- **Fecha:** 2026-11-05
- **Spec:** `docs/specs/2026-11-05-codeloom-organizacion-y-navegabilidad.md`
- **Relacionado:** `docs/plans/2026-11-04-codeloom-estados-de-subagente-y-composer-compartido.md`
- **Principio de orden:** primero lo de riesgo cero y beneficio inmediato (documentar),
  después los guardarraíles, y al final el refactor de monolitos de menor a mayor riesgo.

## Decisiones ya tomadas (spec, sección "Decisiones")

Techo duro 1.000 líneas · guardarraíl = test de Rust con lista blanca decreciente ·
README en `crates/<crate>/README.md` · `docs/INDEX.md` a mano + test de cobertura ·
`app.rs`/`run.rs` tras la Fase 6 del plan de estados.

## Global Constraints

Los cuatro checks en cada commit (en esta máquina, vía
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts\checks.ps1 -TargetDir .verify-target`,
porque `cargo test` directo da `Acceso denegado` con daemons vivos):

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

Reglas que este plan no puede romper:

- `codeloom-core` **no** depende de `ratatui` ni `crossterm`.
- Errores de librería con `thiserror`; el binario puede usar `anyhow`.
- Nada sin cota (decálogo de contexto).
- Sin red real en tests.
- **Un commit que mueve código no cambia comportamiento.** Nunca mezclar movimiento
  con lógica: si algo se rompe, hay que saber cuál de los dos fue.
- Los specs y READMEs son **español**; el código sigue el idioma del archivo que toca.
- **Cifras sin hardcodear:** los conteos (documentos, líneas, tests) los calcula un
  comando o un test; en los documentos se citan "a fecha de" y no como contrato.

---

## Fase 0 — Navegación (riesgo cero, beneficio inmediato)

Ningún archivo de `crates/` se toca (salvo los `README.md` nuevos). Al terminar, un
agente nuevo puede orientarse leyendo cinco documentos. Un commit por paso.

### Paso 0.1 — `docs/ARCHITECTURE.md` (HECHO 2026-11-05)

**Corrección de premisa:** el spec daba por inexistente `docs/ARCHITECTURE.md` y todo
índice. En realidad ya existían `docs/ARCHITECTURE.md` (con rutas desfasadas y la flecha
core→a2a al revés), `docs/README.md` (índice de carpetas) y `docs/INDEX.md`
(estado cotejado de los specs, a 2026-10-30), y `docs/specs|plans/hechos/` separan lo
terminado. El paso se ejecutó como **reescritura** del existente, no como creación.
Consecuencia para el paso 0.5: ver allí.

- Tabla de los 8 crates: responsabilidad · entrada · qué **no** va aquí.
- Diagrama de dependencias permitidas y prohibidas (core sin TUI), sacado de los
  `Cargo.toml` reales.
- Flujo end-to-end: mensaje → sesión → provider → tools → `LiveEvent` → TUI/servidor,
  con el archivo real en cada frontera.
- Los cinco subsistemas: `Provider`, registry de tools, bus de eventos,
  `SessionStore`, orquestador de subagentes.

**Aceptación:** un agente responde "¿dónde entra un mensaje del usuario?" sin abrir
código, solo con el documento; todas las rutas citadas existen (comprobar con `ls`).

### Paso 0.2 — `README.md` por crate (8 archivos, en `crates/<crate>/`) (HECHO 2026-11-05)

Plantilla fija del spec: responsabilidad · no va aquí · tipos clave · puntos de
entrada · cómo se testea · trampas. Una pantalla cada uno.

**Aceptación:** los 8 existen con las mismas secciones en el mismo orden; los tipos y
rutas citados existen.

### Paso 0.3 — `docs/CODE-MAP.md` (HECHO 2026-11-05)

Índice orientado a tareas. Entradas mínimas obligatorias: añadir una tool · un
provider · un evento al bus · una fase de subagente (`subagent_phase.rs`, y el
watchdog en `subagent_watchdog.rs` si ya existe) · un widget de TUI · un comando de
composer · cambiar el flujo de permisos · tocar A2A (cliente y servidor) · añadir una
columna persistida.

**Aceptación:** cada entrada nombra archivos exactos con ruta y **todas existen**
(verificado con un `ls`/`grep` por ruta antes de commitear; no inventar rutas).

### Paso 0.4 — `docs/INVARIANTS.md` (HECHO 2026-11-05)

Tabla invariante → **comando exacto o procedimiento** que lo verifica.

**Aceptación:** cada fila tiene una comprobación ejecutable o un procedimiento
concreto con ruta; se ejecuta cada comando una vez para confirmar que funciona.

### Paso 0.5 — `docs/INDEX.md` (HECHO 2026-11-05, opción a)

Decisión: `inventario.md` se renombra a `docs/INDEX.md` (con `git mv`; enlaces
actualizados) y se amplía: cabecera con la regla «consulta el índice primero», estados
admitidos y una tabla con la tanda del 2026-11-01 al 11-05, que no estaba. Es un índice
**temático** (agrupa por tema, no una fila por fichero) y los documentos en
`specs|plans/hechos/` constan como hechos.

**Aceptación:** la tanda reciente está cubierta y marcada; los documentos sin línea
`Estado:` figuran como «sin estado declarado», sin inventar uno. Pendiente aparte: cotejar
con el código los seis documentos que aún dicen «sin cotejar».

### Paso 0.6 — `AGENTS.md` como puerta de entrada (HECHO 2026-11-05)

Añadir al principio la sección "Mapa de lectura" con los cinco punteros, y la regla
"un cambio que invalida `ARCHITECTURE.md`/`CODE-MAP.md` los actualiza en el mismo
commit". **No duplicar** contenido: solo enlazar.

**Aceptación:** `AGENTS.md` sigue siendo corto y no repite lo de `ARCHITECTURE.md`.

---

## Fase 1 — Guardarraíles

Tres checks como tests de Rust en el workspace (decisión cerrada: corren con
`cargo test`). Crate/ubicación: un test de integración en `crates/codeloom-e2e/tests/`
(ya existe el crate y no es parte del motor), `guardarrailes.rs`.

### Paso 1.1 — Check de tamaño (HECHO 2026-11-05)

Enumera `crates/*/src/**/*.rs` y falla si alguno supera **1.000 líneas** y no está en
la lista blanca. Lista blanca inicial: los archivos que hoy superan el techo (23 en `crates/*/src`, medidos el
2026-11-05; el «25» del spec contaba otros ficheros). Cada entrada lleva un tope redondeado a la
centena superior, cada uno con una línea de justificación. La lista solo puede decrecer: el
test falla también si una entrada de la lista ya no supera el techo (obliga a retirarla).

**Aceptación:** pasa en el estado actual; falla si se añade un archivo nuevo de más de
1.000 líneas; falla si queda una entrada obsoleta en la lista.

### Paso 1.2 — Check de la regla de dependencias (HECHO 2026-11-05)

`codeloom-core/Cargo.toml` no declara `ratatui`/`crossterm` y no hay `use ratatui`/
`use crossterm` en `crates/codeloom-core/src`.

**Aceptación:** falla si alguien añade la dependencia o el import.

### Paso 1.3 — Check de cobertura del INDEX (HECHO 2026-11-05)

Como el índice es temático, el test no exige una fila por fichero: falla si algún
`docs/specs/*.md` o `docs/plans/*.md` de primer nivel con fecha **posterior al corte**
(2026-11-05) no aparece por su nombre corto en `docs/INDEX.md`. Los anteriores quedan
cubiertos por el propio índice y por `hechos/`.

**Aceptación:** pasa tras el paso 0.5; falla al añadir un spec nuevo sin indexarlo.

---

## Fase 2 — Refactor de `subagent.rs` (7.510 líneas)

Va primero porque el trabajo de A2A de `2026-11-04` crece alrededor del archivo.
`subagent_phase.rs` ya existe (Fase 1 de estados) y `subagent_watchdog.rs` es un módulo
nuevo del plan del watchdog: **no hay nada que mover para ellos**; este refactor solo
extrae lo que sigue dentro de `subagent.rs`.

### Paso 2.1 — Mapa antes de mover (HECHO 2026-11-05)

Medido sobre `crates/codeloom-core/src/subagent.rs` (7.553 líneas). **Hallazgo clave: el 71 %
del archivo son tests** (líneas 2141-7553, 110 tests); el código de producción son 2.140.

| Líneas | Contenido | Destino propuesto |
|---|---|---|
| 1-255 | `SubagentRequest`, `trait SubagentLauncher`, profundidad (`root_levels`, `child_levels`, `depth_clipped`, `can_delegate`), `compose_child_system`, struct `InProcessSubagentLauncher` | `subagent/mod.rs` (con los `pub use`) |
| 256-350 | `DiscardOnExit`, constantes de espera, `TaskTracker` | `subagent/tracker.rs` |
| 351-1231 | `impl InProcessSubagentLauncher` (constructores `with_*`, `descriptor_of`, resolución de agente) | `subagent/launcher.rs` |
| 1232-1717 | Espejo de remotos A2A: `EjecucionRemota`, `Espejo`, `Entrada`, `seguir_remoto` | `subagent/remote.rs` |
| 1718-2140 | Helpers (`resumen_de_permiso`, `validar_peticion`, `prompt_de_seguimiento`, `elegir_skills`) e `impl SubagentLauncher` | `subagent/launch.rs` |
| 2141-7553 | `mod tests` | `subagent/tests/` (ver abajo) |

Tests, por tema (los proveedores y lanzadores de prueba están intercalados y pasan a
`tests/support.rs`): profundidad y persona (2149-2230) · ciclo de vida y avisos (2234-3040) ·
reenvío de eventos y mensajes del hijo (3039-3500) · aprobación, capacidad y cancelación
(3611-4300) · enrutado y cancelación fina (4251-4580) · arranque con registro y perfiles
(4606-5240) · seguimiento (5243-5530) · espejo de remotos (5531-6700, ~1.170 líneas: se parte en
espejo y streaming) · recursión, skills y presupuesto familiar (6700-7553).

Orden de commits (cada uno, movimiento puro y los cuatro checks en verde):
1. `git mv subagent.rs subagent/mod.rs` (solo renombrado; la entrada de la lista blanca del
   guardarraíl cambia de ruta con el mismo tope).
2. Tests fuera, por temas, con `support.rs` común. Nada de un `tests.rs` único de 5.400 líneas:
   crearía un archivo nuevo sobre el techo.
3. `remote.rs`, 4. `tracker.rs`, 5. `launcher.rs` y `launch.rs`.
6. Retirar la entrada de `subagent/mod.rs` de la lista blanca cuando baje de 1.000.

**Avance (2026-11-05):** hechos los commits 1 y 2 en uno (`subagent.rs` → `subagent/mod.rs`
y los 110 tests repartidos en `subagent/tests/` en diez temas más `support.rs`; ninguno pasa de
924 líneas; mismos 741 tests). `subagent/mod.rs` queda en 2.142 líneas de producción: el corte
por responsabilidad de los commits 3-5 se replantea por arquitectura (ver nota del paso 2.2).

Riesgos: la visibilidad. Lo que hoy es privado del módulo (`struct Espejo`, constantes) pasa a
`pub(super)`; los tests acceden a internos con `use super::*`, y hay que conservar esa vía.

**Aceptación:** corte escrito y revisado antes del paso 2.2.

### Paso 2.2 — Convertir `subagent.rs` en `subagent/`

**Corte por arquitectura (revisión del 2026-11-05).** Los 2.142 líneas de producción de
`subagent/mod.rs` no se parten «por tamaño»: se parten por **fase del ciclo de vida** de un
subagente, que es como se razona sobre ellos. Hallazgo: `arrancar_con` mide 464 líneas por sí
sola (un único método), y el espejo de remotos otras ~490.

| Fichero | Responsabilidad (fase) | Contenido actual |
|---|---|---|
| `mod.rs` | Contrato y re-exports | `SubagentRequest`, `trait SubagentLauncher`, `pub use` |
| `depth.rs` | Política de recursión | `root_levels`, `child_levels`, `depth_clipped`, `can_delegate` |
| `profile_prompt.rs` | Composición del prompt del hijo | `compose_child_system`, topes de persona, `elegir_skills` |
| `tracker.rs` | Dueño del estado de la tarea | `TaskTracker`, `DiscardOnExit` |
| `launcher.rs` | Estado y construcción del lanzador | struct `InProcessSubagentLauncher`, `new`, `with_*`, `anidado` |
| `admission.rs` | Antes de lanzar: ¿se puede y se quiere? | `validar_peticion`, `resolver_agente`, `route`, `pedir_aprobacion`, `registrar_rechazo`, `mensaje_capacidad` |
| `local_run.rs` | Lanzar y ejecutar un hijo local | `arrancar`, `arrancar_con` (464 líneas: se trocea en pasos con nombre) |
| `remote/` | Hijos que son agentes A2A remotos | `EjecucionRemota`, `Espejo`, `seguir_remoto` (`mod`, `mirror`, `follow`) |
| `control.rs` | Interacción con hijos vivos | `control`, `cancel`, `deliver_notices`, `pending_input_notices`, `start_follow_up`, `prompt_de_seguimiento` |

Mecánica: Rust permite varios `impl InProcessSubagentLauncher` en módulos hijos, y un hijo ve lo
privado del padre, así que casi no cambia la visibilidad (solo `pub(super)` en los métodos que
se llaman entre ficheros). Primero se mueve todo tal cual, un commit por fichero; trocear
`arrancar_con` es un commit **aparte** (es refactor de lógica, no movimiento) y con tests en verde.

**Avance (2026-11-05):** hechos los movimientos de producción: `subagent/mod.rs` baja de 7.553 a 182
líneas, con `depth`, `profile_prompt`, `tracker`, `remote`, `launcher`, `admission`, `local_run` y
`control`; el mayor es `launcher.rs` (619). La entrada de `subagent/mod.rs` sale de la lista blanca.
`arrancar_con` se trocea en pasos con nombre (`local_run.rs` 444, `local_body.rs` 166,
`local_watch.rs` 104). Los cuerpos del `impl SubagentLauncher` con lógica (`route`,
`start_follow_up`, `control`, `cancel`, `deliver_notices`, `pending_input_notices`) pasan a métodos
propios en `admission.rs` y `control.rs`, y el trait solo reenvía. `subagent_phase`,
`subagent_watchdog` y `subagent_input` viven ahora en `subagent/{phase,watchdog,input}.rs`, con
alias en `lib.rs` (`pub use subagent::phase as subagent_phase`…) que conservan las rutas públicas.
**Paso 2.2 completo.**

`subagent/mod.rs` con re-exports (`pub use`) y archivos por responsabilidad. Un commit
por responsabilidad extraída; movimiento puro.

**Aceptación:** 0 archivos de `subagent/` superan 1.000 líneas; API pública intacta;
cuatro checks verdes en cada commit; ningún consumidor cambia sus imports.

---

## Fase 3 — Refactor de `render.rs` (4.810 líneas) (HECHA 2026-11-05)

`render/mod.rs` queda en ~190 líneas (la función `render`); zonas en `chrome`, `transcript`, `sidebar`,
`settings`, `overlays`, `a2a` y `marks` (la mayor, 775); 84 tests repartidos en `render/tests/` por
tema. Movimiento puro; entrada de `render.rs` fuera de la lista blanca.

Un módulo por zona visual (header, transcript, rail, composer, footer).

**Aceptación:** cada zona en su archivo, ninguno sobre 1.000 líneas, salida visual
idéntica (los tests de render existentes como red).

---

## Fase 4 — Refactor de `server.rs` (3.228 líneas) (HECHA 2026-11-05)

`server/mod.rs` queda en 47 líneas (declaraciones); nueve zonas: `protocol`, `daemon`, `client`,
`manager`, `worker`, `snapshot`, `agent_build`, `agent_ui`, `runtime_tasks` (la mayor, 493); los dos
módulos de tests pasan a `runtime_tests.rs` y `tests.rs`. Lo que usaba `super::` para llegar a la raíz
del crate pasa a `crate::`. Entrada de `server.rs` fuera de la lista blanca.

Aislado del resto: IPC · ciclo de sesión · snapshot · suscripciones.

**Aceptación:** ninguno sobre 1.000 líneas; tests de servidor verdes.

---

## Fase 5 — Refactor de `app.rs` (10.769 líneas) — HECHA

**Hecho (2026-11-05):** `app.rs` pasó a `app/` con `mod.rs` (518 líneas: `App`, `new`), 13 ficheros de métodos por responsabilidad (`session`, `events`, `settings`, `agents`, `usage`, `children`, `activity`, `tool_sync`, `pointer`, `prompts`, `composer`, `turn`, `prefs`), tres de tipos (`types`, `settings_types`, `child_types`) y `tests/` en 12 temas más `support`. Movimiento puro: 540 tests antes y después; ningún fichero pasa de 700 líneas; `app.rs` sale de la lista blanca. La precondición de la 6a se cumplió (el borrador por sesión ya estaba hecho); la 6b va después.

**Precondición:** Fase 6 del plan de estados (`2026-11-04`) terminada, para no tener dos
refactores del composer a la vez.

Extraer por responsabilidad: estado de sesión · estado de hijos · scroll/selección ·
comandos · layout. No es estético: es para que el bug del composer (`load_session` en
`app.rs`, no toca `self.composer`) tenga una frontera visible.

**Aceptación:** `app.rs` por debajo de 1.000 líneas; tests de TUI verdes;
`load_session` vive en el módulo de sesión con su test de borrador.

---

## Fase 6 — Refactor de `run.rs` (4.377 líneas) — HECHA

**Hecho (2026-11-05):** `run.rs` pasó a `run/`: `mod.rs` (bucle `run`, tipos de órdenes; ~415 líneas), `keys`, `dialogs`, `slash`, `events`, `clipboard` y `tests/` en 12 temas más `support`. Movimiento puro (119 tests de `run` sin cambios); ningún fichero pasa de 800 líneas; `run.rs` sale de la lista blanca.

**Precondición:** Fase 5 hecha (el bucle depende del estado de `app.rs`).

Bucle de eventos · teclas por contexto · driver de tick.

**Aceptación:** ninguno sobre 1.000 líneas; tests de TUI verdes; el composer no
parpadea ni se retrasa (prioridad de suavidad del composer).

---

## Fase 7 — Refactor de `agent/mod.rs` (5.613 líneas) — HECHA

**Hecho (2026-11-05):** `agent/mod.rs` conserva las constantes, `TurnOutcome` y `AgentLoop` (155 líneas); los métodos se reparten en `builder`, `subagents`, `entry`, `compaction`, `usage` y `turn` (junto a los módulos que ya existían) y los tests, en 10 temas más `support`. Movimiento puro: 741 tests antes y después; ningún fichero pasa de 660 líneas. `run_turn` sigue siendo un único método de ~630 líneas: partirlo sería ya un cambio de lógica, no un movimiento.

Motor central. Turno · tool loop · compactación · dispatch. Máximo cuidado y último
de este ciclo: si hay prisa por features, esta es la fase que se pospone.

**Aceptación:** ninguno sobre 1.000 líneas; tests de motor verdes.

---

## Orden y dependencias

```
Fase 0 (docs) ──→ Fase 1 (checks) ──→ Fase 2 (subagent) ──→ Fase 3 (render)
                                                              ├──→ Fase 4 (server)
                                                              └──→ Fase 5 (app) ──→ Fase 6 (run)   [5 tras Fase 6 de estados]
                                                                    Fase 7 (agent): independiente, posponible
```

- **Fase 0 puede empezar ya** y no depende de nada.
- Cada fase retira de la lista blanca del paso 1.1 los archivos que parte.

## Métricas de éxito

Línea base a 2026-11-05; los valores se recalculan con el test del paso 1.1.

| Métrica | Hoy | Objetivo de este ciclo |
|---|---:|---:|
| % de código en archivos > 1.000 líneas | 54,5 % | < 30 % |
| Archivos > 1.000 líneas | 25 | < 20 (el resto, en una oleada posterior) |
| `app.rs` | ~10.800 | < 1.000 |
| Documentos de navegación | 0 | 5 + 8 READMEs |
| Specs/plans en el INDEX con estado | 0 | todos (test de cobertura) |
| Saltos para localizar un punto de cambio | ≥ 5 | ≤ 3 |

## Riesgos

| Riesgo | Mitigación |
|---|---|
| El refactor de `app.rs` rompe la TUI | Se hace tarde, con commits de movimiento puro y los tests de render como red |
| La documentación se queda obsoleta | Regla en `AGENTS.md`: se actualiza en el mismo commit que la invalida |
| `docs/INDEX.md` se desincroniza | Test de cobertura (paso 1.3) |
| Choque con el trabajo de A2A en curso | `subagent.rs` se parte primero (Fase 2) y `app.rs`/`run.rs` esperan a la Fase 6 de estados |
| Refactor eterno sin features | Cada fase es un commit cerrado; la Fase 7 es posponible |
| Rutas inventadas en la documentación | Cada ruta citada se verifica con `ls` antes de commitear |

## Fuera de alcance

- Reescribir la arquitectura de crates o fusionar/splitear crates.
- Cambiar el modelo de datos de sesión.
- Refactorizar archivos grandes pero cohesivos (`theme.rs`, providers, tablas): están
  justificados como excepción.
- Los ~19 archivos > 1.000 líneas que queden tras este ciclo.
- Migrar a otra herramienta de documentación (mdbook u otra).
- Cobertura de tests nueva: se usa la existente como red.

## Preguntas abiertas

Ninguna (cerradas en la sección "Decisiones" del spec).
