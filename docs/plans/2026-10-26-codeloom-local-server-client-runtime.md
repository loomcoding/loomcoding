# Servidor local Loomcoding y clientes TUI — Plan

> Estado: implementación R1–R6 completada; checks globales verificados en Windows el 2026-10-27.
>
> Spec: `docs/specs/2026-10-26-codeloom-local-server-client-runtime.md`.

## Alcance

Definir e implementar el runtime local propiedad de Loomcoding: una instancia servidor por usuario, un agente supervisor y varias TUI cliente con agentes aislados por directorio. first-agents/A2A es la referencia para identidad, AgentCards, descubrimiento y comunicación entre agentes. Herdr solo es referencia; no se modifica ni se necesita para ejecutar Loomcoding.

## Límite de protocolo

- AgentCards y comunicación entre agentes deben usar únicamente tipos y operaciones A2A soportados. El registro de tarjetas es local al runtime; no se inventa un método de listado A2A.
- El runtime puede mantener metadatos internos (por ejemplo, que un agente es interno) en su registro y política local. A2A no define un campo estándar `internal`; no añadirlo a la tarjeta como campo propio.
- El IPC TUI-servidor es transporte privado de implementación para renderizado y control local; no se presenta como protocolo A2A ni como API de agentes.
- El supervisor observa y controla el runtime, pero no ejecuta las tareas de trabajo delegadas a los agentes.

## Fases

1. **Caracterizar el runtime actual**: `main`, `AgentLoop`, canales TUI, `SqliteStore`, provider/config por proyecto, permisos y apagado. Separar el A2A server de la tarea local.
2. **Cerrar transporte y ciclo de vida**: comprobar dependencias existentes, endpoint por SO, ACL/permisos, bloqueo singleton, handshake de versión, startup concurrente, reconexión y apagado sin clientes.
3. **Fijar el IPC de UI**: socket privado o named pipe con DACL, negociación de versión, JSON length-prefixed y comandos/eventos acotados. El límite vigente es 12 MiB para transportar prompts con adjuntos de imagen locales (versión 5); se mantiene separado de A2A.
4. **Levantar servidor único**: startup race-safe, hijo oculto, 5 minutos idle shutdown; persistir sesiones en el único SQLite owner. `--print` se procesa como petición sin TUI.
5. **Crear el agente supervisor**: mantener el registro/observación sin llamadas al LLM; para razonamiento, usar provider/modelo opcionales configurados para el perfil de usuario, con sesión y coste propios. No heredar `.env` ni configuración de ningún proyecto. Si el LLM no está configurado o falla, seguir el flujo clásico determinista sin bloquear servidor ni agentes; no relajar permisos ni aprobaciones.
6. **Extraer el runtime**: mover `AgentLoop`, provider, tools, permisos, sesiones, A2A y jobs al servidor. Resolver `.env` en un mapa por proyecto, nunca mutar el entorno global.
7. **Convertir la TUI en cliente**: bridge de comandos, eventos, sesiones, catálogo, permisos y preguntas sobre IPC; mantener el renderer existente.
8. **Aislar agentes**: ID único por invocación interactiva, sesiones filtradas por agente, configuración/env/provider por proyecto y directorio de worktree propio.
9. **Supervisor y agentes internos**: registrar la AgentCard de cada agente y su clasificación interna; publicar únicamente las interfaces A2A previstas y autorizadas. Usar `AgentInterface.tenant` para enrutar un endpoint compartido cuando aplique. Retirar `local_agents`/`ask_local_agent` como herramientas de comunicación entre agentes; las tarjetas se distribuyen por los mecanismos de descubrimiento configurados, y las tareas usan operaciones A2A existentes. La aprobación, permisos y presupuesto son política local del runtime.
10. **Verificación local**: carreras de arranque, ACL, límites de marcos, dos proyectos, `--attach`, cliente desconectado, aprobación/rechazo de mensajes, aislamiento de `.env`, configuración y coste propios del supervisor, fallback ante timeout/error del LLM y un único SQLite; providers mockeados y sin red en tests.
11. **Checks requeridos**: `cargo build`, `cargo test`, `cargo fmt --check` y `cargo clippy -- -D warnings`.

## Riesgos principales

- La base `sessions.db` actual asume un solo proceso; solo el servidor la abrirá.
- La configuración actual incluye claves de entorno y `.env`. Un servidor multirepo debe resolverlas por agente sin contaminación cruzada.
- El proceso servidor no puede depender de la vida de la primera TUI ni perder agentes al desconectar otra.
- El arranque simultáneo debe dejar un solo endpoint/servidor y no borrar sockets activos.
- Named pipes de Windows necesitan restricciones al usuario propietario; un endpoint local sin ACL puede filtrar datos o aceptar comandos.
- La API local no debe reutilizar A2A HTTP ni añadir un puerto de red.

## Avance

