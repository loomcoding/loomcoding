# batten

**Responsabilidad:** widgets y estado para chats de agentes en terminal, en dos capas: `state/`
(puro, sin `ratatui`) y `widgets/` (`ratatui`, con tema).

**No va aquí:** nada de `codeloom-core`: no depende de nadie del workspace y debe seguir
siendo reutilizable fuera de CodeLoom.

**Tipos clave:** `Composer` (`state/composer/`), transcript (`state/transcript.rs`), átomos de
herramienta (`state/tool_atom/`), el tema (`widgets/theme.rs`), `widgets/composer.rs`,
`widgets/transcript.rs`, `widgets/permission.rs`.

**Puntos de entrada:** `state/mod.rs` y `widgets/mod.rs` declaran todo lo público.

**Cómo se testea:** tests unitarios de `state/` sin terminal; los widgets con buffers de
`ratatui`. Las features `fixtures` y `dotmax` activan datos de ejemplo y el rasterizado.

**Trampas:**
- La lógica va en `state/` (testeable sin pantalla); `widgets/` solo pinta.
- `theme.rs` es una tabla de datos grande y cohesiva: no se parte.
