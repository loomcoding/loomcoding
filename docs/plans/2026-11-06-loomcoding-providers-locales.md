# Proveedores locales globales — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-06-loomcoding-providers-locales.md`. Parte de la rama `dashboard-limpio`.

## Restricciones globales

- `loomcoding-core` no depende de `ratatui` ni `crossterm`. Ficheros nuevos ≤ 400 líneas.
- Sin red real en las pruebas (`wiremock`). Sin dependencias nuevas salvo que ya estén en el workspace.
- Las claves nunca se escriben en `providers.toml`; solo `api_key_env`.
- Nunca una inferencia al sondear: solo `GET <base_url>/models`.

## Revisión (casos que ninguna tarea cubre por sí sola)

- `providers.toml` corrupto: se avisa y se ignora, no impide arrancar.
- Dos entradas con el mismo `id`: gana la primera y se avisa.
- Servidor que responde con JSON que no es OpenAI (`data` ausente): estado `down` con aviso, sin pánico.
- `LOOMCODING_HOME` aísla el fichero: las pruebas nunca tocan el perfil real.

## Tarea 1 — Registro (`loomcoding-core`)

**Ficheros:** nuevo `crates/loomcoding-core/src/providers_registry.rs` (`LocalProvider`, `ProvidersRegistry::{parse,
load_or_seed(path)}`, constante con las tres entradas por defecto), `lib.rs`.
- [ ] Pruebas: parsea el ejemplo de la spec; `load_or_seed` crea el fichero si falta y no pisa uno existente;
  `default` inexistente cae en la primera entrada; `id` duplicado avisa; toml corrupto devuelve error sin crear nada.

## Tarea 2 — Sondeo (`loomcoding-core`)

**Ficheros:** nuevo `crates/loomcoding-core/src/providers_probe.rs` (`ProbeStatus::{Up{models},Down,Auth}`,
`probe(&LocalProvider, key: Option<&str>) -> ProbeResult`, tiempo máximo 1,5 s, cotas de la spec).
- [ ] Pruebas con `wiremock`: `up` con ids; `401` → `Auth`; conexión rechazada → `Down`; cuerpo sin `data` → `Down`;
  respuesta mayor de 256 KiB se corta; más de 200 modelos se truncan; solo se hace `GET /models`.

## Tarea 3 — Valor por defecto al arrancar (`loomcoding-cli`)

**Ficheros:** `config/mod.rs::load_or_create` admite una configuración inicial opcional; `agent_build.rs` y `main.rs`
cargan el registro (`home::user_home()/.loomcoding/providers.toml`); `build_provider_state` usa la clave ficticia
`local` para URLs de bucle sin `api_key_env`.
- [ ] Pruebas: proyecto sin toml nace con el `default` del registro; toml existente no cambia; `llama` sin clave
  construye un proveedor real (no `UnconfiguredProvider`); URL no local sin clave sigue sin configurar.

## Tarea 4 — Catálogo y estado en la TUI

**Ficheros:** `loomcoding-cli` lanza el sondeo en segundo plano y fusiona los resultados en el `Catalog`
(`CatalogProvider` por servidor local); `loomcoding-tui` guarda el estado `up/down/auth` y lo pinta en el selector de
proveedor; enfriamiento de 5 s al abrir `/model`, `/settings` y el selector.
- [ ] Pruebas: los modelos descubiertos aparecen en el selector de `/model`; un servidor caído conserva su modelo de
  reserva; el enfriamiento evita un segundo sondeo; el estado se pinta junto al nombre.

## Tarea 5 — `/provider` y documentación

**Ficheros:** `commands.rs`, `run/slash.rs`, `app/prefs.rs`, fichero nuevo del comando; `docs/INDEX.md`,
`docs/CODE-MAP.md`, `CHANGELOG.md`; volcado e2e con perfil aislado y servidores simulados.
- [ ] Pruebas: `/provider id` aplica proveedor y modelo por el camino de guardado de `/settings`; id desconocido
  avisa con la lista; sin argumento abre el selector; aparece en el popup y en la ayuda.
