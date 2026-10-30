# Spec: límites y errores de petición del provider

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: límite de 120 s en los adapters y `StreamChunk::Error` propagado.

## Objetivo

Evitar que el agente espere indefinidamente una respuesta del provider o convierta un fallo del stream en una respuesta normal.

## Contrato

- La petición OpenAI-compatible tiene un límite total de 120 segundos, coherente con los adapters existentes.
- Si falla antes de recibir cabeceras, el error del provider indica que no se obtuvo respuesta.
- Si falla el stream después de recibir cabeceras, el loop lo devuelve como error y conserva el texto parcial ya recibido.
- Un stream incompleto no ejecuta tool calls, aunque haya llegado alguna llamada antes del fallo.
- El runtime emite eventos locales cuando empieza la petición y cuando el adapter devuelve una respuesta; si solo aparece el primero, no llegó una respuesta al adapter.
- Los mensajes de error no incluyen credenciales ni el cuerpo de error crudo del provider.

## Aceptación

- Un mock local que retrasa la respuesta produce timeout en vez de quedarse esperando.
- Un error de stream se propaga al loop; se conserva texto parcial y no se despachan tools.
- Pasan `cargo build`, `cargo test`, `cargo fmt --check` y `cargo clippy -- -D warnings`.
