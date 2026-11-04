# Plan: PTC de Anthropic Haiku vía OpenRouter

> Estado: **hecho con resultado negativo** (2026-11-04). La compatibilidad PTC vía OpenRouter no se confirmó (rechazada en las dos pruebas autorizadas del 2026-10-28); queda como intento opt-in con upstream Anthropic fijado y no se afirma ahorro. Se reabre solo si OpenRouter anuncia soporte.

> Spec: `docs/specs/hechos/2026-10-28-codeloom-openrouter-anthropic-ptc.md`.

## Decisiones

- Reutilizar `AnthropicProvider` y Anthropic Messages, con detección estricta de base URL
  OpenRouter y modelo `anthropic/claude-haiku-5.5`.
- Fijar el upstream en el cuerpo OpenRouter por petición y desactivar fallbacks.
- Considerar el modo como intento experimental: fallback solo en rechazo explícito HTTP 400 que
  identifique `code_execution`/PTC como no soportado; mantener errores ambiguos como errores.
- Ejecutar benchmark solo con OpenRouter, con límite de cuatro pares y parada por coste reportado
  de USD 1. Sin llamadas a Anthropic directo.

## Pasos

1. Añadir tests locales del detector de modelo/base URL y del objeto `provider`; asegurar que la
   API Anthropic directa no cambia.
2. Adaptar la consulta de capacidad para OpenRouter Haiku 5.5 sin tratar el catálogo `tools` como
   evidencia; permitir el intento solo con upstream Anthropic fijado.
3. Añadir fallback únicamente para incompatibilidad PTC explícita HTTP 400. Comprobar que 401,
   5xx, cancelación y timeout no reintentan.
4. Ejecutar tests focalizados y los cuatro checks globales.
5. Comprobar sin mostrar el valor si existe `OPENROUTER_API_KEY` o credencial guardada.
6. Ejecutar una pareja directa/PTC con límite de tokens y tope autorizado de USD 1. La directa
   respondió sin invocar tool y PTC devolvió HTTP 400; detener aquí, sin reintentos facturables.
7. Registrar los datos observados y dejar claro que no hubo comparación ni ahorro confirmado.

## Verificación prevista

```text
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

La primera llamada directa estimó USD 0,000106 con el precio publicado. OpenRouter no devolvió el
detalle de facturación en los chunks usados.

## Repetición autorizada (2026-10-28)

Se volvió a ejecutar el test live directo/PTC, con un solo par y el límite de USD 1:

```text
mode=direct tokens_in=402 tokens_out=135 turns=1 latency_ms=1260 tool_ptc=false cost_usd=0.000108 exact=false provider_rejected=false
mode=ptc tokens_in=0 tokens_out=0 turns=1 latency_ms=52 tool_ptc=false cost_usd=0.000000 exact=false provider_rejected=true
```

La ruta directa volvió a responder sin invocar `view`; PTC fue rechazado por el provider. El test
no expone el cuerpo de error y no confirma el código HTTP de esta repetición, ni permite afirmar
que OpenRouter no cobrara el rechazo. PTC sigue sin confirmar y no se afirma ahorro.
