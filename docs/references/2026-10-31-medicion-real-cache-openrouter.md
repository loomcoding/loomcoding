# Medición real de la caché de prompts con Haiku 4.5 por OpenRouter

> Fecha: 2026-10-31. Autorizada por el usuario con tope de 0,50 USD; gasto real ≈ 0,033 USD.
> Prueba: `openrouter_haiku_live_cache_usage_measurement` en `crates/codeloom-core/src/provider/anthropic.rs`
> (`#[ignore]`; necesita `OPENROUTER_API_KEY` en el entorno del proceso y gasta saldo).
> Modelo: `anthropic/claude-haiku-4.5`, ruta compatible con Anthropic (`/api/v1/messages`).

## Qué se midió

Cuatro peticiones con el mismo prefijo de sistema de unos 9 700 tokens y salida de 4 tokens. Dos por el adaptador
**tal como está hoy** (sin `cache_control`) y dos con `cache_control` puesto a mano en el bloque de sistema.

## Resultados

| Petición | `input_tokens` | Caché comunicada | Coste comunicado |
|---|---|---|---|
| Adaptador actual, 1.ª | 9 673 | lectura 0, escritura 0 | 0,009693 $ |
| Adaptador actual, 2.ª (mismo prefijo) | 9 673 | lectura 0, escritura 0 | 0,009693 $ |
| Con `cache_control`, 1.ª | 12 | **escritura 9 661** | 0,012108 $ (+25 % sobre sin caché) |
| Con `cache_control`, 2.ª | 12 | **lectura 9 661** | 0,000998 $ (≈ 10 % de sin caché) |

## Lectura

- **Hoy no hay caché.** Sin `cache_control` el proveedor no cachea el prefijo repetido: la 2.ª petición cuesta lo
  mismo que la 1.ª. Es un resultado medido, no una suposición.
- **Con un punto de caché, la 2.ª petición cuesta ≈ 90 % menos** en este prefijo; la 1.ª cuesta ≈ 25 % más. Con
  este tamaño compensa a partir de la segunda petición. Es **un caso**: un prefijo, un modelo, un proveedor.
- **Anthropic excluye la caché de `input_tokens`** (12 frente a 9 661 en caché): confirmado con datos reales, como
  decía la spec de A1.
- **Nuestro lector de caché entiende la forma real** (`cache_creation_input_tokens`, `cache_read_input_tokens`) en
  respuesta sin streaming. En streaming solo se comprobó con valores a cero: el proveedor informa los campos
  explícitamente a 0, de modo que en OpenRouter un 0 **sí** puede ser una medición y no solo «no informado».
  La spec de A1 dice que cero significa «no informado o sin caché»; sigue siendo válida como cota segura, pero
  conviene matizarla.
- No se midió la ruta compatible con OpenAI de OpenRouter, ni DeepSeek, ni caché con herramientas, ni TTL de 1 h.

## Consecuencias

B1 (puntos de caché en el adaptador de Anthropic) queda justificada con datos propios para prefijos grandes y
estables. No se ha implementado: necesita spec (dónde colocar los puntos, qué prefijo es estable, qué pasa si el
sistema cambia entre turnos, y el coste de escritura en sesiones cortas).
