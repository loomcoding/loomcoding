# batten

**Responsabilidad:** widgets y estado para chats de agentes en terminal, en dos capas: `state/`
(puro, sin `ratatui`) y `widgets/` (`ratatui`, con tema).

**No va aquí:** nada de `codeloom-core`: no depende de nadie del workspace y debe seguir
siendo reutilizable fuera de CodeLoom.

**Tipos clave:** `Composer` (`state/composer/`), transcript (`state/transcript.rs`), átomos de
herramienta (`state/tool_atom/`), el tema (`widgets/theme/`), `widgets/composer/`,
`widgets/transcript/`, `widgets/permission.rs`.

**Puntos de entrada:** `state/mod.rs` y `widgets/mod.rs` declaran todo lo público.

**Cómo se testea:** tests unitarios de `state/` sin terminal; los widgets con buffers de
`ratatui`. Las features `fixtures` y `dotmax` activan datos de ejemplo y el rasterizado.

**Lluvia de Matrix (`widgets/matrix_rain.rs`):** se configura en `[effects.matrix]` de `theme.toml`
(`glyphs` = `katakana`, `emoji`, `binario`, `ascii`, `mixto` o un juego propio; `speed`, `density`,
`length` y `color`, que es un token del tema y no un hex). Lo que la persona cambia en `/temas` se
guarda en las preferencias (`matrix_glyphs`, `matrix_speed`, `matrix_density`, `matrix_length`,
`matrix_color`) y manda sobre el tema. Todo valor se acota antes de llegar al render. Por defecto:
`katakana`, 0,25, 3, 5 y `success`.

**Trampas:**
- La lógica va en `state/` (testeable sin pantalla); `widgets/` solo pinta.
- `theme/` reparte el tema: `styles.rs` (glifos y estilos), `look.rs`, `presets.rs` (tabla de colores) y `loading.rs` (TOML).
