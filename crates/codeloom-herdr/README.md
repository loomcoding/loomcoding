# codeloom-herdr

**Responsabilidad:** integración opcional con `herdr` (un runtime de terminal) como plugin,
sin parchearlo. CodeLoom funciona igual en modo standalone.

**No va aquí:** una TUI propia ni lógica del motor.

**Tipos clave:** `HerdrContext` (cwd del workspace y datos del entorno herdr) y `AgentState`
(`lib.rs`).

**Puntos de entrada:** `lib.rs` (un solo fichero); lo consume `codeloom-cli` al autodetectar
el modo.

**Cómo se testea:** tests unitarios en el propio fichero; validar el pane en una sesión
herdr no elevada sigue pendiente (ver `docs/INDEX.md`, «codeloom-m3»).

**Trampas:** el plugin se llama `codeloom.herdr`; no asumas que herdr está presente.
