# Arquitectura de CodeLoom

Mapa de capas, flujo de un mensaje y guía de «dónde vive qué». Este documento es un
mapa, no un tutorial: la intención de diseño está en los specs enlazados al final. Las
rutas citadas se verifican contra el código; si mueves un archivo, actualiza aquí en el
mismo commit.

## Mapa de capas

Las flechas van de quien depende hacia lo que usa (verificado en los `Cargo.toml`).

```
                        ┌──────────────┐
                        │ codeloom-cli │  binarios `loom` y `codeloom`
                        └──────┬───────┘
        ┌───────────┬──────────┼──────────┬───────────┐
        ▼           ▼          ▼          ▼           ▼
 ┌────────────┐ ┌─────────┐ ┌────────┐ ┌────────┐ ┌─────────┐
 │codeloom-tui│ │codeloom-│ │codeloom│ │codeloom│ │ codeloom│
 │            │ │  herdr  │ │  -mcp  │ │  -a2a  │ │  -e2e*  │
 └──┬───────┬─┘ └─────────┘ └───┬────┘ └───┬────┘ └────┬────┘
    │       │  (herdr: sin      │          │           │
    ▼       │   deps internas)  ▼          ▼           ▼
┌────────┐  └───────────────►┌──────────────────────────────┐
│ batten │                   │         codeloom-core        │  motor puro
└────────┘                   └──────────────────────────────┘
  (sin motor)
                                          * solo como dev-dependency
```

`batten` no depende de nadie del workspace; `codeloom-core` tampoco. `codeloom-a2a` y
`codeloom-mcp` dependen de `core`, **nunca al revés**.

## Reglas de dependencia (duras)

- **`codeloom-core` no depende de `ratatui` ni de `crossterm`.** El motor es una
  librería pura y testeable. Toda la interfaz vive en crates opcionales.
- **Sin red real en tests.** El provider es siempre un mock (`wiremock` o un `Provider`
  de test).
- **Errores de librería con `thiserror`**; el binario puede usar `anyhow`.
- **Herramientas que escriben son `ExecutionMode::Exclusive`** (forman barrera); las de
  solo lectura son `Parallel`.
- **Nada sin cota llega al modelo**: todo output de herramienta, sección de prompt y
  bucle tiene un límite. Ver el decálogo enlazado abajo.

Cómo se comprueba cada una: `docs/INVARIANTS.md`.

## Crates

| Crate | Responsabilidad | Entrada principal | No va aquí | ¿Opcional? |
|---|---|---|---|---|
| `codeloom-core` | Motor: agente, sesiones, herramientas, provider, eventos, tareas, subagentes | `agent/mod.rs` (`AgentLoop`) | TUI, terminal, red A2A | no |
| `codeloom-a2a` | Borde A2A: tarjeta, JSON-RPC, SSE, cliente y servidor | `lib.rs`, `serve/`, `client.rs` | lógica del motor | no |
| `codeloom-cli` | Binarios `loom`/`codeloom`; daemon, IPC y arranque | `main.rs`, `server/mod.rs` | widgets, lógica del agente | no |
| `codeloom-tui` | Interfaz de terminal sobre `ratatui` | `run/mod.rs`, `app/mod.rs`, `render/mod.rs` | acceso directo al motor sin pasar por el cliente | sí |
| `batten` | Widgets y estado reutilizables de chat de agentes | `state/`, `widgets/` | nada de `codeloom-core` | sí |
| `codeloom-herdr` | Integración con el runtime `herdr` como plugin | `lib.rs` | TUI propia | sí |
| `codeloom-mcp` | Integración MCP (herramientas externas) | `lib.rs` | — | sí |
| `codeloom-e2e` | Apoyo a pruebas E2E visuales | `daemon.rs`, `term.rs` | código de producción | solo test |

Cada crate tiene su `README.md` con la plantilla fija (responsabilidad, tipos clave,
puntos de entrada, trampas).

## Los dos binarios

`codeloom-cli` declara dos `[[bin]]` que comparten `src/main.rs`: **`loom`** (comando
corto) y **`codeloom`**. Ambos autodetectan el modo: standalone o plugin de `herdr`. Es
un par de binarios, dos modos.

## Flujo de un mensaje de punta a punta

Un cliente TUI habla con un **daemon** por IPC; el daemon es quien ejecuta el motor.

1. **Teclas → mensaje.** `crates/codeloom-tui/src/run/` (`keys.rs`: `handle_key`; `mod.rs`: `run`) recoge la
   entrada; el estado vive en `app/` (`App` en `app/mod.rs`; sus métodos, repartidos por responsabilidad).
2. **IPC.** El cliente envía por `crates/codeloom-cli/src/ipc.rs` (`send`/`receive`).
   `server/daemon.rs::connect_or_start` arranca el daemon si no existe.
3. **Daemon.** `server/daemon.rs::run_server` → `serve_connection` → `serve_authenticated`
   atienden la conexión (con token); `run_agent_worker` y `run_turn_with_message` lanzan
   el turno.
4. **Bucle del agente.** `AgentLoop::run`/`run_message` en
   `crates/codeloom-core/src/agent/` (`turn.rs`) leen una ventana del historial
   (`SessionStore::recent_messages`), arman el prompt y llaman al provider.
5. **Provider.** `Provider::stream` (`provider/mod.rs`); implementaciones en
   `provider/{anthropic,openai,gemini}.rs`.
6. **Herramientas.** Si el modelo pide tools, `agent/dispatch.rs::execute_tools` las
   ejecuta contra el `ToolRegistry` (`tools/mod.rs`): las `Parallel` a la vez, las
   `Exclusive` como barrera. El permiso se decide en `permission/mod.rs`.
