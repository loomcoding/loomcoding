# Plan: adjuntar imágenes desde el chat

> Estado (anotado en la higiene del 2026-11-01): implementado; cabo suelto: revisión visual en la TUI real.

Spec: [`docs/specs/2026-10-29-codeloom-image-attachments.md`](../specs/2026-10-29-codeloom-image-attachments.md)

## Pasos

1. Añadir la parte de imagen persistible al modelo de mensajes y serializarla en
   los adaptadores OpenAI-compatible y Anthropic.
2. Permitir que `AgentLoop` reciba el mensaje humano multiparte sin convertirlo
   en texto plano.
3. Leer imágenes del portapapeles con Ctrl+V, mostrarlas como insignias y
   mantener el transporte acotado por la cola y el IPC local.
4. Añadir pruebas para formatos, límites, persistencia, cola y payload de ambos
   proveedores; ejecutar las comprobaciones del workspace.

## Estado

- [x] Modelo multiparte, adaptadores OpenAI/Anthropic y ejecución de turno humano.
- [x] Ctrl+V desde el portapapeles, validación, insignia, cola e IPC local.
- [x] Suites de `batten`, `codeloom-core`, `codeloom-tui` y CLI; `fmt --check` y clippy.
- [x] `cargo build` y `cargo test` del workspace tras cerrar el ejecutable en Windows. Hecho el 2026-10-30 con un `CARGO_TARGET_DIR` aparte, que evita el bloqueo del `.exe`.
- [ ] Prueba visual en la TUI.

## Límites

El umbral de tamaño protege la memoria y el IPC, pero no garantiza que todos los
modelos configurados tengan capacidad de visión.
