# CodeLoom — Caché de prompts configurable en `codeloom.toml`

> Estado: **implementado** (2026-10-31).
>
> Origen: mejora anotada en `2026-10-31-codeloom-prompt-cache-points.md` §4 («queda como mejora ponerlo en la
> configuración»).

## 1. Problema

El interruptor de los puntos de caché del adaptador de Anthropic solo se maneja con la variable de entorno
`CODELOOM_PROMPT_CACHE`. Una persona que quiera apagarlo para un proyecto concreto tiene que exportar la variable en
cada arranque; no queda constancia en el proyecto y no hay forma de verlo junto al resto de la configuración del
proveedor.

## 2. Objetivo

Poder fijarlo en `codeloom.toml`, sin cambiar el comportamiento de quien no toque nada.

## 3. No objetivos

- No cambia el valor por defecto (encendido para `kind = "anthropic"`) ni el adaptador.
- No afecta a `openai_compat`, `openai_responses` ni `gemini`.
- No añade ningún campo a `ProviderConfig`: se construye en más de cuarenta sitios.
- No hay comando ni opción en la interfaz.

## 4. Diseño

Clave opcional `prompt_cache` (booleano) en la sección `[provider]`:

```toml
[provider]
kind = "anthropic"
prompt_cache = false
```

Se lee con un cargador aparte, `ProviderCacheConfig`, que solo mira esa clave y descarta el resto, igual que
`AgentsConfig` con `[agents]`. Así el resto del código que construye `ProviderConfig` no cambia. Un valor que no sea
booleano es un error de configuración. `CodeLoomConfig::save` ya conserva las claves ajenas del documento.

**Precedencia**, de mayor a menor:

1. `CODELOOM_PROMPT_CACHE` si está definida (`0`, `off` o `false` apaga; cualquier otro valor enciende), por ser la
   decisión más cercana al arranque y mantener el comportamiento actual.
2. `prompt_cache` de `codeloom.toml`.
3. Encendido.

La función pura `prompt_cache_enabled(entorno, configurado)` resuelve esa precedencia. `build_provider` y
`build_provider_state` reciben el valor configurado (`Option<bool>`); los sitios que no tienen configuración a mano
pasan `None`.

## 5. Criterios de aceptación

1. Sin la clave, `ProviderCacheConfig` da `None`; con `true` o `false`, el valor; con un valor no booleano, error.
2. `prompt_cache_enabled`: sin entorno ni clave, encendido; con clave `false`, apagado; con entorno `off` y clave
   `true`, apagado; con entorno `1` y clave `false`, encendido; con entorno vacío, vale la clave.
3. El servidor y el flujo de ajustes del proveedor leen la clave de `codeloom.toml` y se la pasan al proveedor.
4. Las cuatro comprobaciones del proyecto pasan.

## 6. Riesgos

- Un entorno con la variable definida en el sistema anula sin aviso el fichero del proyecto; es la precedencia
  elegida, y se documenta aquí.
- Los proveedores de perfil (`profile_provider_resolver`) usan el valor del `codeloom.toml` del proyecto, no uno propio.
