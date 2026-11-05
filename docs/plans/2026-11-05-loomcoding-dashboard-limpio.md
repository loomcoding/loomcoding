# Dashboard limpio y muñeco de estado — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-05-loomcoding-dashboard-limpio.md`. **Boceto de referencia:**
`crates/loomcoding-cloth/examples/showcase/demos/dashboard_clean.rs` (de él salen los mapas de puntos del muñeco
y la forma de las tres disposiciones).

## Restricciones globales

- `loomcoding-cloth` no depende de nadie del workspace; el muñeco recibe una postura, no un agente.
- Ficheros nuevos de 400 líneas como máximo. No se cambia el contenido del transcript ni la lógica del agente.
- Los textos de pantalla que ya tienen prueba se mantienen salvo que la tarea diga lo contrario.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Terminal estrecho (<80 columnas): el panel de B no se pinta y el transcript ocupa todo el ancho.
- Rail con cero subagentes, cero jobs y cero tokens: no aparece ningún bloque, ni siquiera un título suelto.
- Tema `Classic`/otros presets: siguen mostrando el aspecto histórico.

## Tarea 1 — Muñeco como widget de `cloth`

**Ficheros:** `crates/loomcoding-cloth/src/widgets/mascot.rs` (nuevo), `widgets/mod.rs`; el demo del showcase pasa a
usarlo y pierde su copia.
**Produce:** `pub enum Pose { Think, Run, Hunt, Wait, Done, Fail }` y
`pub fn mascot_frame(pose: Pose, tick: u64) -> [String; 2]` (dos líneas de seis celdas braille; `Hunt` usa los
fotogramas de `Run`, la lupa la pinta quien lo use). Mapas de puntos 12×8 copiados del demo.
- [ ] Pruebas: `braille_maps_dots_to_the_right_bits` (puntos (0,0)→U+2801, (1,0)→U+2808, (0,3)→U+2840),
  `every_pose_draws_two_lines_of_six_cells`, `running_changes_between_frames`, `fail_is_static`.
- [ ] Mover la función y los mapas; el demo importa `loomcoding_cloth::widgets::mascot`.

## Tarea 2 — De estado de agente a postura

**Ficheros:** `crates/loomcoding-tui/src/mascot.rs` (nuevo), `lib.rs`.
**Produce:** `pub fn pose_of(app: &App) -> (Pose, String)`: la postura y el texto de «qué hace».
Reglas, en este orden: hay pregunta o permiso pendiente (`input-required`) → `Wait`; último turno fallido → `Fail`;
herramienta de búsqueda en curso (`grep`, `glob`, `view`, `ls`) → `Hunt`; otra herramienta en curso o subagente
`working` → `Run`; el modelo emite razonamiento o espera respuesta → `Think`; turno recién acabado con éxito → `Done`
(solo los primeros 3 s); sin actividad → `None` (no hay franja).
- [ ] Pruebas, una por regla, más `question_wins_over_a_running_tool` y `idle_has_no_mascot`.

## Tarea 3 — Disposición como preferencia propia (`/layout`)

**Decisión (2026-11-05):** la disposición es una preferencia **independiente del color**, como `spinner` o `mouse`;
no se crean presets nuevos. Ficheros: `loomcoding-cloth/src/widgets/theme/look.rs`; `loomcoding-tui/src/prefs.rs`,
`app/prefs.rs`, `commands.rs`, `render/mod.rs`, `render/overlays.rs` (ayuda).
**Produce:** `LayoutStyle::{Minimal, Cards}` (los existentes no cambian), con `name() -> &'static str`
(`"rail"`, `"minimal"`, `"cards"`, `"workspace"`), `from_name(&str) -> Option<Self>` y `next(self) -> Self`
(`ContextRail → Minimal → Cards → ContextRail`; `Workspace → ContextRail`); `prefs::{load_layout, save_layout}`
(clave `layout` de `ui.toml`, mismo patrón que `spinner`); `App::set_layout`, que persiste; `Command::Layout`
(`/layout`: rota y avisa del layout elegido). Un layout guardado manda sobre el del preset, también tras `/theme`.
`Minimal` y `Cards` dibujan el transcript a todo el ancho, sin rail ni panel lateral (de momento sin tarjeta ni muñeco).
- [ ] Pruebas: `layout_names_round_trip`, `next_cycles_rail_minimal_cards`, `layout_survives_a_save_and_load`,
  `a_saved_layout_overrides_the_preset_look_after_theme_change`, `minimal_and_cards_draw_no_rail`, `layout_command_cycles`.

