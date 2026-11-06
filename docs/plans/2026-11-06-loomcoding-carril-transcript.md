# Carril del transcript configurable — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-06-loomcoding-carril-transcript.md`. Trabajo en una rama propia.

## Restricciones globales

- El carril ya vive en `crates/loomcoding-tui/src/render/transcript.rs` (`TREE_GUTTER`, `tree_gutter`,
  `draw_tree_gutter`); las pruebas, en `render/tests/transcript_y_arbol.rs`. El fichero de preferencias es
  `ui.toml` (patrón de `style` y `layout` en `app/prefs.rs`) y el panel, `UiRow` (`app/ui_command.rs`).
- Ficheros nuevos ≤ 400 líneas; sin dependencias nuevas; el valor por defecto sigue siendo `tree`.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Un transcript de menos de 40 columnas no reserva carril en ningún valor.
- Cambiar el valor con el transcript desplazado no mueve el texto de forma visible más allá del cambio de ancho.
- Un `ui.toml` con un valor de `rail` desconocido se ignora y queda `tree`, con aviso.

## Tarea 1 — El valor y su pintado

**Ficheros:** nuevo `crates/loomcoding-tui/src/render/rail.rs` (`RailStyle::{Tree,Dots,Bar,None}`,
`rail_width(style, width) -> u16`, `rail_cells(style, first, last, focused) -> [char; 2]` pura y probable),
`render/transcript.rs` (`draw_tree_gutter` y `tree_gutter` consultan el valor), `app/mod.rs` (campo
`rail_style`).
- [ ] Pruebas: por cada valor, la primera, la intermedia, la última y la fila única de un bloque; el enfocado en
  `tree` y `dots` usa trazo grueso; en `bar` solo el enfocado lleva `▌`; en `none` el ancho es 2 y no hay trazo;
  por debajo de 40 columnas el ancho es 0.

## Tarea 2 — Preferencia, fila de `/ui` y `ui.toml`

**Ficheros:** `app/prefs.rs` (carga y guardado de `rail`), `app/ui_command.rs` (`UiRow::Rail`, `/ui carril <valor>`
con `tree|dots|bar|none`), `render/overlays.rs` (ayuda si hace falta).
- [ ] Pruebas: la fila cicla con `←`/`→`; `/ui carril none` cambia y guarda; un valor desconocido avisa con la
  lista; `ui.toml` sobrevive al cambio de tema; el valor ausente da `tree`.

## Tarea 3 — Documentación y volcado

`docs/INDEX.md`, `docs/CODE-MAP.md`, CHANGELOG; volcado e2e con perfil aislado de los cuatro valores
(`docs/visual-e2e.md`), con capturas en `docs/mockups/`.
