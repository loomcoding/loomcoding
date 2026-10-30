# Plan — Capacidades de tools por provider y modelo

> Estado: **pendiente**. Sin implementar: en el código solo existe una caché puntual de capacidad en el adapter de Anthropic (`capability_cache`), no el tipo de estado/evidencia por provider y modelo que propone.

Spec: `docs/specs/2026-10-28-codeloom-capacidades-tools-provider-modelo.md`.

1. Auditar la metadata actual de Anthropic, OpenRouter y adaptadores, contrastando cada campo con su API documentada.
2. Definir el tipo mínimo de estado/evidencia en `codeloom-core` sin convertir la capability wire del adapter en una propiedad global del modelo.
3. Implementar parseo/caché de metadata solo para endpoints oficiales que den evidencia utilizable; desconocido se conserva como `unknown`.
4. Enrutar solicitudes según la combinación exacta; separar tools directas, streaming, PTC y herramientas del provider.
5. Clasificar rechazos sin cuerpos crudos y permitir fallback únicamente ante incompatibilidad explícita.
6. Añadir pruebas de fixtures para destinos distintos, metadata ausente/expirada, rechazo de capacidad y errores de auth/cuota; sin red.
7. Actualizar evidencia y ejecutar los cuatro checks del repositorio.
