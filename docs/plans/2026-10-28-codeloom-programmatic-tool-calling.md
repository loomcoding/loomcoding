# Plan — Orquestación programática de tools por provider

> Estado: adapters y pruebas locales implementados; checks completos pasados. Queda la evaluación comparativa con modelos reales.
>
> Spec: `docs/specs/2026-10-28-codeloom-programmatic-tool-calling.md`.
>
> La disponibilidad y el protocolo se deben volver a contrastar con documentación oficial
> al empezar cada adapter: cambian por API y modelo.

## Objetivo

Implementar la orquestación programática manteniendo los permisos, límites y dispatcher de
Loomcoding, con adapters independientes para Anthropic, OpenAI Responses y Gemini. El código
generado solo corre en el entorno hospedado del provider. La ruta normal de tools sigue
disponible y el modo empieza apagado.

## Decisiones de planificación

1. La unidad de capacidad es provider + modelo + API + conjunto de tools; no se infiere por
   compatibilidad de endpoint ni por el nombre del provider.
2. Modo opt-in con `LOOMCODING_PROGRAMMATIC_TOOL_CALLING=1`, desactivado por defecto. No se
   activa en perfiles remotos, tarjetas A2A ni por respuesta del modelo.
3. Los adapters conservan sus formatos de protocolo y estado de continuación; el core solo
   coordina invocaciones, permisos, ejecución de tools y límites.
4. Las tools de escritura y las que requieren aprobación quedan direct-only salvo que un
   contrato provider permita conservar inequívocamente el gate normal antes de cada efecto.
5. OpenAI se implementa como Responses API independiente de `OpenAiCompatProvider`.
6. Gemini recibe primero una prueba de contrato: la documentación actual confirma combinación
   de code execution y function calling, pero no documenta que el código Python invoque las
   funciones cliente. Se ofrece como tool combination, sin etiquetarlo como PTC equivalente.
7. Sin evidencia verificable para modelo/API/tools, el adapter cae al bucle normal y explica
   la razón; nunca amplía permisos como fallback.
8. `GeminiProvider` ofrece combinación nativa de code execution y function calling para los
   modelos documentados; no se presenta como ejecución de funciones cliente desde código.

## Avance implementado

- El opt-in llega al request del provider; la allowlist inicial solo incluye `view`, `glob`,
  `grep` y `ls`, y excluye herramientas MCP.
- Anthropic Messages consulta la capacidad por modelo, usa continuación con container y
  conserva bloques opacos localmente; si no confirma soporte, usa llamadas directas.
- OpenAI Responses es un adapter separado; conserva items de continuación con `store:false`,
  caller IDs y fallback a function calls directas.
- Gemini Interactions conserva la continuación y solo añade `code_execution` en modelos Gemini
  3/4; la llamada a funciones cliente sigue el flujo normal de la API.
- El estado privado del provider tiene un límite de 64 KiB, vive solo en memoria durante el
  turno activo y no se serializa a SQLite, TUI ni protocolo A2A. Se reatacha tras cada recarga
  durable y conserva varias pausas dentro del mismo turno.
- La respuesta del runtime limita a 32 las tool calls y 64 por turno; el estado opaco acumulado
  no puede superar 256 KiB. Los adapters rechazan
  una llamada programática a una tool fuera de la allowlist; OpenAI Compat avisa y sigue directo.
- La suite completa pasó el 2026-10-28: 1698 tests pasaron y 13 quedaron ignorados.
- Hay pruebas locales de contratos, continuidad, elegibilidad, denegación y fallback (Anthropic
  y Gemini); no se hacen llamadas a providers reales en la suite.

## Tareas

### T0. Preparar el contrato de ejecución común

- [x] Auditar `Provider`, `ProviderRequest`, `StreamChunk`, `AgentLoop`, dispatch, historial,
  permisos y representación de resultados de tools en el árbol vigente.
- [x] Definir el mínimo protocolo interno para una invocación hospedada, tools pendientes,
  resultados, continuación opaca y salida final; no normalizar payloads propios de cada API.
- [x] Añadir opt-in de configuración apagado por defecto y selección por request sin alterar
  la API normal de tool calling.
- [x] Definir límites de tiempo, llamadas totales, concurrencia, bytes, tokens y cancelación.
- [x] Mantener en Loomcoding la validación, `PermissionService`, `ExecutionMode`, eventos y
  registro de resultados de cada tool del cliente.
- [x] Añadir tests locales para contratos, reanudación, denegación, elegibilidad y fallback;
  no se hacen llamadas a providers reales.