- R1–R5 implementados y R6 verificado; las specs y planes por tarea están enlazados en `docs/specs/2026-10-27-loomcoding-runtime-*.md` y `docs/plans/2026-10-27-loomcoding-runtime-*.md`.
- Se retiraron las herramientas `local_agents`/`ask_local_agent` y los mensajes directos de agente por IPC. Las tareas entre agentes usan `SendMessage`, `GetTask`, `CancelTask` y streaming A2A existentes; el directorio local no añade un método de listado.
- El runtime expone un listener común solo si el perfil de usuario activa A2A y configura autenticación. Cada AgentCard declara el `tenant`; el cliente que consume la tarjeta lo replica en sus peticiones según A2A 1.0.
- Las AgentCards locales llevan una marca interna solo en el registro runtime. El JSON de la tarjeta se mantiene A2A estándar.
- El supervisor opera con registro determinista y LLM opcional; el provider del perfil, credencial, sesión y uso están separados de los agentes. Los errores y timeout caen al estado determinista y nunca aprueban permisos.
- Checks finales en Windows: `cargo build --target-dir .\target` correcto; `cargo test --target-dir .\target` 1648 aprobadas y 13 ignoradas; `cargo fmt --check`, `cargo clippy --target-dir .\target -- -D warnings` y `git diff --check` correctos. Build/clippy conservan el aviso existente de `main.rs` compartido entre los binarios `loom` y `loomcoding`.
- El test de carrera de socket `cfg(unix)` está añadido, pero necesita ejecutarse en CI Unix. Se especificó `--target-dir .\target` para mantener los artefactos dentro del workspace.

## Backlog de tareas

| Tarea | Alcance | Estado |
|---|---|---|
| R1 — AgentCards runtime | Identidad y clasificación interna por agente vivo; especificación y plan propios. | Verificado |
| R2 — Entrada A2A compartida | Una interfaz A2A enruta por `tenant` a agentes vivos; tarjetas por agente en ubicación de descubrimiento directa; solo métodos A2A existentes. | Verificado |
| R3 — Ejecución A2A sobre agentes vivos | Mapear `SendMessage`, `GetTask`, `CancelTask` y streaming soportado a tareas/sesiones del agente destino, con permisos y consentimiento local. | Verificado |
| R4 — Supervisor opcional | Supervisión determinista siempre activa; LLM opcional con provider/config/sesión/coste propios y fallback clásico acotado. | Verificado con providers mock |
| R5 — Aislamiento y ciclo de vida | Arranque único, attach, desconexión, apagado, `.env` por proyecto, SQLite único y exposición externa autenticada. | Verificado |
| R6 — Cierre/verificación | Aceptación de la spec completa y los cuatro checks del repositorio. | Completado |

## Avance de implementación R2–R5

- `~/.loomcoding/runtime.toml` es la configuración de runtime. `[a2a] role = "serve"` o `"both"` crea un solo listener autenticado; si no se activa, no se abre puerto. El secreto se resuelve del entorno o Credential Manager; en Windows se genera y guarda ahí cuando se activa serving sin secreto previo.
- Las AgentCards de agentes vivos se resuelven directamente en `/agents/{tenant}/.well-known/agent-card.json`; cada interfaz declara el tenant estándar. `SendMessage`, `SendStreamingMessage`, `GetTask`, `CancelTask` y `SubscribeToTask` comparten el host del runtime y no añaden métodos A2A.
- Las tareas de agentes pasan por su cola y sesión de proyecto, con límites de concurrencia, duración y resultado. Los permisos siguen vinculados a la TUI; cancelación de tarea no cancela las otras sesiones.
- La tarjeta `supervisor` publica una instantánea determinista. Si `[supervisor]` tiene provider/modelo y la credencial de entorno existe, una petición explícita usa una sesión propia, sin herramientas, con timeout de 15 s y salida limitada. Si falta o falla, completa con estado determinista.
- Se eliminó el target compartido anterior por instrucción del usuario; se liberó espacio y las compilaciones posteriores usaron el target dentro del workspace.

## Evidencia de la inspección

- No existe una spec de Loomcoding daemon + TUI cliente.
- La spec base conservaba inicialmente standalone/plugin de Herdr; su nota de vigencia y las decisiones 1-2 ya apuntan al servidor Loomcoding propio. El resto de §8 queda solo como descripción del adaptador opcional.
- `crates/loomcoding-cli/src/main.rs` crea provider, `SqliteStore`, `AgentLoop` y TUI dentro de cada proceso.
- `crates/loomcoding-a2a/src/serve` expone el protocolo A2A HTTP para agentes remotos; no coordina las TUI locales.
- El `SqliteStore` actual documenta el supuesto de un proceso por base.

## Dogfooding

El intento anterior de implementar presencia con Loomcoding no produjo cambios: en Windows `head`, `pwd` y `grep` fallaron como comandos. Las cuatro sesiones registraron 79 326 tokens de entrada y 2 820 de salida; no se comprobó el coste en USD. No repetir llamadas al proveedor hasta corregir selección de shell y fijar un presupuesto por ejecución.
