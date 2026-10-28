# Plan — Orquestación programática de tools por provider

> Estado: implementación parcial en curso; los tres adapters compilan, falta validar contratos
> con fixtures y cerrar observabilidad/medición.
>
> Spec: `docs/specs/2026-10-28-codeloom-programmatic-tool-calling.md`.
>
> La disponibilidad y el protocolo se deben volver a contrastar con documentación oficial
> al empezar cada adapter: cambian por API y modelo.

## Objetivo

Implementar la orquestación programática manteniendo los permisos, límites y dispatcher de
CodeLoom, con adapters independientes para Anthropic, OpenAI Responses y Gemini. El código
generado solo corre en el entorno hospedado del provider. La ruta normal de tools sigue
disponible y el modo empieza apagado.

## Decisiones de planificación

1. La unidad de capacidad es provider + modelo + API + conjunto de tools; no se infiere por
   compatibilidad de endpoint ni por el nombre del provider.
2. Modo opt-in con `CODELOOM_PROGRAMMATIC_TOOL_CALLING=1`, desactivado por defecto. No se
   activa en perfiles remotos, tarjetas A2A ni por respuesta del modelo.
3. Los adapters conservan sus formatos de protocolo y estado de continuación; el core solo
   coordina invocaciones, permisos, ejecución de tools y límites.
4. Las tools de escritura y las que requieren aprobación quedan direct-only salvo que un
   contrato provider permita conservar inequívocamente el gate normal antes de cada efecto.
5. OpenAI se implementa como Responses API independiente de `OpenAiCompatProvider`.
6. Gemini recibe primero una prueba de contrato: la documentación actual describe tool
   context circulation y combinación de code execution con funciones, pero hay que comprobar
   si permite el mismo patrón de orquestación de llamadas cliente desde código. No se
   anunciará PTC equivalente si la API solo ofrece combinación/secuenciación de tools.
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
- El estado privado del provider tiene un límite de 64 KiB y no forma parte del contenido
  visible de los mensajes ni del protocolo A2A.
- Verificación ejecutada: `cargo check --workspace --all-targets --target-dir .\target` pasó.
  No se han ejecutado tests ni los cuatro checks de cierre.

## Tareas

### T0. Preparar el contrato de ejecución común

- [ ] Auditar `Provider`, `ProviderRequest`, `StreamChunk`, `AgentLoop`, dispatch, historial,
  permisos y representación de resultados de tools en el árbol vigente.
- [ ] Definir el mínimo protocolo interno para una invocación hospedada, tools pendientes,
  resultados, continuación opaca y salida final; no normalizar payloads propios de cada API.
- [ ] Añadir opt-in de configuración apagado por defecto y selección por request sin alterar
  la API normal de tool calling.
- [ ] Definir límites de tiempo, llamadas totales, concurrencia, bytes, tokens y cancelación.
- [ ] Mantener en CodeLoom la validación, `PermissionService`, `ExecutionMode`, eventos y
  registro de resultados de cada tool del cliente.
- [ ] Añadir tests con providers simulados para secuencias, reanudación, denegación,
  cancelación, timeout y fallback; ningún test usa red real.

### T1. Anthropic Messages API

- [ ] Verificar en documentación vigente versiones de `code_execution`, `allowed_callers`,
  `caller`, `container`, expiración y forma de continuar tool results.
- [ ] Comprobar soporte para el modelo seleccionado mediante la señal de capacidad oficial;
  no fijar una lista global de modelos en el core.
- [ ] Extender `AnthropicProvider` para preservar bloques, IDs, caller y container entre
  pausas; manejar respuestas parciales y expiradas sin repetir efectos.
- [ ] Calcular allowlist solo para tools compatibles. MCP Connector queda excluido para este
  adapter; las tools propias de CodeLoom requieren gates por llamada.
- [ ] Añadir pruebas con eventos/respuestas grabadas y errores simulados, incluyendo modelo
  no compatible, schema no elegible, denegación y expiración.

### T2. OpenAI Responses API

- [ ] Auditar las structs y configuración actuales; implementar `ResponsesProvider` separado
  del adapter OpenAI Chat Completions compatible.
- [ ] Verificar en documentación vigente los items `program`, `function_call` con `caller`,
  `program_output`, continuación, estado y límites por modelo.
- [ ] Implementar el ciclo de continuación copiando el `caller` requerido y conservando la
  secuencia original; ejecutar en CodeLoom solo function calls propiedad del cliente.
- [ ] No habilitar hosted tools de shell, MCP, browser u otras herramientas provider en este
  primer alcance; solo las tools de CodeLoom elegibles y con autorización propia.
- [ ] Añadir pruebas de parsing/stream, respuestas almacenadas y stateless, calls pendientes,
  errores, fallback y permisos con fixtures locales.

### T3. Gemini API

- [ ] Verificar si la API/SDK vigente permite que el código ejecutado por Gemini invoque
  funciones de cliente, o si solo combina tools integradas y function calling por circulación
  de contexto.
- [ ] Si hay contrato equivalente, añadir `GeminiProvider` separado con preservación exacta de
  `id`, firmas/contexto cifrado y todos los pasos requeridos por la API.
- [ ] Si no hay contrato equivalente, cerrar esta tarea como capacidad no soportada para PTC,
  documentar lo que sí soporta Gemini y dejar el tool calling normal intacto.
- [ ] Para cualquier adapter, comprobar la lista vigente de modelos y restricciones; la
  combinación de tools actualmente documentada como preview no se marca estable por defecto.
- [ ] Añadir pruebas de fixtures para continuación y pérdida/alteración de firma/contexto; sin
  red real.

### T4. Observabilidad y fallback

- [ ] Mostrar provider, modelo y motivo de fallback sin imprimir código generado, credenciales
  ni payloads sensibles.
- [ ] Diferenciar en eventos y TUI una llamada directa de una llamada originada en ejecución
  programática, conservando IDs para diagnóstico.
- [ ] Verificar que error, timeout, denegación y pérdida de continuación no autoaprueban ni
  reintentan tools con efectos laterales.
- [ ] Documentar por adapter las capacidades verificadas y sus límites.

### T5. Evaluación de ahorro y calidad

- [ ] Comparar llamadas directas frente a programáticas en tareas equivalentes, separado por
  provider, modelo, API y tipo de tool.
- [ ] Medir tokens, latencia total y de provider, pasos del modelo, bytes intermedios,
  llamadas, errores y cumplimiento de permisos.
- [ ] Definir antes de ejecutar el conjunto de tareas y el criterio de calidad; no afirmar
  mejora de calidad por menor coste o latencia.

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

- Spec: `docs/specs/2026-10-28-codeloom-programmatic-tool-calling.md`.
- Evidencia inicial: `docs/references/2026-10-28-programmatic-tool-calling-providers.md`.
- Providers actuales: `crates/codeloom-core/src/provider/{mod,anthropic,openai}.rs`.
- Loop y dispatch: `crates/codeloom-core/src/agent/`.

## Fuera de estas tareas

- Ejecutar código generado localmente o añadir un sandbox propio.
- Cambiar A2A/AgentCard para representar esta capacidad.
- Declarar compatibilidad de PTC por `OpenAiCompatProvider`.
- Autoaprobar una tool porque el provider la invoque desde su entorno.
- Afirmar equivalencia de protocolo entre Anthropic, OpenAI y Gemini.
