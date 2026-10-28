# Plan — Flujo de desarrollo coordinado por agentes

> Estado: implementación realizada; suite completa verificada el 2026-10-28; revisión visual pendiente.
>
> Spec: `docs/specs/2026-10-25-codeloom-agent-development-workflow.md`.
>
> La spec y este plan se aprobaron antes de iniciar cambios.

## Objetivo

Implementar el flujo aprobado como coordinación acotada sobre el runtime de
subagentes existente. El usuario aprueba el plan exacto y acepta el resultado;
el runtime hace cumplir las transiciones. No crear un motor genérico de
workflows ni cambiar el protocolo A2A.

## Límites de diseño

- `TaskState` y `SubagentDescriptor` describen tareas A2A/subagentes; no
  reutilizarlos como fases del flujo de desarrollo.
- La decisión de aprobación se liga a los bytes presentados de spec y plan.
  Cualquier edición invalida la aprobación antes de iniciar al implementador.
- El perfil/identidad de cada rol proviene del registro existente. No fijar
  nombres, proveedores ni skills de terceros en `codeloom-core`.
- El revisor debe carecer de herramientas que puedan modificar el árbol. Una
  instrucción de prompt que diga «no edites» no satisface este límite.
- Publicar el cambio de estado solo después de persistirlo. Una caída o
  desconexión no puede convertir una aprobación ausente en aprobación.
- Mientras un workflow esté activo, el turno padre solo puede usar el
  coordinador y herramientas de lectura; el dispatch debe bloquear escritura y
  delegaciones genéricas, también si llegan en el mismo lote que el inicio.
- Mantener límites de contexto/salida, presupuestos, permisos y cancelación que
  ya se aplican a los subagentes.

## Fases

### 0. Aprobar y fijar alcance

1. Revisar juntos spec y plan; registrar las decisiones pendientes de la fase 1.
2. Confirmar el alcance de los perfiles: la propuesta para el primer corte es
   local-only; no asumir paridad de pausa, cancelación o revisión sin escritura
   en los transportes A2A remotos.

**Salida:** spec y plan aprobados, sin abrir aún cambios de código.

### 1. Auditoría de los puntos de integración

Leer y anotar contratos concretos antes de diseñar tipos:

- Delegación: `crates/codeloom-core/src/tools/subagent.rs` y
  `crates/codeloom-core/src/subagent.rs` (`SubagentLauncher`, seguimiento,
  cancelación y entrega del resultado).
- Perfiles y permisos: `crates/codeloom-core/src/registry/profile.rs`,
  `crates/codeloom-core/src/registry/mod.rs` y construcción del `ToolRegistry`
  en `crates/codeloom-cli/src/main.rs`/`server.rs`.
- Preguntas: `crates/codeloom-core/src/tools/ask_user.rs`, el responder del
  runtime y el diálogo en `crates/codeloom-tui/src/app.rs`.
- Durabilidad: `SessionStore` en `crates/codeloom-core/src/session/mod.rs` y
  migraciones/adaptador en `crates/codeloom-core/src/session/sqlite.rs`.
- Eventos y cliente: `crates/codeloom-core/src/event/mod.rs`,
  `crates/codeloom-cli/src/server.rs`, el puente de `main.rs` y
  `crates/codeloom-tui/src/run.rs`, `app.rs` y `render.rs`.

Decisiones aprobadas junto con la spec:

| Decisión | Propuesta |
|---|---|
| Inicio y perfiles | Petición explícita del usuario; el padre invoca el coordinador con tres nombres del registro existente. |
| Puerta de aprobación | El coordinador valida cada transición antes de llamar al launcher; no depende solo de las instrucciones del modelo. |
| Revisor | Primera versión local; lista permitida `view`, `glob`, `grep`. Recibe diff y evidencia, sin shell libre ni herramientas de escritura. |
| Durabilidad | SQLite guarda estado actual y transiciones append-only, sesión padre, IDs de hijos, refs de artefactos, rondas y SHA-256 de spec/plan. |
| Respuestas no afirmativas | Solo la opción explícita aprueba; texto libre, cancelación, timeout o desconexión detienen/cancelan el flujo sin avanzar. |

Revalidar las huellas antes de cada fase que consume los artefactos aprobados.

### Resultado de la auditoría de contratos

- `SubagentLauncher::start` retorna enseguida y el hijo sigue en segundo plano;
  `SubagentSettled` y el aviso durable despiertan al padre. El coordinador
  avanzará entre turnos del padre, sin bloquear un turno durante minutos.
- `SessionStore` solo persiste sesiones, mensajes, subagentes y uso. Se
  necesitan estado e historial de workflow propios con migraciones SQLite
  aditivas y consulta por sesión padre.
- `ToolCtx` no expone el almacén; `AgentLoop` sí lo posee. Pasarlo al contexto
  permitirá al coordinador persistir y al dispatch aplicar la puerta del padre.
- El lanzador entrega a todos los hijos el mismo `ToolRegistry`. Debe aceptar
  una política local para el revisor y construirle una allowlist; los hijos no
  recibirán la herramienta de workflow.
