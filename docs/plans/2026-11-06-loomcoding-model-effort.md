# Comandos `/model` y `/effort` — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-06-loomcoding-model-effort.md`. Parte de la rama `dashboard-limpio`.

## Restricciones globales

- Reutilizar el flujo de ajustes: `App::provider_settings`, `settings_save_requested`,
  `reconcile_effort`, `effort_options`, `SettingsPicker::Model`. No duplicar el guardado.
- Ficheros nuevos de 400 líneas como máximo; los grandes (`app/settings.rs`, `run/keys.rs`, `app/prefs.rs`) no crecen.
- Sin red real en las pruebas. Sin dependencias nuevas.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Sin `provider_settings` (modo sin ajustes): aviso «Provider settings are unavailable», como `/settings`.
- Un `<id>` con espacios o vacío no cambia nada y avisa del uso.
- Tras `/model` con un modelo que no admite el esfuerzo vigente, el esfuerzo vuelve a `default` y se dice.

## Tarea 1 — `/model`

**Ficheros:** `commands.rs` (`Command::Model`, entrada en `COMMANDS`), `run/slash.rs`, `app/prefs.rs`
(`slash_query` admite argumentos para `model`), fichero nuevo `app/model_effort.rs` (`App::set_model_now(&mut self,
id: &str) -> Result<(), String>` y la apertura del selector rápido), `render/overlays.rs` (ayuda).
- [ ] Pruebas: `/model x` fija `provider_settings.model`, deja `settings_save_requested` y el aviso; id con
  espacios o vacío no cambia nada; sin ajustes avisa; sin catálogo y sin argumento avisa de usar `/model <id>`;
  con catálogo, sin argumento, abre el selector de modelos y `Enter` aplica; `Esc` cancela sin cambios; el
  esfuerzo no soportado por el modelo nuevo vuelve a `default` con aviso; aparece en el popup y en la ayuda.

## Tarea 2 — `/effort`

**Ficheros:** `commands.rs` (`Command::Effort`), `run/slash.rs`, `app/prefs.rs` (`slash_query`),
`app/model_effort.rs` (`set_effort_now(&mut self, level: Option<&str>) -> Result<(), String>` y selector de
niveles), `render/` del selector, `render/overlays.rs` (ayuda).
- [ ] Pruebas: `/effort high` fija el nivel y lo aplica; `/effort default` lo quita; nivel desconocido avisa con
  la lista real; modelo sin niveles avisa de declarar `efforts`; sin argumento abre el selector con `default`
  y los niveles marcando el vigente, `Enter` aplica y `Esc` cancela; los niveles declarados en
  `[provider] efforts` valen cuando el catálogo no publica niveles; aparece en el popup y en la ayuda.

## Tarea 3 — Documentación y comprobación

`docs/INDEX.md`, `docs/CODE-MAP.md` (sección de comandos), CHANGELOG; volcado e2e con perfil aislado
(`docs/visual-e2e.md`): `/effort` muestra el selector y `/model <id>` cambia el pie.
