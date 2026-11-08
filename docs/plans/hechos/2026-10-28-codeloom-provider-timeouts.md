# Plan — Timeout y propagación de errores del provider

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: límite de 120 s en los adapters y `StreamChunk::Error` propagado.

Spec: `docs/specs/hechos/2026-10-28-codeloom-provider-timeouts.md`.

1. [x] Añadir un límite total de 120 segundos al request de OpenAI-compatible y error claro si vence antes de las cabeceras.
2. [x] Representar el error de lectura del stream en `StreamChunk` y propagarlo desde el loop sin perder texto parcial ni ejecutar tool calls incompletas.
3. [x] Añadir una prueba local de timeout y una regresión del loop para stream fallido; verificar también los eventos de inicio y respuesta.
4. [x] Ejecutar los cuatro checks con `CARGO_TARGET_DIR` apuntando a `C:\Users\<usuario>\AppData\Local\Temp\codeloom-final-target`.
5. [x] Probar `view` con Haiku en el fixture temporal. El runtime mostró el inicio de la petición; Anthropic respondió HTTP 400 en aproximadamente 1,2 s. No hubo permiso ni llamada a `view`, así que no hay consumo atribuible a una tool ni evidencia de que las tools internas funcionen por esta ruta. No repetir PTC hasta resolver el request directo.

El cuerpo HTTP de error se mantiene fuera de los mensajes visibles; el adapter devuelve solo el estado HTTP.
