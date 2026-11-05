# Loomcoding — Sacar los flujos de orquestación del núcleo

> Estado: **propuesta** (2026-11-05). Plan: `docs/plans/2026-11-05-loomcoding-extraer-flujos-del-core.md`.
> Relacionado: `2026-11-04-loomcoding-gauntlet-minimo.md` (el flujo que se mueve),
> `docs/ARCHITECTURE.md` y `AGENTS.md` (regla: «TUI, integraciones… viven en crates opcionales, nunca en el núcleo»).

## 1. Problema

El Gauntlet y `development_workflow` están dentro de `loomcoding-core`. Medido el 2026-11-05:

- **~3 600 líneas de Gauntlet en el núcleo** (con pruebas). `gauntlet.rs` tiene 911 líneas, el segundo
  fichero más grande del núcleo; mezcla identificadores y fases, sorteo A/B, parseo del veredicto, modelo
  de estado y guardas de lectura.
- **Acoplamiento en ~20 ficheros ajenos.** `SessionStore` (`session/mod.rs`) trae seis métodos propios del
  Gauntlet y otros seis del flujo de desarrollo; el esquema SQLite tiene sus tablas; `agent/dispatch.rs`,
  `agent/turn.rs`, `agent/stop.rs`, `agent/mod.rs` y `subagent/launcher.rs` nombran la herramienta
  `gauntlet` en listas de texto; `LiveEvent::GauntletChanged` obliga a la TUI y al snapshot del cliente a
  conocer sus tipos.
- **Ficheros dios**: siete ficheros del núcleo pasan de 700 líneas (`catalog.rs` 976, `gauntlet.rs` 911,
  `config/mod.rs` 886, `transport.rs` 857, `turn.rs` 797, `router.rs` 721, `dispatch.rs` 720).

Consecuencia: cada flujo nuevo (arranque guiado del Gauntlet, lista de tareas con dueño, `check`) añadiría
otro tramo de acoplamiento. El Gauntlet es opcional y ya vive en el núcleo; eso rompe la regla del producto.

## 2. Objetivos y no objetivos

**Objetivos**

1. El núcleo no nombra ningún flujo concreto: ni tipos, ni herramientas, ni tablas, ni eventos.
2. El Gauntlet vive en un crate propio, `loomcoding-workflows`, que depende del núcleo (nunca al revés).
3. Cada fichero nuevo o movido tiene como mucho 400 líneas (excluidas las pruebas) y una regla automática
   impide que crezcan los gordos que quedan.
4. Cero cambios de comportamiento observable: mismas fases, mismos eventos en pantalla, mismos datos.

**No objetivos**

- Mover `development_workflow` (mismo método, plan aparte, cuando este esté cerrado).
- Partir `catalog.rs`, `config/mod.rs`, `transport.rs`, `turn.rs`… (se parten al tocarlos; aquí solo se
  congelan con la regla de tamaño).
- Un motor genérico de flujos o una máquina de estados común: el núcleo solo ofrece **almacenamiento,
  evento y política de turno** genéricos; las fases son del crate del flujo.

## 3. Reglas de dependencia

```text
loomcoding-cli  ──► loomcoding-workflows ──► loomcoding-core
       │                                          ▲
       └──────────► loomcoding-tui ───────────────┘
```

- `loomcoding-core` no depende de `loomcoding-workflows`. `loomcoding-tui` tampoco: pinta lo que el núcleo
  publica de forma genérica.
- `loomcoding-cli` es la raíz de composición: registra las herramientas de `loomcoding-workflows` en el
  `ToolRegistry` al arrancar.

## 4. Puntos de extensión del núcleo

Genéricos, sin la palabra «gauntlet»:

1. **Almacén de flujos** (`crates/loomcoding-core/src/flow.rs`). `FlowId(i64)`;
   `FlowRecord { id, kind: String, parent: SessionId, phase: String, terminal: bool, version: u32,
   summary: FlowSummary, payload: serde_json::Value, created_at_ms, updated_at_ms }` (el resumen viaja en
   el registro para que el snapshot del cliente lo lea sin conocer el flujo); `FlowEventRecord { id, flow, from_phase,
   to_phase, actor, detail, created_at_ms }`. Métodos en `SessionStore`, con implementación por defecto
   que devuelve «flow storage unavailable»: `create_flow`, `update_flow` (con comprobación de `version`),
   `flow`, `active_flow(parent, kind)`, `latest_flow(parent, kind)`, `flow_events`. Tablas SQLite `flows` y
   `flow_events`; un solo flujo activo por sesión y `kind`.
