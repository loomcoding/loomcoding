# Panel `/ui` — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-06-loomcoding-panel-ui.md`. Parte de la rama `dashboard-limpio`.

## Restricciones globales

- `loomcoding-cloth` no depende de nadie del workspace. Ficheros nuevos de 400 líneas como máximo.
- Compatibilidad: las claves de `ui.toml` actuales se leen igual; los comandos viejos siguen funcionando.
- Los ficheros que ya pasan de 400 líneas no crecen: lo nuevo va a módulos propios.

## Revisión (casos que ninguna tarea cubre por sí sola)

- `ui.toml` con un `layout` o `theme` inválido no rompe el arranque (ya ocurre; que siga).
- Con un overlay abierto, `/ui` no se puede teclear: no hay doble panel.
- Un terminal estrecho recorta el panel sin pánico.

## Tarea 1 — Estilo: el par `classic`/`flat`

**Ficheros:** `loomcoding-cloth/src/widgets/theme/look.rs` (o un `style.rs` nuevo si crece).
`StyleBundle::{Classic, Flat}` con `name()`/`from_name()`/`next()`, `Look::bundle()` y `Look::with_bundle()`
que fijan `composer`, `tool` y `chrome` a la vez; `Classic` = `Look::classic()` sin layout, `Flat` =
`Look::crush()` sin layout. El layout no se toca.
- [ ] Pruebas: ida y vuelta de `name`/`from_name`; `with_bundle` conserva el layout; `Look::crush().bundle()
  == Flat`; un `Look` mixto a mano devuelve el bundle más cercano o `None` sin pánico.

## Tarea 2 — Estado y preferencias del estilo

**Ficheros:** `loomcoding-tui/src/prefs.rs` (`style`), `app/prefs.rs` (`set_style`, y `layout_override`
deja de existir: el layout se guarda y se carga directamente en el `Look`).
`ThemeFile` (`loomcoding-cloth/src/widgets/theme/loading.rs`) admite `style` y `layout` opcionales como valores
por defecto del tema; prioridad: tema < `ui.toml`.
- [ ] Pruebas: un tema de usuario con `style = "flat"` y `layout = "minimal"` los aplica; `ui.toml` los
  sobrescribe; valores inválidos en el tema dan error de carga legible sin pánico; `set_style` aplica y persiste; cargar `ui.toml` antiguo sin `style` deja el del preset; un
  valor inválido se ignora; el layout guardado sigue mandando sobre el del preset.

## Tarea 3 — Filas y valores del panel

**Ficheros nuevos:** `loomcoding-tui/src/app/ui_panel.rs` (`UiRow`, `UiRow::ALL`, `label()`, `value(&App)`,
`cycle(&mut App, forward)`), sin tocar `TemasLevel` todavía.
- [ ] Pruebas: una fila por fila de la spec en ese orden; `value` refleja el estado; `cycle` hacia delante
  y hacia atrás da la vuelta y persiste; `Muñeco`, `Menú lateral` y `Ratón` alternan.

## Tarea 4 — Del árbol `/temas` al panel `/ui`

**Ficheros:** `app/settings.rs`, `app/settings_types.rs` (`TemasLevel::Root` pasa a listar las `UiRow`),
`render/` del overlay de `/temas`, `run/keys.rs` (`←`/`→` en la raíz).
Los subniveles `Effects`, `Matrix`, `Theme` y `Spinner` se conservan. Cabecera «Interfaz».
- [ ] Pruebas: la raíz muestra las ocho filas con su valor; `←`/`→` cambian el valor sin entrar; `Enter`
  entra en Efectos/Tema/Spinner; `Esc` sube y a la raíz cierra; los tests actuales de `/temas` siguen verdes.

## Tarea 5 — `/ui` y alias ocultos

**Ficheros:** `commands.rs` (`Command::Ui`; campo de visibilidad en `COMMANDS`), `run/slash.rs`,
`app/prefs.rs` (`slash_matches` oculta los alias), `render/overlays.rs` (ayuda).
`/ui <fila> [valor]` se resuelve con `UiRow` y `cycle`; los comandos viejos delegan en lo mismo.
- [ ] Pruebas: el popup de `/` no lista los ocho alias y sí `/ui`; teclear `/crush` exacto sigue
  funcionando; `/ui layout rail` fija el layout; `/ui style` rota; valor desconocido lista los posibles;
  `/ui` sin argumentos abre el panel; la ayuda nombra los alias.

## Tarea 6 — Documentación y comprobación visual

`docs/INDEX.md`, `docs/CODE-MAP.md`, CHANGELOG; volcado e2e del panel y del popup de `/` reducido
(`docs/visual-e2e.md`, `LOOMCODING_HOME` aislado, sin `NO_COLOR`).
