# Plan — Registro de tokens de caché en el ledger (fase 1)

> Estado: fase 1 implementada (2026-10-31). Salvedad: la API Responses de OpenAI no se ha cubierto (seguimiento).
> Spec: `docs/specs/2026-10-31-codeloom-cache-usage-ledger.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`.
- Sin red real en las pruebas: proveedor simulado.
- No se añaden campos a `Usage` ni a `UsageUpdate`.

## Tareas

1. **Tipo y fragmento.** Añadir `CacheUsage { read_tokens: u64, write_tokens: u64 }` y `StreamChunk::CacheUsage` en
   `provider/mod.rs`. Compilar para ver qué `match` exhaustivos exigen el variante nuevo.
2. **Anthropic.** Prueba primero: `message_start` con `cache_read_input_tokens` y `cache_creation_input_tokens` produce
   el fragmento; sin ellos, no. Ídem para la respuesta sin streaming y para `message_delta` si trae los campos.
3. **Compatible con OpenAI.** Prueba primero: `cached_tokens`, `cache_write_tokens` y `prompt_cache_hit_tokens`.
4. **Apunte.** `UsageRecord` con los dos campos y valor por defecto 0 al deserializar. Actualizar todos los
   constructores que el compilador señale.
5. **SQLite.** Migración aditiva de dos columnas, escritura y lectura. Pruebas: ida y vuelta, y migración de una base
   antigua creada con el esquema previo.
6. **Agente.** Acumular el máximo de cada contador durante el flujo y pasarlos a `record_usage`. Prueba de extremo a
   extremo con un proveedor simulado que emita el fragmento.
7. **Cierre.** Cuatro comprobaciones, actualizar este plan y el inventario, y commit.

## Fuera de esta fase

Desglose del contexto, presentación en la TUI, puntos de caché y cálculo de coste con caché.

## Verificación

- Cada criterio de aceptación de la spec tiene al menos una prueba que falla antes del cambio.
- Se comprueba a mano que las pruebas «muerden»: al quitar la lectura del campo, la prueba correspondiente falla.