## Tarea 4 — Cabecera de una línea

**Fichero:** `crates/loomcoding-tui/src/render/chrome.rs` (`draw_flat_header`).
`loom · modelo · esfuerzo` a la izquierda; `ctx usado/ventana ██░░` a la derecha y, en `Minimal` y `Cards`, la
etiqueta del flujo activo. Sin ruta, sin segunda línea. Cuando no hay ventana conocida se muestra `ctx 2.2k`.
- [ ] Pruebas con `TestBackend`: una sola línea de cabecera; la etiqueta del flujo aparece solo en A y C.

## Tarea 5 — Rail adelgazado (B)

**Ficheros:** `crates/loomcoding-tui/src/rail/mod.rs` y submódulos.
Sin `Borders::LEFT`; títulos en negrita (`Agentes`, `Tokens`); bloques `Agentes`, `Tokens` y una línea de MCP/LSP;
fuera `MODEL`, ruta y el resto de secciones que ya cubre la cabecera; **un bloque vacío no se pinta**.
Cada fila de agente: glifo de estado (`◐ ○ ✓ ✗`), nombre, tiempo; si agrupa estados, el A2A original entre paréntesis.
- [ ] Pruebas: `empty_blocks_are_not_drawn`, `no_left_border`, `agent_rows_show_glyph_name_and_elapsed`,
  `a2a_state_is_named_in_parentheses`.

## Tarea 6 — Franja del muñeco y preferencia `mascot`

**Ficheros:** `loomcoding-tui/src/render/` (franja sobre el compositor), `prefs.rs` (clave `mascot`, por defecto `true`).
Franja de tres filas: muñeco a la izquierda (acento; error en `Fail`; éxito en `Done`) y «qué hace» a la derecha;
`Hunt` añade `⌕`. Con `mascot = false` o `NO_COLOR`, una sola línea de texto `◌ postura · qué hace`; sin actividad,
nada. El reloj de 50 ms existente basta; no se añade ninguno.
- [ ] Pruebas: franja ausente en reposo; apagada, sin caracteres braille; `NO_COLOR` sin braille; `/theme` o ajustes
  persisten `mascot` en `ui.toml`.

## Tarea 7 — Tarjeta de agentes (C)

**Fichero nuevo:** `loomcoding-tui/src/render/agents_card.rs`.
Tarjeta sobre el compositor (solo en `Cards`): cabecera `❀ Agentes` con `▾ plegar`, una fila por tarea con columnas de
modelo · esfuerzo, tokens y tiempo alineadas al ancho máximo de lo mostrado, tope de filas
(`min(8, 25 % del alto)`) y `… N more`. `ctrl+b` pliega y despliega.
- [ ] Pruebas: columnas alineadas, tope de filas con `… N more`, plegada ocupa una fila, ausente fuera de `Cards`.

## Tarea 8 — Pie de cuatro atajos y ayuda `?`

**Ficheros:** localizar el pie actual (`enter send  ctrl+j newline…`) y la tecla `?`.
Pie: `enter envía · ctrl+o pliega · ctrl+b panel · ? atajos` (`tarjeta` en `Cards`). `?` abre una lista con todos los
atajos actuales, sin perder ninguno.
- [ ] Pruebas: el pie tiene exactamente cuatro; `?` lista los que había; ningún atajo existente desaparece.

## Tarea 9 — Documentación y comprobación visual

`docs/INDEX.md` (fila nueva), `docs/CODE-MAP.md` y `docs/ARCHITECTURE.md` si cambian módulos, CHANGELOG; volcado e2e
de las tres disposiciones (`docs/visual-e2e.md`, con `LOOMCODING_HOME` aislado) y captura en `docs/mockups/`.
