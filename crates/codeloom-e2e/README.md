# codeloom-e2e

**Responsabilidad:** apoyo para las pruebas E2E visuales de CodeLoom: terminal simulada,
proveedor simulado, snapshots y limpieza de daemons. Solo se usa como `dev-dependency`.

**No va aquí:** código de producción de ningún tipo.

**Tipos clave:** `Term` y `Key` (`term.rs`: PTY y teclas), `Snapshot` (`snapshot.rs`),
`mock_provider` y `Workspace` (`mock.rs`), `kill_daemons_of` (`daemon.rs`).

**Puntos de entrada:** `lib.rs`; el consumidor es `codeloom-cli/tests/tui_pty.rs`.

**Cómo se testea:** es la herramienta de test; se ejercita con los escenarios de `tui_pty.rs`
(guía en `docs/visual-e2e.md`).

**Trampas:** usa siempre un perfil aislado con `CODELOOM_HOME` temporal y mata los daemons al
terminar (`kill_daemons_of`); si no, el cliente se engancha al daemon real del usuario.
