# Plan — Capacidades de tools por provider y modelo

> Estado: **parcial** (2026-10-31). Hecho el registro puro de capacidades por destino, el clasificador de rechazos, el parseo de metadata de OpenRouter y el cableado en el adapter de Anthropic. Falta el enrutado por destino en el resto de adapters y la obtención de metadata real (véase «Avance»).

Spec: `docs/specs/2026-10-28-codeloom-capacidades-tools-provider-modelo.md`.

1. [x] Auditar la metadata actual de Anthropic, OpenRouter y adaptadores, contrastando cada campo con su API documentada.
2. [x] Definir el tipo mínimo de estado/evidencia en `loomcoding-core` sin convertir la capability wire del adapter en una propiedad global del modelo.
3. [~] Implementar parseo/caché de metadata solo para endpoints oficiales que den evidencia utilizable; desconocido se conserva como `unknown`.
4. [~] Enrutar solicitudes según la combinación exacta; separar tools directas, streaming, PTC y herramientas del provider.
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
- **Metadata (3), parcial.** Hay parseo de ambos formatos de OpenRouter con fixtures; el catálogo
  agregado solo prueba que alguna ruta acepta `tools`, y un endpoint fijado con parámetros
  discrepantes devuelve «sin dato». No hay aún obtención de red ni caché en disco.
- **Enrutado (4), parcial.** Solo el adapter de Anthropic consulta el registro. Tras un rechazo real
  de PTC ya no se repite la petición rechazada en cada turno. Pendientes: Chat Completions,
  Responses y Gemini, cuyas heurísticas de «no soportado» (`is_programmatic_not_supported`,
  `is_tool_combination_unsupported`) siguen siendo propias y trabajan sobre el mensaje, no sobre
  estado y cuerpo.
- **Rechazos (5).** `classify_rejection` separa autenticación, cuota, transporte, petición inválida
  e incompatibilidad explícita; solo la última autoriza fallback, y no conserva el cuerpo.
- **Pruebas (6).** 15 pruebas del registro (con mutaciones comprobadas a mano) y 3 de cableado.