7. **Eventos.** Todo lo observable se publica como `LiveEvent` en el `EventBus`
   (`event/mod.rs`). El daemon se suscribe (`bus.subscribe()` en `server/agent_build.rs` y `server/client.rs`) y reenvía
   al cliente.
8. **Persistencia.** Cada mensaje se guarda con `SessionStore::append_message`
   (`session/mod.rs`; SQLite en `session/sqlite/`: `mod.rs` solo reenvía y cada tema vive en su módulo; los métodos inherentes llevan prefijo `sql_` porque, sin él, harían sombra al trait para quien use `SqliteStore` a pelo).
9. **Pintado.** El cliente aplica el evento a `App` y `render/` lo dibuja con los
   widgets de `batten`.

## Los cinco subsistemas que hay que conocer

| Subsistema | Tipo clave | Dónde |
|---|---|---|
| Provider | `trait Provider` (`capabilities`, `stream`) | `crates/codeloom-core/src/provider/mod.rs` |
| Herramientas | `trait Tool`, `ToolRegistry`, `ExecutionMode` | `crates/codeloom-core/src/tools/mod.rs` (registro de las integradas en `project_registry`) |
| Bus de eventos | `LiveEvent`, `EventBus` | `crates/codeloom-core/src/event/mod.rs` |
| Sesiones | `trait SessionStore`, `SqliteStore` | `crates/codeloom-core/src/session/{mod,sqlite}.rs` |
| Subagentes | `SubagentLauncher`, `InProcessSubagentLauncher`, `SubagentPhase`, observador de fases, vigilante de latidos y supervisor | `crates/codeloom-core/src/subagent/` (`mod.rs`, `launcher.rs`, `phase.rs`, `phase_observer.rs`, `watchdog.rs`, `supervisor.rs`, `supervisor_driver.rs`, `input.rs`, y el ciclo de vida en `admission.rs`, `local_run.rs`, `remote.rs`, `control.rs`) |

Ojo con el nombre: `crates/codeloom-core/src/registry/` es el **registro de agentes**
(perfiles `<nombre>.md`), no el de herramientas.

## Si buscas X, mira en Y

| Buscas | Mira en |
|---|---|
| Modelo de tarea A2A (`TaskState`) | `crates/codeloom-core/src/task.rs` |
| Fase local del subagente (`SubagentPhase`) | `crates/codeloom-core/src/subagent/phase.rs` |
| Vigilante de latidos (`Stalled`/`Lost`) | `crates/codeloom-core/src/subagent/watchdog.rs` |
| Descriptor de subagente y su persistencia | `crates/codeloom-core/src/session/mod.rs`, `session/sqlite/subagents.rs` |
| Bucle del agente y criterios de parada | `crates/codeloom-core/src/agent/` (`turn.rs`, `entry.rs`, `stop.rs`) |
| Compactación del contexto | `crates/codeloom-core/src/agent/compact.rs` |
| Ciclo de vida de subagentes (contrato, admisión, ejecución local, remotos, control) | `crates/codeloom-core/src/subagent/` (`mod.rs`, `admission.rs`, `local_run.rs`, `remote.rs`, `control.rs`, `launcher.rs`) |
| Entrada dirigida a un subagente | `crates/codeloom-core/src/subagent/input.rs` |
| Herramientas (una por archivo) | `crates/codeloom-core/src/tools/` |
| Registro de agentes (perfiles, router) | `crates/codeloom-core/src/registry/` |
| `trait Provider` y clientes de modelo | `crates/codeloom-core/src/provider/` |
| Bus de eventos | `crates/codeloom-core/src/event/mod.rs` |
| Presupuesto y cotas | `crates/codeloom-core/src/budget.rs` |
| Permisos | `crates/codeloom-core/src/permission/mod.rs` |
| Configuración (`[agents]`, umbrales) | `crates/codeloom-core/src/config/mod.rs` |
| Tarjeta de agente A2A | `crates/codeloom-core/src/card.rs`, `crates/codeloom-a2a/src/` |
| Daemon, IPC y servidor (protocolo, daemon, cliente, gestor de agentes, worker, instantáneas) | `crates/codeloom-cli/src/server/`, `ipc.rs` |
| Estado y teclas de la TUI | `crates/codeloom-tui/src/app/`, `run/` |
| Renderizado de la TUI (una zona por fichero: `chrome`, `transcript`, `sidebar`, `settings`, `overlays`, `a2a`, `marks`) | `crates/codeloom-tui/src/render/` |
| Widgets reutilizables | `crates/batten/src/widgets/`, estado puro en `crates/batten/src/state/` |
| Integración con herdr | `crates/codeloom-herdr/src/` |

Para «dónde toco para hacer X» paso a paso, ver `docs/CODE-MAP.md`.

## Documentos de diseño

- [Diseño base](specs/2026-10-13-loom-design.md) — el modelo de orquestación
  agente → subagentes → jobs.
- [Vista general de la arquitectura](specs/2026-10-14-loom-architecture-overview.md) —
  cómo encajan todas las piezas.
- [Decálogo de contexto y coste](specs/2026-10-22-codeloom-decalogo-de-contexto-y-coste.md) —
  las cotas que nada puede saltarse.
- [Hoja de ruta A2A](plans/2026-10-22-codeloom-a2a-roadmap.md) — estado de cada tarea A2A.
- [Fallos observados en subagentes](status/2026-11-05-fallos-subagentes.md) — bitácora de
  campo, no diseño.
