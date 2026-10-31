# CodeLoom — Registro de tokens de caché en el ledger de uso

> Estado: **borrador para implementar la fase 1** (registro). Las fases 2 y 3 (desglose del contexto y su
> presentación) quedan definidas pero sin diseño de detalle.
>
> Fecha: 2026-10-31
>
> Origen: candidata A1 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
> Relacionado: `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.
>
> Patrones afines (agentpatterns.ai, CC BY 4.0):
> [The Four Terms That Decide What an Agent Task Costs](https://agentpatterns.ai/token-engineering/four-term-agent-cost-model/),
> [Reducing System-Prompt Token Bloat](https://agentpatterns.ai/context-engineering/system-prompt-bloat-reduction/).

## 1. Problema

El ledger de uso guarda tokens de entrada, de salida y coste comunicado. No distingue cuántos tokens de entrada se
sirvieron desde la caché del proveedor ni cuántos se escribieron en ella. Sin ese dato no puede saberse si hoy hay
ahorro por caché ni cuánto se ahorraría al activarla (candidatas B1 y B2). Además, con la caché de Anthropic el campo
`input_tokens` **excluye** los tokens leídos y escritos, de modo que el total real del prompt no está recogido.

## 2. Objetivo

Registrar, por cada apunte de uso, los tokens de caché **tal como los comunica el proveedor**, sin cambiar el
comportamiento del agente, el coste calculado ni la interfaz.

## 3. No objetivos

- No se activa la caché del proveedor ni se colocan puntos de caché (candidata B1).
- No se normaliza ni se reescribe `input_tokens`; las dos semánticas se documentan, no se unifican.
- No se calcula coste a partir de los tokens de caché; el coste sigue siendo el comunicado o el derivado de precios de
  entrada y salida.
- No se tocan los registros de compactación ni la estimación local.
- Gemini queda fuera de esta fase (su campo es distinto y no se ha comprobado).

## 4. Diseño

### 4.1 Semántica de los campos

Se añaden al apunte dos contadores independientes:

- `cache_read_tokens`: tokens de entrada servidos desde la caché.
- `cache_write_tokens`: tokens de entrada escritos en la caché.

Diferencia entre proveedores, que se **conserva tal cual**:

| Proveedor | `input_tokens` | Caché |
|---|---|---|
| Anthropic | excluye lectura y escritura | `cache_read_input_tokens`, `cache_creation_input_tokens` |
| Compatible con OpenAI | incluye los tokens cacheados | `prompt_tokens_details.cached_tokens`; `cache_write_tokens` si lo informa OpenRouter; `prompt_cache_hit_tokens` de DeepSeek |

Por eso no se suman ni se restan al guardar: cualquier agregado posterior tendrá que conocer el origen del dato.

### 4.2 Transporte

Un fragmento nuevo de flujo, `StreamChunk::CacheUsage`, lleva los dos contadores acumulados. No se añaden campos a
`Usage` ni a `UsageUpdate` para no alterar los muchos sitios que los construyen. Los adaptadores emiten el fragmento
solo si el proveedor informó algún campo de caché; sin informe, el agente guarda ceros.

### 4.3 Persistencia

`UsageRecord` gana `cache_read_tokens` y `cache_write_tokens` (`u64`, con valor por defecto 0 al deserializar). La tabla
`usage` recibe dos columnas con migración aditiva (`INTEGER NOT NULL DEFAULT 0`), como las anteriores. Las filas
antiguas se leen con ceros: **cero significa «no informado o sin caché»**, no una medición.

### 4.4 Acotación

Los contadores se saturan al convertir a entero de SQL con la misma función que ya valida los demás. Un valor no
numérico, negativo o ausente en la respuesta del proveedor se ignora. No hay nada sin cota que llegue al modelo.

## 5. Fases siguientes (sin diseño cerrado)

- **Fase 2 (hecha en parte, 2026-10-31):** `compact::breakdown_request` desglosa una petición en sistema, herramientas,
  mensajes y formato, y su total es exactamente la estimación que ya usaba el agente. Queda fuera llevarlo al evento de
  uso y a la TUI (fase 3), y separar las herramientas MCP de las propias en el desglose (hoy las MCP ya constan por
  servidor en los componentes del apunte). No hay una «memoria» aparte que desglosar: los ficheros de contexto del proyecto viajan dentro del sistema.
- **Fase 3:** presentación en la TUI y mostrar lectura y escritura de caché en el resumen de uso. Toca la interfaz,
  de modo que necesita su propia spec.

## 6. Criterios de aceptación

1. Un mensaje de inicio de Anthropic con campos de caché produce un fragmento con esos valores.
2. Una respuesta de Anthropic sin streaming con campos de caché los conserva.
3. Una respuesta compatible con OpenAI con `cached_tokens` produce lectura; con `cache_write_tokens`, escritura;
   con `prompt_cache_hit_tokens` (DeepSeek), lectura.
4. Sin campos de caché no se emite el fragmento y los apuntes guardan ceros.
5. El apunte con valores de caché se guarda y se lee de SQLite con los mismos valores.
6. Una base antigua sin las columnas se migra sin perder filas y las lee con ceros.
7. De extremo a extremo con un proveedor simulado, los valores llegan al ledger del agente.
8. Las cuatro comprobaciones del proyecto pasan.

## 7. Riesgos y preguntas abiertas

- La forma exacta que devuelve OpenRouter se ha tomado de su documentación y de otros adaptadores; **no está verificada
  con una llamada real**. Los tests usan respuestas simuladas.
- Si ningún proveedor activo informa caché, la fase solo demuestra que hoy no se mide; es un resultado válido.
- Un cambio de esquema afecta a los dos procesos (demonio y cliente) si usan versiones distintas del binario; la
  migración es aditiva, de modo que un binario antiguo sigue funcionando con la tabla ampliada.
