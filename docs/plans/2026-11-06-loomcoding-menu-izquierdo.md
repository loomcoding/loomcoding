# Menú izquierdo: logo con efectos, secciones limpias y diff de la sesión — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-06-loomcoding-menu-izquierdo.md`. Parte de la rama `dashboard-limpio`.

## Restricciones globales

- `loomcoding-cloth` no depende de nadie del workspace. Ficheros nuevos de 400 líneas como máximo.
- Sin dependencias nuevas: `tachyonfx` (feature `fx`, activa por defecto) y `MatrixRain` ya están.
- Sin `rand`: generador mínimo con semilla inyectable. Con `NO_COLOR`, ni braille ni efectos.
- Nada de `git` ni de efectos en el hilo de teclas. Toda salida, lista y duración con cota.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Menú estrecho (23 columnas) y alto escaso: el logo cae a `logo::compact` y el diff se acota al alto.
- Sin ficheros tocados: ni título `Ficheros` ni filas. Sin git: una línea que lo dice.
- El diff de un fichero binario, borrado o renombrado no rompe el parser ni el dibujo.

## Tarea 1 — Logo de cabecera

**Ficheros:** `loomcoding-cloth/src/widgets/logo.rs` y `sidebar.rs` (reutilizar `Wordmark`/`FONT_ROWS`/`compact`);
`loomcoding-tui/src/app/composer.rs` (`App::sidebar`).
Logo «LOOM» de dos filas de bloques, franja `╱` en el acento y `loomcoding` tenue; a 21 columnas útiles cabe;
más estrecho, `compact`.
- [ ] Pruebas: el sidebar empieza por el logo; cabe en 21 columnas; cae a `compact` si no cabe.

## Tarea 2 — Secciones limpias

**Fichero:** `loomcoding-tui/src/app/composer.rs` (`App::sidebar`) y lo que haga falta en `cloth`
(`widgets/sidebar.rs`, `state/sidebar.rs`) para títulos en negrita y filas en blanco entre bloques.
Estado + modelo·esfuerzo + ruta corta (`…/carpeta`); `Actividad` en una línea (`◌ N herramientas`,
`◐ N agentes`, `N en cola`, `N jobs`), **lo que vale cero no se pinta** y, si todo vale cero, ni el título;
`Sesión` con tokens abreviados (`format_tokens`). El desglose A2A de agentes va entre paréntesis.
- [ ] Pruebas: bloque vacío no se pinta; sin mayúsculas; filas en blanco entre bloques; A2A nombrado.

## Tarea 3 — Diff de la sesión: modelo, parser y obtención

**Ficheros nuevos:** `loomcoding-tui/src/session_diff.rs` (parser puro y tipos), y el enganche asíncrono
(averiguar cómo llegan los eventos a `App`; tomar `app.changes` como lista de ficheros de la sesión).
Parser de `git diff --no-color -U2`: `FileDiff { path, added, removed, hunks: Vec<Hunk> }`,
`Hunk { header, lines: Vec<DiffLine> }`, `DiffLine::{Context, Added, Removed}`. Obtención en
`spawn_blocking`, plazo 2 s, salida acotada a 64 KiB, como mucho 20 ficheros; resultado `Ok`, `NoGit` o
`TimedOut`, nunca un pánico. Se refresca cuando cambian `app.changes`.
- [ ] Pruebas del parser (hunks, `+`/`-`, binarios, borrados, renombrados, salida truncada) y de las cotas;
  fuera de un repositorio, `NoGit`.

## Tarea 4 — Diff desplegable en el menú

**Ficheros:** `loomcoding-tui/src/render/sidebar.rs` (bloque `Ficheros`), `run/keys.rs` (con el foco en el
menú: `↑/↓` eligen, `Enter` despliega/pliega), estado de selección en `App`.
Fila por fichero `▸ nombre +A −B` (`▾` si está desplegado); diff con `@@`, `-` en error, `+` en éxito, líneas
recortadas con `…`, como mucho 12 líneas y `… N líneas más`; un fichero desplegado a la vez.
- [ ] Pruebas: una fila por fichero; `Enter` despliega y pliega; solo uno abierto; líneas recortadas; tope
  de 12; sin ficheros no hay bloque; `NoGit` muestra su aviso; no altera la entrada del compositor.

## Tarea 5 — Planificador y catálogo de efectos del logo

**Ficheros nuevos:** `loomcoding-cloth/src/widgets/logo_fx.rs`.
`LogoEffect::{BinaryRain, Shimmer, Flashlight, Scramble, Glitch, Dissolve, Pulse}`; `Flashlight` es un foco
elíptico (radio horizontal ≈ 5 columnas, vertical ≈ 2 filas, borde suave) que cruza el logo de izquierda a
derecha o al revés (al azar) en 1,5 s con `Interpolation::SineInOut`, aclarando las celdas que cubre y
atenuando el resto mientras dura; generador mínimo con semilla;
`LogoFx { interval, burst, last, next_at, rng, active }` con `tick(now) -> LogoFxState`, `deadline()` y
`needs_clock()`; nunca el mismo efecto dos veces seguidas; cada ráfaga dura 1,5–3 s; intervalo acotado a
5–600 s. Los efectos con `tachyonfx` se construyen con colores del tema; la lluvia es un `MatrixRain`
propio con `GlyphSet::binary()`.
- [ ] Pruebas deterministas con semilla y reloj inyectados: la linterna aclara las celdas bajo el foco y no
  toca las de fuera de su rectángulo, y el foco termina a un lado distinto del que empezó; sin repetición consecutiva; no hay reloj en
  reposo; `deadline` coincide con el próximo disparo; cotas del intervalo; con efectos apagados no se dispara.

## Tarea 6 — Enganche en la TUI y preferencias

**Ficheros:** `loomcoding-tui/src/render/sidebar.rs` (aplicar el efecto sobre el rectángulo del logo),
`run/mod.rs` (reloj y `deadline`), `prefs.rs` (`logo_fx`, `logo_fx_interval`), `commands.rs`/`run/slash.rs`
(`/logofx`), `render/overlays.rs` (ayuda).
Solo con el layout `Workspace` y el menú visible; sin overlays encima; `NO_COLOR` o `logo_fx = false`, nada.
- [ ] Pruebas: sin ráfaga no corre el reloj; con ráfaga sí y se detiene al acabar; `/logofx` alterna y
  persiste; `NO_COLOR` sin efectos; el efecto no sale del rectángulo del logo.

## Tarea 7 — Documentación y comprobación visual

`docs/INDEX.md`, `docs/CODE-MAP.md` y `docs/ARCHITECTURE.md` si cambian módulos, CHANGELOG; volcado e2e del
menú con efecto activo y con diff desplegado (`docs/visual-e2e.md`, con `LOOMCODING_HOME` aislado y sin
`NO_COLOR`).