- [x] Cubrir fallback de OpenAI Responses con fixture local.
- [ ] Añadir casos específicos de cancelación/timeout.

### T1. Anthropic Messages API

- [x] Verificar en documentación vigente versiones de `code_execution`, `allowed_callers`,
  `caller`, `container`, expiración y forma de continuar tool results.
- [x] Comprobar soporte para el modelo seleccionado mediante la señal de capacidad oficial;
  no fijar una lista global de modelos en el core.
- [x] Extender `AnthropicProvider` para preservar bloques, IDs, caller y container entre
  pausas; manejar respuestas parciales y expiradas sin repetir efectos.
- [x] Calcular allowlist solo para tools compatibles. MCP Connector queda excluido para este
  adapter; las tools propias de Loomcoding requieren gates por llamada.
- [x] Añadir pruebas con respuestas simuladas para modelo no compatible, schema no elegible y
  fallback a llamadas directas.
- [x] Probar expiración de container sin reintentar la petición como llamada directa.
- [ ] Añadir pruebas de cancelación y denegación de permiso en el loop completo.

### T2. OpenAI Responses API

- [x] Auditar las structs y configuración actuales; implementar `ResponsesProvider` separado
  del adapter OpenAI Chat Completions compatible.
- [x] Verificar en documentación vigente los items `program`, `function_call` con `caller`,
  `program_output`, continuación, estado y límites por modelo.
- [x] Implementar el ciclo de continuación copiando el `caller` requerido y conservando la
  secuencia original; ejecutar en Loomcoding solo function calls propiedad del cliente.
- [x] No habilitar hosted tools de shell, MCP, browser u otras herramientas provider en este
  primer alcance; solo las tools de Loomcoding elegibles y con autorización propia.
- [x] Añadir pruebas de parsing, estado stateless, continuación/caller y rechazo de calls a
  tools no elegibles.
- [x] Añadir prueba de fallback si la API rechaza el modo programático.
- [ ] Añadir pruebas de cancelación/timeout y permisos del loop con fixtures locales.

### T3. Gemini API

- [x] Verificar si la API/SDK vigente permite que el código ejecutado por Gemini invoque
  funciones de cliente, o si solo combina tools integradas y function calling por circulación
  de contexto.
- [x] No hay contrato documentado para invocar funciones cliente desde el código Python. Se
  implementó `GeminiProvider` para la combinación nativa y el function calling normal.
- [x] Documentar que Gemini no ofrece PTC equivalente en esta integración; el tool calling
  normal sigue activo.
- [x] Comprobar la restricción vigente: combinación nativa Preview, documentada para Gemini 3/4.
- [x] Añadir pruebas de continuación y fallback a llamadas directas para modelos fuera de las
  familias conocidas y tras un rechazo del API; sin red real.
- [ ] Añadir fixtures de pérdida/alteración de firma/contexto en la continuación.

La combinación nativa de Gemini se mantiene Preview; no se anuncia como estable.

### T4. Observabilidad y fallback

- [x] Mostrar provider/modelo y motivo de fallback sin imprimir código generado, credenciales
  ni payloads sensibles.
- [x] Diferenciar llamadas programáticas de tool calls directas con avisos locales e IDs.
- [x] Mantener las tools programáticas en allowlist de solo lectura y no reintentar efectos
  laterales al fallar una continuación.
- [x] Documentar por adapter las capacidades verificadas y sus límites.

### T5. Evaluación de ahorro y calidad

- [ ] Comparar llamadas directas frente a programáticas en tareas equivalentes, separado por
  provider, modelo, API y tipo de tool.
- [ ] Medir tokens, latencia total y de provider, pasos del modelo, bytes intermedios,
  llamadas, errores y cumplimiento de permisos.
- [ ] Definir antes de ejecutar el conjunto de tareas y el criterio de calidad; no afirmar
  mejora de calidad por menor coste o latencia.

T5 no es una prueba de software: requiere elegir modelos/API, preparar un conjunto equivalente
y autorizar presupuesto para llamadas facturables. No se han ejecutado llamadas reales ni se
afirma ahorro medido.

### Retomar: prueba de tool interna normal con OpenRouter (2026-10-28)

Esta prueba buscaba verificar el tool calling normal de Loomcoding con `view`; **no era una prueba
de PTC ni cuenta como evaluación T5**.

- Configuración: `nvidia/nemotron-3.5-super-vl:free` por `OpenAiCompatProvider` y OpenRouter.
- `loom --print` llegó al modelo, pero Loomcoding rechazó `view` con `permission denied`: el modo
  no interactivo usa `DenyAll` cuando no se solicita `--yolo`.
