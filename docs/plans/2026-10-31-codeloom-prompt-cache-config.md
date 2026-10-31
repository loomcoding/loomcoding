# Plan — Caché de prompts configurable en `codeloom.toml`

> Estado: implementado (2026-10-31).
> Spec: `docs/specs/2026-10-31-codeloom-prompt-cache-config.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real; sin gasto.
- `ProviderConfig` no gana campos.

## Tareas

1. **Cargador (pruebas primero).** En `codeloom-core/src/config/mod.rs`: `ProviderCacheConfig` con `from_toml` y `load`;
   pruebas de ausencia, `true`, `false` y valor no booleano.
2. **Precedencia (pruebas primero).** En `codeloom-cli/src/main.rs`: `prompt_cache_enabled(entorno, configurado)` con las
   pruebas de los casos del §5.2, y actualizar la prueba existente.
3. **Cableado.** `build_provider` y `build_provider_state` reciben `Option<bool>`; el servidor lo carga una vez de
   `codeloom.toml` y lo pasa también al resolutor de proveedores de perfil; `apply_provider_settings` lo carga de su
   `config_path`; el listado de modelos pasa `None`.
4. **Cierre.** Cuatro comprobaciones, spec del cambio anterior y inventario actualizados, commit.
