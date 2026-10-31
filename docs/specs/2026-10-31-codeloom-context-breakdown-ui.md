# CodeLoom — Presentación del desglose del contexto y de la caché (A1, fase 3)

> Estado: **implementado** (2026-10-31).
>
> Fecha: 2026-10-31
>
> Origen: fase 3 de `2026-10-31-codeloom-cache-usage-ledger.md` (A1). Parte de la fase 1 (tokens de caché en el
> ledger) y de la fase 2 (`compact::breakdown_request`).
> Relacionado: `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.
>
> Patrones afines (agentpatterns.ai, CC BY 4.0):
> [The Four Terms That Decide What an Agent Task Costs](https://agentpatterns.ai/token-engineering/four-term-agent-cost-model/),
> [Reducing System-Prompt Token Bloat](https://agentpatterns.ai/context-engineering/system-prompt-bloat-reduction/).

## 1. Problema

El panel «Token usage» muestra tokens de turno, sesión y familia, y la petición activa. No dice de qué se compone el
contexto (sistema, herramientas, mensajes) ni cuánto del prompt sirvió la caché del proveedor. Los datos ya existen:
el ledger guarda lectura y escritura de caché por apunte, y `breakdown_request` desglosa la petición, pero ninguno
llega a la pantalla.

## 2. Objetivo

Mostrar, en el panel «Token usage», dos líneas **solo cuando hay dato**:

- Desglose estimado de la última petición propia: `Context ~T · sys S · tools H · msgs M`.
- Caché acumulada en la sesión, tal como la comunica el proveedor: `Cache read R · write W`.

## 3. No objetivos

- No cambia ningún cálculo de coste ni de presupuesto.
- No se normaliza `input_tokens`: la lectura de caché se muestra aparte, con la semántica del proveedor.
- No se separan las herramientas MCP de las propias en el desglose (ya constan por servidor en su panel).
- No se muestra el desglose de los subagentes: solo el de la sesión que se está viendo.
- Esto es telemetría local; no representa estados ni semántica A2A.

## 4. Diseño

### 4.1 Evento

Variante nueva `LiveEvent::ContextBreakdown { request_id, breakdown }`, publicada una vez por petición, justo tras
calcular la estimación y antes de enviarla. Se añade una variante en lugar de ampliar `UsageProgress` para no tocar
sus nueve puntos de uso. `ContextBreakdown` ya es serializable.

Riesgo asumido: un cliente de una versión anterior no conoce la variante nueva. Demonio y cliente salen del mismo
binario, de modo que es el mismo riesgo que ya tenía cualquier evento nuevo.

### 4.2 Estado de la interfaz

`App` guarda el último desglose recibido de la sesión visible (`Option<ContextBreakdown>`); se descarta al cambiar de
sesión. La caché se obtiene sumando `cache_read_tokens` y `cache_write_tokens` de `usage_records`, sin estado nuevo.

### 4.3 Presentación

Funciones puras en `render.rs`:

- `context_breakdown_line(&ContextBreakdown) -> String`.
- `cache_usage_line(&[UsageRecord]) -> Option<String>`: `None` si ambos acumulados son cero, porque cero significa
  «no informado o sin caché», no una medición.

El panel mide hoy 6 filas (4 de contenido). Cada línea con dato añade una fila a su altura; sin dato, el panel no
cambia. Con poco alto de terminal el panel inferior conserva su mínimo de 3 filas y se recortan primero las líneas
nuevas.

## 5. Criterios de aceptación

1. `ContextBreakdown` se publica una vez por petición con las cuatro partes y el mismo total que la estimación.
2. La línea de desglose muestra total y las tres partes principales.
3. Sin tokens de caché registrados no hay línea de caché; con ellos, muestra lectura y escritura sumadas.
4. El panel crece una fila por línea con dato y no cambia sin datos.
5. Cambiar de sesión descarta el desglose anterior.
6. Las cuatro comprobaciones del proyecto pasan.

## 6. Riesgos y preguntas abiertas

- Si ningún proveedor activo informa caché, la línea no aparecerá nunca; es un resultado válido, no un fallo.
- Los números de lectura (Anthropic excluye la caché de `input_tokens`; los compatibles con OpenAI la incluyen) no son
  comparables entre proveedores: la línea no sugiere una tasa de acierto.
- Una pantalla muy baja puede ocultar las líneas nuevas; es deliberado.
