# Plan: composer de chat multilínea

Spec: [`docs/specs/2026-10-29-codeloom-multiline-chat-composer.md`](../specs/2026-10-29-codeloom-multiline-chat-composer.md)

## Cambios

1. Limitar el alto visible del composer a tres filas visuales en Classic y
   Crush, con ajuste automático al ancho y seguimiento del cursor.
2. Mantener el pegado multilínea como inserción atómica del prompt, nunca como
   envío; conservar `Enter` para enviar y `Ctrl+J` para nueva línea.
3. Usar el soporte Windows VT de Crossterm para que el pegado llegue como un
   solo evento `Paste`, sin clasificar Enter por temporización.
4. Probar el crecimiento, el tope con scroll y el evento de pegado multilínea.

## Estado

- [x] Implementar el límite visual de tres líneas.
- [x] Conservar Enter para enviar y Ctrl+J para nueva línea.
- [x] Habilitar bracketed paste real en Windows mediante Crossterm VT input.
- [x] Añadir o ajustar pruebas de crecimiento, scroll y pegado.
- [x] Ejecutar `cargo fmt --check` y las pruebas de los crates afectados.
- [ ] Verificar el comportamiento en la TUI.

Verificación tras los ajustes: 45 pruebas del composer (batten), la prueba de
inserción de `Event::Paste`, `cargo check -p codeloom-cli` y la compilación del
binario pasaron; también
`cargo fmt --check` y `git diff --check`.

Windows VT input usa la revisión fija `e2bc49e` de Crossterm PR #1030, aún
abierto upstream. La comprobación visual del pegado en el terminal del usuario
sigue pendiente.
