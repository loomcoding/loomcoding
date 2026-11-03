# Arquitectura de CodeLoom

Mapa de capas y guía de "dónde vive qué". Este documento es un índice: la intención de
diseño está en los specs enlazados al final, no aquí.

## Mapa de capas

```
                        ┌──────────────┐
                        │ codeloom-cli │  binarios `loom` y `codeloom`
                        └──────┬───────┘
                 ┌─────────────┼─────────────┐
                 ▼             ▼             ▼
          ┌────────────┐ ┌───────────┐ ┌───────────┐
          │codeloom-tui│ │codeloom-  │ │codeloom-  │
          │            │ │  herdr    │ │   mcp     │
          └─────┬──────┘ └─────┬─────┘ └─────┬─────┘
                │              │             │
                ▼              ▼             ▼
          ┌──────────────────────────────────────┐
          │           codeloom-core              │  motor puro
          └──────────────┬───────────────────────┘
                         ▼
          ┌──────────────────────────────────────┐
          │           codeloom-a2a               │  edge de red
          └──────────────────────────────────────┘

          ┌──────────────┐
          │    batten    │  widgets y estado de TUI (sin motor)
          └──────────────┘
```

## Reglas de dependencia (duras)

- **`codeloom-core` no depende de `ratatui` ni de `crossterm`.** El motor es una
  librería pura y testeable. Toda la interfaz vive en crates opcionales.
- **`codeloom-core` no hace red real en tests.** El provider es siempre un mock
  (`wiremock` o un `Provider` de test).
- **Errores de librería con `thiserror`**; el binario puede usar `anyhow`.
- **Herramientas que escriben son `ExecutionMode::Exclusive`** (forman barrera); las de
  solo lectura son `Parallel`.
- **Nada sin cota llega al modelo**: todo output de herramienta, sección de prompt y
  bucle tiene un límite. Ver el decálogo enlazado abajo.

## Crates

| Crate | Responsabilidad | ¿Opcional? |
|---|---|---|
| `codeloom-core` | Motor: agente, sesiones, herramientas, provider, eventos, tareas, subagentes | no |
| `codeloom-a2a` | Edge A2A: tarjeta, JSON-RPC, SSE, cliente y servidor | no |
| `codeloom-cli` | Punto de entrada; construye los binarios `loom` y `codeloom` | no |
| `codeloom-tui` | Interfaz de terminal sobre `ratatui` | sí |
| `batten` | Widgets y estado de TUI reutilizables, sin motor | sí |
| `codeloom-herdr` | Integración con el runtime `herdr` como plugin | sí |
| `codeloom-mcp` | Integración MCP | sí |
| `codeloom-e2e` | Pruebas de extremo a extremo | solo test |

## Los dos binarios

`codeloom-cli` declara dos `[[bin]]` que comparten `src/main.rs`: **`loom`** (comando
corto) y **`codeloom`**. Ambos autodetectan el modo de ejecución: standalone o plugin de
`herdr`. Es un par de binarios, dos modos.

## Si buscas X, mira en Y

| Buscas | Mira en |
|---|---|
| Modelo de tarea A2A (`TaskState`) | `crates/codeloom-core/src/task.rs` |
| Fase local del subagente (`SubagentPhase`) | `crates/codeloom-core/src/subagent_phase.rs` |
| Descriptor de subagente y su persistencia | `crates/codeloom-core/src/session/mod.rs`, `session/sqlite.rs` |
| Bucle del agente y criterios de parada | `crates/codeloom-core/src/agent/mod.rs`, `agent/stop.rs` |
| Ejecución y ciclo de vida de subagentes | `crates/codeloom-core/src/subagent.rs` |
| Entrada dirigida a un subagente | `crates/codeloom-core/src/subagent_input.rs` |
| Registro de herramientas | `crates/codeloom-core/src/tools/` |
| `trait Provider` y clientes de modelo | `crates/codeloom-core/src/provider/` |
| Bus de eventos | `crates/codeloom-core/src/event.rs` |
| Presupuesto y cotas | `crates/codeloom-core/src/budget.rs` |
| Permisos | `crates/codeloom-core/src/permission.rs` |
| Configuración | `crates/codeloom-core/src/config.rs` |
| Tarjeta de agente A2A | `crates/codeloom-core/src/card.rs`, `crates/codeloom-a2a/src/` |
| Estado y handles de teclado de la TUI | `crates/codeloom-tui/src/app.rs` |
| Renderizado | `crates/codeloom-tui/src/render.rs` |
| Arranque y bucle de la TUI | `crates/codeloom-tui/src/run.rs` |
| Integración con herdr | `crates/codeloom-herdr/src/` |

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
