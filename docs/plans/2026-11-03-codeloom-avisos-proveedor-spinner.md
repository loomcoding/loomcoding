# Plan — avisos de proveedor, spinner de estado y comando `/verbose`

Spec: `docs/specs/2026-11-03-codeloom-avisos-proveedor-spinner.md`

## Estado

Hecho: preferencia `[ui]`, contador `provider_in_flight`, ocultación de los
avisos, spinner `DOT_RING` con el nombre del modelo en la barra de estado y
comando `/verbose` (toggle y `on|off` explícito).

Pendiente: nada de este plan. Queda abierto en el spec si `/verbose` debe
persistir en `loomcoding.toml`.

## Restricciones globales

Los cuatro checks deben pasar en cada commit:

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

Sin dependencias nuevas. Sin red en tests.

## Fases

### Fase 1 — `Command::Verbose` (toggle en caliente)

1. `crates/loomcoding-tui/src/commands.rs`
   - Añadir la variante `Verbose` al enum `Command`.
   - Registrarla en `COMMANDS` con la descripción
     «Toggle provider request notices in the transcript».
   - Resolver el nombre `verbose` en el parser (mismo patrón que el resto).
2. `crates/loomcoding-tui/src/app.rs`
   - `pub fn set_provider_notices_verbose(&mut self, on: bool)` — fija el flag.
   - `pub fn toggle_provider_notices(&mut self) -> bool` — alterna y devuelve el
     valor nuevo (lo usa el feedback).
3. `crates/loomcoding-tui/src/run.rs` (despacho de `Command::`, junto a
   `Command::Mouse`)
   - `/verbose` sin argumentos → `toggle_provider_notices()`.
   - `/verbose on` / `/verbose off` → valor explícito.
   - Otro argumento → `slash_feedback` con el uso.
   - Escribir `provider notices: on|off` en `slash_feedback`.
4. Test en `commands.rs`: `verbose` resuelve a `Command::Verbose`.
5. Tests en `app.rs`: el toggle alterna; `off` oculta avisos y `on` los muestra.

**Verificación:** `cargo test -p loomcoding-tui verbose`.

### Fase 2 — Animación con el nombre del modelo

1. `crates/loomcoding-cloth/Cargo.toml`
   - `dotmax` pasa de dev-dependency a dependencia **opcional** con feature
     `dotmax`; el dev-dependency de `cloth` sobre sí mismo la activa para el
     showcase.
2. `crates/loomcoding-cloth/src/widgets/activity.rs` (nuevo)
   - `ACTIVITY_STYLES` con los estilos que se leen a una fila.
   - `activity_glyph(style, frame, width, theme) -> Option<Span>`: renderiza con
     `dotmax` y devuelve `None` si la feature está apagada o el estilo no existe.
   - Tests: cada estilo pinta algo, el glifo cambia entre frames, un estilo
     desconocido devuelve `None`, dos celdas no son más estrechas que una.
3. `crates/loomcoding-tui/src/render.rs`
   - `provider_activity_span(app, width)` con fallback al glifo propio.
   - Usarlo en `draw_flat_footer` y en la barra clásica, seguido de
     `Provider working… <modelo>`.
4. Test en `render.rs`: con `provider_in_flight = 1` y verbose `false`, el
   buffer contiene `Provider working` y el nombre del modelo; con verbose `true`
   no aparece.

**Verificación:** `cargo test -p loomcoding-cloth --features dotmax activity` y
`cargo test -p loomcoding-tui provider`.

### Fase 3 — Documentación

1. `loomcoding.toml` — el bloque `[ui]` ya está con su comentario; revisar que
   mencione `/verbose`.
2. `docs/specs/2026-11-03-codeloom-avisos-proveedor-spinner.md` — mover a
   «hecho» lo que quede cerrado y actualizar «Preguntas abiertas».

## Riesgos

- **Repintado continuo**: incluir `provider_in_flight > 0` en `animating` hace
  que el bucle repinte a 20 fps mientras dura la petición. Es aceptable porque
  ya ocurre con `thinking` y con las herramientas girando, pero conviene no
  añadir más fuentes de animación sin revisar el coste.
- **Colisión de `/verbose` con otros comandos**: hoy no existe ninguno que
  empiece por `verb`; el autocompletado no gana ambigüedad.
- **Texto de modelo largo**: un id tipo `nvidia/nemotron-3-ultra-550b-a55b:free`
  ocupa mucho; el recorte al ancho del área evita desbordar el chasis.

## Fuera de alcance

- Persistir la preferencia desde la TUI (requiere un escritor de config).
- Un `LineKind::Debug` con filtrado por nivel.
- Elegir el estilo de `dotmax` desde la UI: hoy se fija `dot-ring` en código.
  `activity_style_names()` expone el catálogo para cuando se quiera un
  `/spinner <estilo>`.
