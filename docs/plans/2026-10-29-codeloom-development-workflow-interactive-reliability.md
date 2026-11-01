# Plan: fiabilidad interactiva del workflow de desarrollo

> Estado (anotado en la higiene del 2026-11-01): sin cierre declarado: la spec gemela solo consta como aprobada; no se ha verificado contra el código en esta higiene.

> Spec: `docs/specs/2026-10-29-codeloom-development-workflow-interactive-reliability.md`.

## Cambios

1. Compartir la allowlist de herramientas del padre entre el dispatch y la
   construcción de `ProviderRequest`; aplicarla mientras haya un workflow activo.
2. Tras guardar el resultado exitoso de `development_workflow start`, salir del
   bucle del turno padre para esperar el aviso del hijo.
3. Añadir una instrucción de planificación específica para Windows y describir
   con precisión que `bash` ejecuta `cmd.exe` en Windows.
4. Explicar en el esquema que los tres perfiles de workflow deben ser distintos.
5. Añadir verificaciones pequeñas con proveedores falsos para el corte del turno
   y la allowlist; actualizar el índice de documentación.
6. Conservar los errores del turno en el transcript aunque el indicador vuelva
   a `ready`, con una prueba de regresión.

## Verificación

- Tests unitarios dirigidos de `codeloom-core`, sin proveedores ni red reales.
- `cargo fmt --all --check` y `git diff --check`.
- Los checks globales de `AGENTS.md` quedan pendientes si el ejecutable local
  sigue bloqueado por el servidor activo; no detenerlo para liberar el archivo.

## Estado

- [x] Reproducir desde la base de datos el orden de llamadas y los errores del
  planner.
- [x] Definir el contrato de salida del padre y el filtro de herramientas.
- [x] Implementar el corte tras `start` y el filtro durante workflow activo.
- [x] Hacer explícitas las restricciones de shell de Windows.
- [x] Ejecutar verificaciones y registrar el resultado.
- [x] Mantener visibles los errores del turno en el transcript.
- [ ] Volver a probar en TUI cuando se pueda lanzar el binario actualizado.

## Verificación 2026-10-29

| Comando | Resultado |
|---|---|
| `cargo test -p codeloom-core --lib` (target aislado en `%TEMP%`) | 545 pasaron, 2 ignorados. |
| `cargo clippy -p codeloom-core --lib -- -D warnings` (target aislado) | 0 errores; Cargo informa 1 warning existente por el `main.rs` compartido entre `loom` y `codeloom`. |
| `cargo build -p codeloom-cli --bin codeloom` (target aislado) | Correcto; mismo warning existente de targets duplicados. |
| `cargo fmt --all --check` | Correcto. |
| `git diff --check` | Correcto. |

El servidor CodeLoom antiguo se detuvo después de compilar el binario actualizado
en el target aislado. El cliente TUI viejo sigue abierto como PID 25752 y mantiene
bloqueado `target/debug/codeloom.exe`; la prueba interactiva debe usar el binario
aislado o esperar a que se cierre ese cliente.

## Límite conocido

La sesión confirma que dos frases entraron con rol `user`, pero no conserva el
cliente que las envió. No borraré ni reescribiré ese historial. Hace falta
instrumentar o reproducir la entrada desde la TUI para atribuir su origen.
