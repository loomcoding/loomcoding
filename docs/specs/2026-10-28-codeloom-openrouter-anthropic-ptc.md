# Spec: PTC de Anthropic Haiku vía OpenRouter

> Estado: **parcial**. La compatibilidad PTC real vía OpenRouter está sin confirmar (HTTP 400 en la prueba autorizada del 2026-10-28); queda como intento opt-in con upstream Anthropic fijado.

## Objetivo

Probar y, si la ruta lo admite, habilitar la orquestación programática de tools (PTC) de
Anthropic desde Loomcoding usando OpenRouter, empezando por `anthropic/claude-haiku-5.5`.
Comparar la ruta directa de tool calling con PTC bajo el mismo modelo y una ruta upstream fija.

## Distinción de responsabilidades

- Loomcoding sigue siendo dueño de las tools cliente, validación, permisos, resultados y eventos.
- Anthropic ejecuta el código programático en su runtime cuando el proveedor upstream acepta
  `code_execution` con `allowed_callers`.
- OpenRouter es el gateway que recibe Anthropic Messages y enruta al proveedor indicado.
- `openrouter:bash` y `openrouter:shell` son tools alojadas por OpenRouter; no equivalen al PTC
  de Anthropic y quedan fuera de esta integración.

## Contrato y restricciones

1. Mantener el transporte de Anthropic Messages y apuntar `base_url` a `https://openrouter.ai/api`.
2. Permitir la prueba PTC solo con el ID exacto `anthropic/claude-haiku-5.5`, el opt-in ya
   existente `LOOMCODING_PROGRAMMATIC_TOOL_CALLING=1` y `provider.order=["anthropic"]` con
   `allow_fallbacks=false`. Así se evita comparar backends distintos o un fallback opaco.
3. No interpretar la presencia de `tools` en el catálogo de OpenRouter como prueba de PTC.
   La lista de compatibilidad de Haiku 5.5 en Anthropic acredita el modelo, no que OpenRouter
   acepte `code_execution`. El adapter debe probar el contrato al enviar la petición; un rechazo
   explícito y seguro por tool/version no soportada vuelve a tools directas y emite un aviso.
4. No reintentar ante timeout, 5xx, desconexión u otro error ambiguo: podría haberse ejecutado
   código y repetir la petición duplicaría trabajo. Solo un rechazo explícito HTTP 400 de
   incompatibilidad permite fallback directo.
5. No añadir capabilities genéricas a `ProviderConfig`, no cambiar OpenAI-compatible, MCP,
   A2A ni el valor por defecto (opt-in apagado).
6. Nunca registrar la API key, el cuerpo crudo de error ni código generado.

## Evaluación real mínima

- Modelo fijo: `anthropic/claude-haiku-5.5`; endpoint OpenRouter Anthropic Messages; upstream
  Anthropic fijado, sin fallback.
- Misma instrucción, tool de lectura local, contenido y `max_steps` en dos recorridos: tool call
  directa y PTC habilitado.
- Limitar a cuatro pares de tareas pequeñas, `max_tokens` por respuesta y detenerse si el coste
  acumulado indicado por OpenRouter alcanza USD 1. La herramienta de lectura devuelve un fixture
  local pequeño, no accede a secretos ni a red.
- Guardar por recorrido: modo realmente usado (incluido fallback), tokens de entrada/salida,
  latencia, cantidad de turnos del provider, tool calls, respuesta y coste reportado por
  OpenRouter. No declarar ahorro si PTC no fue confirmado.
- Comparar exactitud de salida contra respuestas esperadas; tokens/latencia no sustituyen
  calidad. Publicar solo datos de uso sin cabeceras ni credenciales.

## Criterios de aceptación

- Pruebas locales verifican modelo elegible, pin estricto del proveedor, cuerpos Anthropic
  normales sin cambios y fallback únicamente en rechazo explícito de PTC.
- Haiku PTC solo se anuncia como soportado tras una respuesta PTC observada a través de
  OpenRouter con upstream Anthropic; se registra el uso de al menos una tool cliente de Loomcoding.
- Si OpenRouter/upstream rechaza el formato, el turno pasa a tool calling directo sin repetir una
  ejecución ambigua.
- El benchmark se detiene antes de superar USD 1 y deja mediciones reproducibles. Si no hay key
  OpenRouter disponible, no se afirma prueba real.
- `cargo build`, `cargo test`, `cargo fmt --check` y `cargo clippy -- -D warnings` pasan.

## Fuentes consultadas

- [Anthropic: PTC, compatibilidad y Haiku 5.5](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling)
- [OpenRouter: Anthropic Messages skin](https://openrouter.ai/docs/guides/coding-agents/claude-code-integration)
- [OpenRouter: routing y provider order](https://openrouter.ai/docs/guides/features/provider-routing)
- [OpenRouter: endpoints actuales de Haiku 5.5](https://openrouter.ai/api/v1/models/anthropic/claude-haiku-5.5/endpoints)
- [OpenRouter: precio/modelo Haiku 5.5](https://openrouter.ai/anthropic/claude-haiku-5.5/)

## Resultado de la prueba autorizada (2026-10-28)

- Una petición directa: 402 tokens de entrada, 132 de salida, 1 turno, 1.428 ms y coste
  calculado con la tarifa publicada de USD 0,000106. El modelo no invocó la tool y no devolvió
  exactamente el valor esperado; por tanto, no es una medición válida de la ruta con tool.
- La petición PTC recibió HTTP 400 de OpenRouter. No se expuso ni guardó el cuerpo de error.
  No se observó una llamada PTC ni se completó el par comparativo; no se afirma ahorro.
- Gasto reportado por OpenRouter no disponible en el resultado del adapter. El importe anterior
  es solo una estimación con tokens y tarifa pública; el rechazo puede tener facturación distinta.
- La integración queda como intento opt-in con upstream Anthropic fijado. La compatibilidad PTC
  real vía OpenRouter permanece sin confirmar; no se habilita como capacidad garantizada.

## Fuera de alcance

- Soporte PTC genérico de todos los modelos Anthropic a través de OpenRouter.
- Ejecutar `openrouter:bash` o `openrouter:shell` desde el runtime Loomcoding.
- Inferir compatibilidad de PTC desde los parámetros estándar `tools` o desde un endpoint
  distinto al proveedor realmente fijado.
