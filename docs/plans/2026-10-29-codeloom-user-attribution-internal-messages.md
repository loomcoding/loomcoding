# Plan: atribución de mensajes internos en el transcript

> Estado (anotado en la higiene del 2026-11-01): implementado; cubierto por `tui_pty` (atribución `You`).

Spec: [`docs/specs/2026-10-29-codeloom-user-attribution-internal-messages.md`](../specs/2026-10-29-codeloom-user-attribution-internal-messages.md)

## Cambios

1. Añadir procedencia persistente a `Message`: prompts externos `Human`, mensajes
   `Role::User` internos `Internal` por defecto.
2. Atribuir `You` solo a mensajes `Human`, al restaurar sesiones y en eventos
   en vivo.
3. Conservar roles/textos del modelo y probar procedencia core/TUI.

## Estado

- [x] Implementar procedencia persistente y atribución estricta.
- [x] Ejecutar pruebas de core/TUI y `cargo fmt --check`.
- [ ] Repetir visualmente en la TUI tras reiniciar el daemon local.

Verificación: 552 pruebas core pasaron (2 ignoradas), 463 pruebas TUI pasaron;
`cargo check -p codeloom-cli --bin codeloom`, `cargo build -p codeloom-cli --bin codeloom`,
`cargo fmt --check` y `git diff --check` pasaron.

Pendiente para aceptación visual: sigue ejecutándose el daemon anterior (PID
7424, `E:\workspace\personal\loom\target\debug\codeloom.exe --server`). No lo
detuve porque podría tener trabajo activo; la TUI debe reiniciarse contra el
binario nuevo en `%LOCALAPPDATA%\Temp\codeloom-user-attribution-target\debug`.
