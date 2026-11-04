# Plan: uso de tokens preciso y supervisión en vivo de subagentes

> Spec: `docs/specs/2026-10-29-codeloom-precision-usage-live-subagent-supervision.md`.
> Estado: implementación y validación automatizada completas; queda la prueba interactiva con provider configurado.

## Reglas de ejecución

- Trabajar una tarea cada vez y respetar las aprobaciones del workflow padre.
- No cambiar decisiones aceptadas en las specs anteriores salvo los dos cambios
  explícitos de esta spec: streaming visible y apertura por defecto de hijos
  activos.
- `loomcoding-core` sigue sin depender de ratatui/crossterm. No usar red real en
  pruebas. Mantener límites de contexto, herramientas, concurrencia y memoria.
- En el TUI no ocultar la diferencia entre cifra confirmada, estimada y ausente.
- No commitear.

## Tareas

### 1. Inventario de contratos de uso por provider

**Archivos principales:** `crates/loomcoding-core/src/provider/{anthropic,openai,gemini}.rs`,
`crates/loomcoding-core/src/provider/mod.rs`, `crates/loomcoding-core/src/agent/mod.rs`.

- Documentar para cada adapter cuándo llega `Usage`, si sus eventos son deltas o
  snapshots acumulados, si necesita habilitar una opción para emitir uso en
  streaming y qué se conoce en error/cancelación.
- Añadir fixtures unitarios para secuencias reales representativas sin red.
- Resolver explícitamente el caso Anthropic de evento inicial más total final:
  no sumar dos snapshots del mismo campo.

**Salida:** tabla breve en esta spec o referencia enlazada desde ella y un
contrato interno normalizado antes de tocar el render.

### 2. Contrato normalizado de actualización por respuesta

**Archivos principales:** `crates/loomcoding-core/src/message/`,
`crates/loomcoding-core/src/provider/`, `crates/loomcoding-core/src/event/mod.rs`.

- Representar actualizaciones acumuladas por `request_id`, con ámbito de sesión,
  tokens de entrada/salida, origen (`provider`/`estimate`) y carácter final.
- Normalizar los adapters a snapshots acumulados; deduplicar/reemplazar según
  `request_id`, no sumar snapshots repetidos.
- Mantener la señal de que el provider todavía no ha dado uso. No sintetizar un
  valor confirmado a partir de la longitud del texto.
- Emitir cada actualización conforme llega para que UI y registros puedan
  pintarla en vivo sin esperar a `TurnEnded`.

**Tests:** adapter con varios eventos, snapshots que crecen, evento final
repetido, final que corrige estimación, provider sin uso y stream cancelado.

### 3. Registro durable y contadores de familia

**Archivos principales:** `crates/loomcoding-core/src/session/{mod.rs,sqlite.rs}`,
`crates/loomcoding-core/src/agent/mod.rs`, `crates/loomcoding-tui/src/app.rs`.

- Mantener una fila final por respuesta; ampliar el ledger para fuente,
  finalización e identidad de respuesta si hace falta reemplazar/recuperar una
  actualización provisional sin duplicar.
- Migrar SQLite sin perder sesiones ni filas existentes. Los registros viejos se
  clasifican como `provider` solo si ya tienen semántica confirmada; no inferir
  un origen no almacenado.
- Guardar cifra final confirmada; si acaba sin cifra del provider, guardar una
  estimación marcada como tal solo si el estimador estaba disponible. El error
  de persistencia no debe perder el turno.
- Atribuir cada apunte al modelo y sesión que generaron la petición. Separar
  contadores de petición, turno, sesión y familia.

**Tests:** migración sobre DB anterior, acumulación de varias llamadas por
turno, cambio de modelo, atribución padre/hijo, final confirmado que reemplaza
estimación, error de almacenamiento y recarga sin doble suma.

### 4. Registro de tokens en vivo en la TUI

**Archivos principales:** `crates/loomcoding-tui/src/app.rs`,
`crates/loomcoding-tui/src/render.rs`, `crates/loomcoding-tui/src/rail.rs`,
`crates/loomcoding-tui/src/run.rs`.

- Mostrar la petición activa y actualizar entrada/salida al recibir eventos.
- Presentar estado y fuente con etiquetas distinguibles: confirmado,
  estimación provisional o sin dato; al llegar el dato confirmado reemplazar el
  provisional de esa petición.
- Mostrar por separado turno actual, sesión y familia; mostrar cada hijo con su
  propio uso/coste en el nodo existente.
- Mantener estables los contadores al cambiar sesión, reiniciar turno,
  reconectar al servidor o completar un hijo.

**Tests:** render con cifra confirmada, provisional y ausente; reemplazo sin
doble suma; familia padre+hijos; dimensiones estrechas; reabrir sesión.

### 5. Actividad en vivo del subagente

**Archivos principales:** `crates/loomcoding-core/src/event/mod.rs`,
`crates/loomcoding-core/src/subagent.rs`, `crates/loomcoding-tui/src/app.rs`,
`crates/loomcoding-tui/src/rail.rs`, `crates/loomcoding-tui/src/render.rs`.

- Reenviar deltas de texto visibles del asistente y eventos de herramienta con
  identidad del hijo. No reenviar `ReasoningDelta`.
