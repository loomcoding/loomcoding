# Plan: Temas → Efectos → Matrix rain

- **Spec**: `docs/specs/2026-11-02-codeloom-temas-efectos-matrix-rain.md`
- **Fecha**: 2026-11-02

## Restricciones globales

Los cuatro checks deben pasar en cada commit:

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

Reglas que aplican a todo el plan:

- `codeloom-core` **no** se toca: esto es TUI y widgets.
- Nada de red en tests.
- Todo lo que se pinte tiene cap: el juego de glifos y la densidad tienen
  límites explícitos.
- Los tests de render usan `TestBackend`, no terminal real.

## Fase 1 — Hacer configurable el widget (crate `batten`) ✅ HECHA

Sin tocar la TUI todavía. Al final de la fase, `MatrixRain` acepta una config y
hay tests que lo prueban.

**Resultado**: `MatrixConfig`, `GlyphSet` (5 presets + `custom`), `GlyphError`,
`MatrixRain::with_config`/`set_config`/`config`, render con ancho variable y
12 tests. `Default` conserva el comportamiento previo. Los cuatro checks pasan
(510 tests).

1. **`MatrixConfig`** en `matrix_rain.rs`:
   ```rust
   pub struct MatrixConfig {
       pub glyphs: GlyphSet,
       pub speed: f32,
       pub density: u8,
       pub length: u16,
       pub head: Color,
       pub tail: Color,
   }
   ```
   `Default` reproduce exactamente el comportamiento actual (katakana, 0.25,
   3, 5, los colores de hoy). Esto es importante: el *showcase* no debe cambiar.

2. **`GlyphSet`** con `chars: Vec<char>` y `width: u8` (1 o 2). Presets
   `katakana`, `emoji`, `binario`, `ascii`, `mixto`. Constructor
   `GlyphSet::custom(&str) -> Result<Self, GlyphError>` que valida ancho
   uniforme.

3. **`MatrixRain::with_config(cfg)`** y sustituir los usos de `GLYPHS` y de los
   `Color::Rgb` hardcodeados por la config. Mantener `Default` como
   `with_config(MatrixConfig::default())`.

4. **Render con ancho 2**: cuando `glyphs.width == 2`, avanzar de dos en dos
   columnas y no pintar la celda contigua. Test con `TestBackend` que verifica
   que no se solapan glifos.

5. **Tests**:
   - `default_config_matches_previous_behaviour`
   - `custom_glyph_set_rejects_mixed_width`
   - `wide_glyphs_do_not_overlap`
   - `density_zero_emits_nothing`
   - `speed_and_length_are_clamped`

## Fase 2 — Persistencia (crate `batten` + `codeloom-tui`) ✅ HECHA

**Resultado**: `[effects.matrix]` en `theme.toml` (`ThemeFile` → `EffectsFile` →
`MatrixFile`), `Theme.matrix`, `StyleToken::key`/`from_key`/`ALL_TOKENS`,
`prefs::load_matrix`/`save_matrix` con claves `matrix_*`, y el arranque aplica
tema + override en `set_prefs_path`. 8 tests de TOML y 6 de prefs.

**Decisión de color**: los colores son **tokens del tema**, no hex. Un solo
token (`color`) define la gota; la cola se deriva oscureciéndolo, así que
cambiar de preset de tema mantiene la armonía. El default es
`StyleToken::Success` (el verde del tema), que es el verde Matrix.

1. **`theme.rs`**: añadir `effects: Option<EffectsFile>` a `ThemeFile`
   (línea 272), con `[effects.matrix]` deserializable a `MatrixConfig`.
   `Theme` gana `pub matrix: MatrixConfig`.
2. **`prefs.rs`**: `load_matrix`/`save_matrix` con el patrón de
   `load_theme`/`save_theme`. El override de prefs se aplica encima del tema.
3. **Tests**: ida y vuelta TOML → `MatrixConfig`; prefs vacías devuelven el
   valor del tema; prefs corruptas no rompen el arranque (caen al tema).

## Fase 3 — Ventana con árbol (crate `codeloom-tui`)

1. **`Command::Temas`** en `commands.rs` y su entrada en `COMMANDS`.
   Despacho en `run.rs` junto a `Command::Settings`.
2. **Estado**: `TemasWindow { level: TemasLevel, selection: usize, matrix: MatrixConfig }`
   en `app.rs`, con `TemasLevel::{Root, Effects, Matrix, Theme}`.
3. **`open_temas`** en `app.rs`, siguiendo `open_settings` (línea 1845).
4. **`draw_temas`** en `render.rs`, siguiendo `draw_provider_settings`
   (línea 1200). Árbol a la izquierda, formulario a la derecha.
5. **Teclado**: `↑`/`↓` selección, `Enter` entra, `Esc` sube un nivel y cierra
   en la raíz, `←`/`→` ajustan rangos, `Space` alterna bools.
6. **Tests**: navegación del árbol (entrar, volver, cerrar en raíz); aplicar un
   preset de tema desde la ventana cambia el tema; `Esc` en raíz cierra.

## Fase 4 — Conectar la lluvia a la TUI real

1. Instanciar `MatrixRain` en `App`, inicializada desde tema + prefs.
2. Pintarla como fondo antes de los widgets de primer plano.
3. Avanzarla en el tick de 50 ms existente.
4. Test: con `enabled = false` no se pinta nada y `animating()` es `false`.

## Fase 5 — Documentación

1. Actualizar `docs/specs/2026-10-23-batten-showcase-theme-matrix-design.md`
   para que apunte a la nueva spec y no duplique la config.
2. Documentar `[effects.matrix]` en el README (en español, como el resto).

## Riesgos

- **Ancho de emoji**: depende de la fuente del terminal. Si el terminal no
  renderiza el emoji a doble ancho, la rejilla se desalinea igual. Mitigación:
  el preset `emoji` es opcional y el usuario puede volver a `katakana`.
- **Coste de repintado**: la lluvia pinta celda a celda. Con densidad alta en
  terminales grandes puede notarse. Mitigación: la densidad tiene cap (10) y
  `animating()` corta el trabajo cuando no hay gotas.
- **Conflicto con el showcase**: si el *showcase* cambia su copia, divergen.
  Mitigación: `Default` conserva el comportamiento actual, así que el showcase
  sigue igual sin tocarlo.