- `AgentProfile` no codifica roles; el coordinador recibe tres nombres del
  registro. La primera versión verifica que sean perfiles locales.
- `ask_user` vuelve como `Option<Vec<Answer>>` y permite texto libre. Solo una
  selección exacta de la opción positiva aprobará; `None` o cualquier otra
  selección cierra el flujo sin avanzar.
- El cliente carga mensajes, uso y subagentes por páginas. Añadir una
  actualización de workflow al mismo flujo de carga y a `LiveEvent` permitirá
  renderizar el estado tras attach y reconexión.
- `sha2` ya es dependencia directa de `codeloom-core`; no hace falta añadir
  dependencias para las huellas.

**Salida:** contratos auditados; las brechas y puntos de integración quedan
registrados arriba. Si durante la implementación aparece una contradicción con
la spec, detenerse, actualizar spec y plan y pedir aprobación de la nueva versión.

### 2. Estado mínimo y almacenamiento durable

1. Definir un tipo específico del flujo, con los estados de la spec y
   transiciones validadas. No generalizarlo a otras tareas.
2. Persistir en `SessionStore` la instancia y su historial de transiciones,
   asociados a la sesión padre. Incluir como mínimo fase, perfiles/agentes,
   IDs de subagentes, huellas aprobadas, número de revisión, decisión y motivo
   de cierre.
3. Añadir tablas/migraciones SQLite aditivas y pruebas de reapertura de base;
   conservar íntegros sesiones y datos A2A preexistentes.
4. Validar cada transición en un único punto: fase origen, actor esperado,
   huella y ronda. Rechazar duplicados o una aprobación para contenido distinto.
5. Persistir primero; después publicar el evento de UI. Si falla la escritura,
   no avanzar la ejecución.

**Pruebas:** tabla de transiciones permitidas/prohibidas, huella invalidada,
actualización duplicada, historial de dos revisiones y recuperación tras
reabrir SQLite.

### 3. Coordinación y puertas de aprobación

1. Implementar el coordinador mínimo en `codeloom-core`, conectado al
   `SubagentLauncher` y a la respuesta del usuario existente. No añadir una
   segunda forma de lanzar hijos.
2. Lanzar el planificador con la petición original; guardar y presentar spec y
   plan resultantes como artefactos identificables.
3. Esperar una respuesta explícita ligada a las huellas. Solo la opción de
   aprobación exacta puede crear la transición a `implementing`.
4. Pasar al implementador las referencias y el contenido aprobados. Ante cambio
   de alcance, detener la secuencia y volver a planificación/aprobación.
5. Pasar al revisor la petición original, los artefactos aprobados, diff y
   verificaciones. Usar un conjunto de herramientas de solo lectura impuesto
   por código; probar que no puede editar, ejecutar comandos ni delegar.
6. Si hay hallazgos, iniciar corrección con el implementador aprobado,
   conservar cada dictamen y revisar de nuevo. Permitir como máximo dos rondas
   automáticas; tras la segunda, volver al usuario.
7. Una revisión limpia pasa a `awaiting_final_acceptance`, nunca a `completed`.
   Solo la decisión final explícita completa el flujo.
8. Cancelación, error del agente, timeout sin respuesta o indisponibilidad del
   perfil quedan como estados no exitosos y durables; no disparan la fase
   siguiente.

**Pruebas:** sin aprobación no se lanza implementador; texto libre/ambigüedad,
cancelación, timeout y fallo no aprueban; revisión limpia espera aceptación;
dos rondas y el caso de hallazgos persistentes conservan su historial.

### 4. Eventos y presentación en TUI

1. Añadir un evento tipado mínimo para cambios del flujo; transportarlo por el
   `EventBus` y el IPC local acotado ya existente.
2. En `App`, mostrar fase, agente, revisión y decisión pendiente junto a la
   conversación/árbol actual. Reutilizar el diálogo de `ask_user` para las dos
   decisiones humanas; no crear otro sistema de permisos.
3. Al cargar una sesión, recuperar el estado e historial desde `SessionStore`.
   Tras reconexión, renderizar el mismo estado persistido aunque se hayan
   perdido eventos en vivo.
4. Mantener artefactos, diffs y resultados dentro de los límites de bytes del
   runtime y del IPC.

**Pruebas:** eventos de todas las fases, respuesta y cancelación de diálogos,
reconstrucción visual desde estado cargado y límites del mensaje IPC.

### 5. Verificación y cierre

1. Pruebas de integración con launcher, perfiles y providers mock; cero red
   real.
2. Verificar aislamiento del revisor intentando cada herramienta de escritura
   disponible y comprobar que el árbol no cambia.
3. Revisar los casos de reinicio, desconexión y cancelación en cada puerta.
4. Ejecutar desde la raíz los cuatro checks requeridos antes del commit:
   `cargo build`, `cargo test`, `cargo fmt --check` y
   `cargo clippy -- -D warnings`.