- En TUI, tanto el selector como `/new` devolvieron `session is outside this agent`. Se
  reemplazó el daemon `target/release/loom.exe` por el daemon debug actual tras comprobar que
  no tenía subagentes activos ni procesos hijos; el error de sesión persistió.
- No se confirmó ninguna ejecución de tool. Las pruebas locales `runs_tool_then_finishes` y
  `view_prints_line_numbers` pasaron, pero usan un provider simulado y no demuestran integración
  con OpenRouter.
- Las dos últimas filas visibles del libro de consumo para este modelo registran 4.549/497 y
  4.548/290 tokens de entrada/salida, con coste reportado `$0.00` cada una. Son llamadas del
  modelo `:free`; no son una medición de coste para otros modelos.

**Siguiente paso:** resolver primero el ownership/selección de sesión de la TUI; después abrir
una sesión nueva y repetir con una petición acotada que invoque solo `view` sobre un archivo
público del repo. Comprobar el evento/resultado de la tool y el apunte de consumo. Mantener
`--yolo` desactivado. No contabilizar esta prueba como PTC ni como ahorro medido.

### Reintento tras corregir ownership de sesión (2026-10-28)

- El runtime actualizado abre la sesión actual vacía y el listado de sesiones sigue mostrando solo
  sesiones con mensajes. La regresión local cubre sesión propia vacía y sesión ajena.
- Con el binario recién compilado y el daemon correcto se envió una petición de solo lectura con
  `nvidia/nemotron-3.5-super-vl:free` para que usara `view`. El provider no respondió tras más de
  tres minutos; no hubo diálogo de permiso ni evento de tool. Se canceló la petición.
- La sesión de la prueba no tiene apuntes de consumo en SQLite; esto no demuestra coste cero ni
  confirma que OpenRouter recibiera la petición.
- El test live del adapter sí repitió el par directo/PTC (detalle abajo), pero no hubo resultado
  útil de tool ni comparación de calidad. La prueba de tool interna en TUI quedó pendiente; no
  tratar el test del adapter como integración completa del runtime.
- Repetición directa/PTC con el adapter Anthropic: directa 402/135 tokens, 1.260 ms, sin tool y
  exactitud falsa; PTC rechazada en 52 ms, sin tokens de respuesta ni tool. Estimación de la directa:
  USD 0,000108. El test no captura el código HTTP de este rechazo ni el coste facturado real.
- Una prueba adicional en TUI usó Haiku 5.5 y un README temporal como fixture para invocar la tool
  interna `view`; se quedó esperando al provider, no mostró permiso ni tool y se canceló. La sesión
  de esa prueba no tiene apunte de consumo en SQLite. No se cuenta como ejecución de tool.

## Orden y criterio de cierre

Orden: T0 → T1; T2 y T3 pueden empezar cuando T0 estabilice el límite de integración. T4
acompaña cada adapter. T5 se hace cuando al menos una ruta programática esté disponible.

Cada tarea de provider se cierra con documentación oficial vigente, fixtures de protocolo,
tests sin red, fallback seguro y compatibilidad comprobada para modelo/API/tool. Los cuatro
checks del repositorio deben pasar antes de cerrar el conjunto:

```text
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

## Archivos de entrada

## Verificación 2026-10-28

Los cuatro checks volvieron a pasar tras completar los casos locales: `cargo build` (aviso
existente de `main.rs` compartido entre los bins `loom` y `loomcoding`), `cargo test` (1.712
pasaron, 13 ignorados), `cargo fmt --check` y `cargo clippy -- -D warnings`. Se usó el target
temporal `C:\\Users\\<usuario>\\AppData\\Local\\Temp\\loomcoding-final-target`. La evaluación T5
sigue pendiente; la combinación Gemini permanece Preview.

- Spec: `docs/specs/2026-10-28-codeloom-programmatic-tool-calling.md`.
- Evidencia inicial: `docs/references/2026-10-28-programmatic-tool-calling-providers.md`.
- Providers actuales: `crates/loomcoding-core/src/provider/{mod,anthropic,openai}.rs`.
- Loop y dispatch: `crates/loomcoding-core/src/agent/`.

## Fuera de estas tareas

- Ejecutar código generado localmente o añadir un sandbox propio.
- Cambiar A2A/AgentCard para representar esta capacidad.
- Declarar compatibilidad de PTC por `OpenAiCompatProvider`.
- Autoaprobar una tool porque el provider la invoque desde su entorno.
- Afirmar equivalencia de protocolo entre Anthropic, OpenAI y Gemini.
