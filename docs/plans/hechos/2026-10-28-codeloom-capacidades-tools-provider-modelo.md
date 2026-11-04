# Plan — Capacidades de tools por provider y modelo

> Estado: **hecho** (2026-11-04). Registro puro de capacidades por destino, clasificador de rechazos, parseo de metadata de OpenRouter y cableado en los adapters de Anthropic, Responses y Gemini. La obtención de metadata por red queda fuera de alcance a propósito (paso 3; véase «Avance»).

Spec: `docs/specs/hechos/2026-10-28-codeloom-capacidades-tools-provider-modelo.md`.

1. [x] Auditar la metadata actual de Anthropic, OpenRouter y adaptadores, contrastando cada campo con su API documentada.
2. [x] Definir el tipo mínimo de estado/evidencia en `loomcoding-core` sin convertir la capability wire del adapter en una propiedad global del modelo.
3. [x] Implementar parseo/caché de metadata solo para endpoints oficiales que den evidencia utilizable; desconocido se conserva como `unknown`.
4. [x] Enrutar solicitudes según la combinación exacta; separar tools directas, streaming, PTC y herramientas del provider.
5. [x] Clasificar rechazos sin cuerpos crudos y permitir fallback únicamente ante incompatibilidad explícita.
6. [x] Añadir pruebas de fixtures para destinos distintos, metadata ausente/expirada, rechazo de capacidad y errores de auth/cuota; sin red.
7. [x] Actualizar evidencia y ejecutar los cuatro checks del repositorio.

## Avance (2026-10-31)

Código: `crates/loomcoding-core/src/capability.rs` (registro puro, sin red) y su uso en
`provider/anthropic.rs`.

- **Auditoría (1).** El catálogo de models.dev solo aporta `tool_call` y `reasoning` como booleanos
  y descarta los modelos sin `tool_call`: es evidencia de tipo «catálogo», no de una ruta. La
  `Capabilities` de `provider/mod.rs` es estática por adapter. La caché de `code_execution` de
  Anthropic estaba indexada solo por modelo. En OpenRouter, `GET /api/v1/models` da
  `supported_parameters` agregados y `GET /api/v1/models/{id}/endpoints` los da por endpoint
  (`provider_name`, `tag`); Haiku 4.5 tenía endpoints de Anthropic, Amazon Bedrock, Google y Azure.
- **Tipo (2).** `Capability` (siete funciones independientes), `Target` (protocolo, URL base
  normalizada, modelo y ruta fijada), `State` (`Supported`, `Unsupported`, `Unknown`,
  `Rejected { status }`), `Source` y `CapabilityRegistry` con caducidad. Lo que el adapter no
  implementa es `Unsupported` aunque la metadata diga lo contrario.
- **Metadata (3), hecho con alcance acotado.** Hay parseo de ambos formatos de OpenRouter con fixtures; el catálogo
  agregado solo prueba que alguna ruta acepta `tools`, y un endpoint fijado con parámetros
  discrepantes devuelve «sin dato». Cerrado sin obtención de red ni caché en disco: la spec prohíbe el sondeo y pide `unknown` cuando
  no hay metadata documentada; el registro ya acepta evidencia de metadata (`record`) con caducidad, y
  el rechazo real de una petición cubre el resto. Si algún día se quiere descargar metadata de
  OpenRouter, será un plan nuevo.
- **Enrutado (4), hecho (2026-11-04).** Los adapters de Anthropic, Responses y Gemini consultan el
  registro por destino exacto (protocolo, URL base y modelo) y clasifican el rechazo con
  `classify_rejection`: solo un 400/422 que nombra la capacidad autoriza el fallback, y un 401, 402,
  429 o 500 que la mencione ya no degrada en silencio. Tras un rechazo real no se repite la petición
  rechazada en el turno siguiente (aviso «rejected earlier») y un modelo distinto no hereda el
  rechazo. Se retiraron las heurísticas propias `is_programmatic_not_supported` e
  `is_tool_combination_unsupported`. Chat Completions no tenía heurística de rechazo que sustituir.
- **Rechazos (5).** `classify_rejection` separa autenticación, cuota, transporte, petición inválida
  e incompatibilidad explícita; solo la última autoriza fallback, y no conserva el cuerpo.
- **Pruebas (6).** 15 pruebas del registro (con mutaciones comprobadas a mano), 3 de cableado en
  Anthropic, 3 en Responses y 2 en Gemini (rechazo recordado; auth/cuota sin fallback).