5. Actualizar esta planilla con comandos y resultados exactos, y enlazar la
   implementación desde la spec solo tras verificarla.

## Criterios de aceptación

- El coordinador controla las transiciones sin fiarse solo del prompt del
  agente padre.
- No se ejecuta el implementador antes de aprobar las huellas presentadas.
- El revisor no puede modificar el árbol por ninguna herramienta disponible.
- La aprobación queda invalidada ante cambios en spec o plan.
- Hay hasta dos correcciones automáticas y todas las revisiones quedan
  recuperables; persistir más hallazgos devuelve el control al usuario.
- Revisión limpia espera aceptación final explícita.
- Fallos, cancelaciones, desconexiones y reinicios no saltan puertas ni se
  representan como éxito.
- Estados, agentes, decisiones y artefactos se muestran y reconstruyen desde la
  sesión durable.
- Los cuatro checks del repositorio pasan y no hay llamadas de red en tests.

## Resultado de implementación y verificación (2026-10-27)

- Estado y eventos append-only guardados en tablas SQLite aditivas; se conserva
  la versión optimista, la sesión padre, IDs de hijos, hashes y rutas.
- Herramienta `development_workflow` coordina planner, aprobación, implementer,
  reviewer, hasta dos rondas, revisión del plan tras el límite y aceptación
  final. Las respuestas se aprueban solo por selección exacta.
- Los artefactos se leen como UTF-8 dentro del proyecto, con límite de 20 KB en
  conjunto; cada hash se comprueba de nuevo antes de implementar y revisar.
- Solo se admiten perfiles locales en el flujo. Al reviewer se le entrega una
  allowlist real (`view`, `glob`, `grep`). El padre queda limitado a esas
  herramientas de lectura y el coordinador mientras el flujo esté activo.
- Los cambios de fase llegan a la TUI y la última instancia se recupera al
  cargar una sesión.
- Se añadieron pruebas de transiciones, persistencia SQLite y límites de ruta/
  huella. No se añadieron dependencias.

Verificación ejecutada:

| Comando | Resultado |
|---|---|
| `cargo check --workspace` | Correcto, 24 crates antes de los últimos ajustes de manejo de error y presentación. |
| `cargo test -p codeloom-core --lib` | Correcto: 495 pasaron, 1 ignorada, antes de los últimos ajustes de manejo de error y presentación. |
| `cargo fmt --check` | Correcto sobre el árbol final. |
| `cargo clippy --workspace -- -D warnings` | Correcto, sin errores, antes de los últimos ajustes de manejo de error y presentación. |
| `cargo build -p codeloom-cli --bin codeloom` | Correcto antes de los últimos ajustes de manejo de error y presentación. |
| `cargo build` / suite completa | No verificados: `loom.exe` estaba bloqueado por procesos externos; una ejecución aislada agotó el espacio disponible. No se detuvieron esos procesos. |
| `cargo test --workspace --exclude codeloom-cli -j 1` | Interrumpido: quedó esperando el lock del target compartido mientras los procesos anteriores seguían activos. |

La compilación y tests de los últimos ajustes siguen pendientes cuando se libere
el target compartido y haya espacio suficiente. No se afirma que se hayan
validado los criterios de integración que requieren la suite completa.

## Tareas para cerrar esta spec

- [ ] Releer los diffs actuales de la herramienta, el dispatch, la persistencia y la TUI;
  verificar cada criterio de aceptación contra el código actual.
- [x] Probar aprobación exacta, rechazo de una opción no exacta, hash cambiado y flujo durable
  reabierto con launcher simulado; comprobar que reviewer se lanza en modo solo lectura y que
  el cierre limpio aún exige aceptación final.
- [x] Probar el límite de dos rondas de corrección, el bloqueo de escrituras en dispatch y
  el arranque del reviewer a través de la interfaz de solo lectura.
- [ ] Probar intentos explícitos de escritura desde el agente reviewer y delegación iniciada
  por el padre durante un workflow activo.
- [x] Ejecutar `cargo build`, `cargo test`, `cargo fmt --check` y
  `cargo clippy -- -D warnings` sobre el árbol actual.
- [x] Registrar los resultados actuales; la suite completa no sustituye la revisión visual.

## Verificación adicional 2026-10-28

Las pruebas de integración nuevas recorren specifying → aprobación → implementación → revisión
solo lectura → aceptación final, reabren SQLite entre planner y coordinador, invalidan una
aprobación si cambia el contenido, cancelan sin lanzar implementer ante una selección inexacta,
limitan las correcciones a dos rondas y bloquean escritura antes de iniciar el workflow. El
launcher simulado reconstruye el descriptor del hijo después de abrir de nuevo el store; esto no
simula la recuperación de un proceso externo real.

Los checks globales pasaron: `cargo build` (aviso existente por `main.rs` compartido entre
`loom` y `codeloom`), `cargo test` (1.712 pasaron, 13 ignorados), `cargo fmt --check` y
`cargo clippy -- -D warnings`. Target: `C:\Users\negrero\AppData\Local\Temp\codeloom-final-target`.