2. **Evento** `LiveEvent::FlowChanged { summary: FlowSummary, actor: String, detail: String }` con
   `FlowSummary { kind, id, phase, round, max_rounds, terminal, label }`. `label` es el texto que la TUI
   muestra tal cual; las fases siguen siendo cadenas del crate del flujo.
3. **Orquestadores**: método `Tool::is_orchestrator(&self) -> bool` (por defecto `false`). El padre con un
   flujo activo queda restringido a las herramientas de lectura y a los orquestadores; un orquestador no se
   ofrece a los subagentes (hoy `.without("gauntlet")`). Las listas de nombres de `dispatch.rs`, `turn.rs`,
   `stop.rs`, `agent/mod.rs` y `launcher.rs` desaparecen.
4. **Registro**: `ToolRegistry::register` ya es público. `project_registry` deja de registrar el Gauntlet;
   `loomcoding_workflows::register(&mut ToolRegistry)` lo hace y el binario lo llama.

## 5. Regla de tamaño

Ya existe un guardarraíl (`crates/loomcoding-e2e/tests/guardarrailes.rs`, plan de organización, hecho) con
techo de 1 000 líneas y lista blanca que solo decrece; hoy la lista está vacía. Se **endurece**: techo de
**400 líneas para el código** (se excluyen los ficheros de pruebas, que conservan el de 1 000) y una lista
blanca inicial con los ficheros de código que hoy pasan de 400 (60 el 2026-11-05; 36 pasan de 500 y 16 de
700). Se aplican las reglas que ya tiene: solo decrece, un tope por fichero y fallo si queda una entrada
obsoleta. Cada tarea que parta un fichero retira su entrada. Se anota en `INVARIANTS.md`.

## 6. Migración de datos

Las bases existentes tienen las tablas `gauntlets` y `gauntlet_events`. Al abrir el almacén, si `flows`
está vacía y `gauntlets` tiene filas, se copian una sola vez a `flows` con `kind = 'gauntlet'`; las tablas
viejas no se borran. Cubierto por una prueba con una base creada con el esquema antiguo.

## 7. Riesgos y vigilancia

- **Formato del cable.** `LiveEvent` viaja entre el daemon y el cliente: un cliente viejo con un daemon
  nuevo no entenderá `FlowChanged`. Mismo binario para ambos hoy; se anota en el CHANGELOG.
- **Visibilidad.** Mover la herramienta obliga a hacer `pub` lo que hoy es `pub(crate)` en el núcleo
  (lanzador de revisión de solo lectura, ayudas de la sesión): mínimo imprescindible, nada más.
- **Paridad en pantalla.** La TUI pinta hoy `gauntlet #N · fase · ronda a/b` desde tipos; debe pintar lo
  mismo desde `FlowSummary.label`, con las pruebas de la TUI en verde sin tocarlas más de lo necesario.

## 8. Criterios de aceptación

1. `grep -ril gauntlet crates/loomcoding-core crates/loomcoding-tui` no devuelve ninguna fuente (solo
   documentación, si acaso).
2. `loomcoding-core` compila y pasa sus pruebas sin `loomcoding-workflows`.
3. Las pruebas del Gauntlet (reglas, herramienta, e2e con proveedor simulado) pasan en el crate nuevo, en
   número no menor que hoy.
4. Una base con filas antiguas de `gauntlets` se lee como flujos tras abrirla.
5. La prueba de tamaño existe, pasa y la lista no tiene entradas nuevas respecto de la fotografía de hoy.
6. Los cuatro chequeos del repositorio (`build`, `test`, `fmt --check`, `clippy -D warnings`) en verde en
   cada commit.

## Preguntas abiertas

- ¿Nombre del crate: `loomcoding-workflows` (agrupa también `development_workflow` después) o
  `loomcoding-gauntlet`? Se propone el primero.
- ¿El tope de 400 líneas es el adecuado? Se propone 400 para ficheros nuevos y congelar los actuales.
