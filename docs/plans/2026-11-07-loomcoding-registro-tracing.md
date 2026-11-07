# Registro de `tracing` a fichero — plan

> **Para quien ejecute:** prueba que falla primero, código mínimo, los cuatro chequeos, un commit por tarea. Cargo desde
> PowerShell. Rama `compresion`. Binario real solo con `LOOMCODING_HOME` temporal.

**Spec:** `docs/specs/2026-11-07-loomcoding-registro-tracing.md`.

## Tarea 1 — Escritor acotado e instalación del suscriptor

**Ficheros:** módulo nuevo `crates/loomcoding-cli/src/registro_tracing.rs` (≤ 400 líneas) con un escritor que rota a
`.1` al pasar de 5 MiB (`MAX_LOG_BYTES`) y la función `instala(rol: &str, dir_datos: &Path) -> Option<Guard>` que solo
instala si `RUST_LOG` está definida y no vacía; se llama desde el arranque del cliente y del daemon (localizar los dos
puntos de entrada en `crates/loomcoding-cli/src`), con un fichero por proceso (`loomcoding-<rol>.log`).
- [ ] Pruebas: sin `RUST_LOG` no se crea fichero ni directorio; con `RUST_LOG=output_filter=info` un evento de ese destino
  llega al fichero y uno de otro destino no; al pasar de la cota rota y el fichero actual queda por debajo de ella; un
  directorio no escribible no provoca error ni pánico; nada va a stdout/stderr (el escritor es solo el fichero).
  Hazlas sin tocar el entorno global del proceso de pruebas más de lo imprescindible (suscriptor local con
  `tracing::subscriber::with_default`).
- Documentación: `docs/output-filter.md` (cómo medir de verdad: `RUST_LOG=output_filter=info`, ruta del fichero),
  `docs/odd.md` (idem con `RUST_LOG=odd=info`), `docs/CODE-MAP.md`, `CHANGELOG.md`, `docs/INDEX.md`.

## Tarea 2 — Prueba e2e en PTY de la compresión y del registro

**Ficheros:** `crates/loomcoding-cli/tests/tui_pty_compresion.rs` (+ `docs/mockups/compresion-e2e.md` con el volcado real).
Sigue el patrón de `tui_pty_odd.rs` y `docs/visual-e2e.md`: proveedor simulado de `loomcoding-e2e` (`mock_provider_*`; añade
un guion nuevo que llame a `bash` con una salida ruidosa conocida, por ejemplo `cargo`-like repetida) y `LOOMCODING_HOME`
temporal con `[output_filter] enabled = true` y `RUST_LOG=output_filter=info`.
- [ ] Prueba: en el transcript aparece la cabecera `[filtrado: N líneas omitidas de M — regla …; el original sigue en …]`
  de lo que recibió el modelo; el fichero de original existe y contiene la salida íntegra; el fichero de registro contiene
  una línea del destino `output_filter` con la regla y los bytes; sin `enabled = true` no hay cabecera y no hay línea de
  registro. Sin red real.