- Actualizar el nodo con texto, estado exacto, paso, herramienta, argumentos
  seguros y resultado/error limitado. El nodo activo se despliega por defecto;
  plegarlo conserva su resumen y estado de actividad. Enter alterna el nodo y
  las flechas derecha/izquierda lo despliegan/pliegan explícitamente.
- Asegurar que seleccionar/abrir un hijo presenta su transcript completo y no
  requiere que el padre esté llamando herramientas.
- Re-sincronizar desde mensajes/estado persistidos si el broadcast pierde
  eventos. Mantener cotas de texto, número de eventos y trabajo por frame.

**Tests:** deltas y herramienta aparecen en el hijo correcto; actualizaciones
monótonas; error visible; nodo activo por defecto, toggle Enter en ambos
sentidos, plegado todavía activo, recuperación tras lag y carga grande acotada.

### 6. Parada directa de un subagente

**Archivos principales:** `crates/loomcoding-tui/src/run.rs`,
`crates/loomcoding-tui/src/app.rs`, `crates/loomcoding-core/src/subagent.rs` y el
canal de comandos UI/runtime que ya use el cliente.

- Añadir una acción directa sobre el hijo seleccionado desde la vista del padre;
  no pasar por un prompt o llamada del modelo.
- Cancelar solo ese hijo mediante el runtime existente. Representar
  inmediatamente `stopping`, esperar estado terminal con el timeout existente y
  mostrar confirmación, rechazo o error.
- No cerrar ni cancelar hermanos ni el turno del padre. Mantener la posibilidad
  de abrir el hijo mientras termina de asentarse.

**Tests:** parada del hijo seleccionado con padre inactivo, aislamiento de
hermanos, hijo ya terminal, timeout/error de cancelación remota y actualización
del nodo al estado final.

### 7. Estado verdadero del workflow y dashboard

**Archivos principales:** `crates/loomcoding-core/src/tools/development_workflow.rs`,
`crates/loomcoding-core/src/workflow.rs`, `crates/loomcoding-tui/src/app.rs`,
`crates/loomcoding-tui/src/render.rs`.

- Hacer que `development_workflow status` consulte el estado real del hijo
  activo y devuelva una descripción accionable si ya terminó.
- Limpiar/transicionar `active_child` y fase de forma válida; un planner
  `completed` sin resultado debe acabar en `failed`/bloqueado con motivo, sin
  decir que corre ni dejar el workflow aparentemente activo.
- Mantener sincronizados dashboard, FLOCK, nodo y status con el mismo
  `SubagentDescriptor`; distinguir claramente `0 activos` de hijos terminados.

**Tests:** finalización con resultado, vacío, fallo y cancelación; el dashboard
no reporta activo a un hijo terminal; no avanza a implementación con artefactos
ausentes.

### 8. Validación integrada y prueba de terminal

- [x] Ejecutar pruebas de los crates afectados y los cuatro checks de
  `AGENTS.md`: `cargo build`, `cargo test`, `cargo fmt --check` y
  `cargo clippy -- -D warnings`.
- [x] Probar con provider falso secuencias de uso confirmado/estimado, uso ausente,
  error y cancelación; probar un hijo que emite texto, llama herramienta, falla y
  se detiene.
- [ ] En terminal Windows, comprobar un stream de verdad con etiqueta de fuente,
  expansión del nodo activo, acceso a chat, cancelación de un solo hijo y
  estados coherentes al terminar. Anotar proveedor, modelo y si el uso procede
  del provider o es estimado. Intento del 2026-10-29: el binario nuevo compiló
  en un `target-dir` aislado, pero el arranque encontró dos daemons antiguos del
  repo y falló con `protocolo local incompatible`; no se cerró ninguna sesión.
  Reintentar cuando se hayan cerrado/reiniciado las instancias activas.
- [x] Actualizar esta spec y el índice de `docs/README.md` con lo realmente
  implementado y cualquier provider que no suministre uso.

## Dependencias y orden

1 → 2 → 3 → 4. La tarea 5 puede avanzar en paralelo con 2–4 una vez cerrado el
contrato de eventos. 6 depende del control y estado ya existentes del runtime;
7 depende de estados persistidos fiables y se puede implementar en paralelo con
5–6. La tarea 8 cierra todo. No iniciar una fase de implementación hasta que la
persona usuaria apruebe esta spec y este plan.

## Estado

- [x] Revisar ledger, stream de provider, eventos de hijos, vista de árbol,
  cancelación y la sesión #6.
- [x] Preparar contrato y tareas con pruebas de aceptación.
- [x] Aprobación de la spec y el plan.
- [x] Implementación secuencial conforme a las dependencias.
- [x] Checks automatizados: suite del workspace (1.739 pasados, 14 ignorados, un hilo); tras los últimos ajustes, `loomcoding-core` + `loomcoding-tui` (1.008 pasados, 2 ignorados); `cargo build`, `cargo fmt --all -- --check` y `cargo clippy -- -D warnings`.
- [ ] Prueba interactiva en terminal Windows con el provider/modelo de la persona usuaria;
  pendiente de reiniciar los daemons activos, que rechazaron el protocolo del binario nuevo.
