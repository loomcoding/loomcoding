# Compresión de salida — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla (rojo visible), código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`), un commit por
> tarea. Cargo desde PowerShell. Rama `compresion`.

**Spec:** `docs/specs/2026-11-06-loomcoding-compresion-de-salida.md` (sección «Decisiones de la persona usuaria»). Las
reglas **R1–R10** de la spec son obligatorias: cada tarea las nombra en sus pruebas.

## Restricciones globales

- Módulo nuevo del núcleo `crates/loomcoding-core/src/output_filter/`; ficheros ≤ 400 líneas; sin `ratatui`; sin
  dependencias nuevas (`regex` ya está).
- **Opt-in:** `[output_filter] enabled = false` por defecto y `LOOMCODING_NO_FILTER=1` lo apaga siempre (R10).
- Solo salidas de **herramientas** (`bash`, `job_output`, `job_start`); nunca la respuesta de un subagente ni de una tarea
  A2A. Nunca `read`/`grep`/`glob` (salvo la dedup de la tarea 4). Ningún estado A2A.
- El original se conserva con `ToolSpill` (R2); la cabecera lleva su ruta (R1). El transcript y el modelo ven lo mismo (R9).
- Sin red real en las pruebas. Rendimiento: entrada mayor de 2 MiB o filtro que pasa de 50 ms → la salida pasa sin
  filtrar (R8).
- Binario real solo con `LOOMCODING_HOME` temporal.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Salida vacía, de una línea, sin salto final, con UTF-8 inválido o con `\r\n`: pasa igual o filtrada, sin pánico.
- Salida desmesurada (decálogo, regla 10): se acota antes de filtrar.
- Comando con tuberías o `&&` (`cargo test && echo ok`): decide el primer ejecutable; ante la duda, sin regla.
- `LANG` distinto de inglés o `--message-format json`: formato no reconocido → pasa sin tocar (R6).
- Dos llamadas en paralelo filtran sin compartir estado mutable.

## Tarea 1 — Capa base: reglas generales, cabecera, original y `raw`

**Ficheros:** `output_filter/mod.rs` (`FilterConfig`, `apply(tool: &str, input: &Value, out: &ToolOutput, cfg:
&FilterConfig) -> Filtered`), `output_filter/general.rs` (ANSI, retornos de carro de barras de progreso, líneas idénticas
consecutivas «×N», líneas en blanco), `output_filter/header.rs` (cabecera R1), `config/` (sección `[output_filter]`, cargador
propio como `FlowsConfig`), `agent/turn.rs` (una llamada antes de `contenido_de_herramienta`; la lógica fuera de `turn.rs`),
`tools/bash.rs` y `tools/job_output.rs` (parámetro `raw: bool` en el esquema; `raw: true` salta el filtro), `tools/spill.rs`
(guardar también cuando se omite algo aunque quepa).
- `Filtered { content: String, omitted_lines: usize, rule: Option<&'static str>, original: Option<PathBuf> }`.
- [ ] Pruebas: R1 (cabecera `[filtrado: N líneas omitidas de M — regla X; el original sigue en <ruta>]` siempre que se
  omite), R2 (el original íntegro está en el volcado y `raw: true` lo devuelve sin filtro), R4 (`is_error` o código ≠ 0 →
  solo reglas generales), R5 (el colapso siempre dice cuántas), R6 (ejecutable sin regla pasa), R8 (entrada de 5 MiB pasa
  sin filtrar y rápido), R10 (`enabled = false` y `LOOMCODING_NO_FILTER=1`), UTF-8 inválido, sin salto final, salida vacía;
  propiedad: tras las reglas generales no desaparece ninguna línea con `error`, `FAILED`, `panic` o `warning:`.

## Tarea 2 — `cargo test`, `cargo build` y `cargo clippy`

**Ficheros:** `output_filter/cargo.rs` (+ `output_filter/fixtures/` con capturas reales). Lista blanca de señal (R3): se
conservan errores, avisos, `FAILED`, `panicked`, resumen final y la línea de contexto de cada fallo; se omiten `Compiling`,
`Downloading`, `Finished` repetidos y las líneas `test x ... ok`. Línea no reconocida → se conserva.
- [ ] Pruebas con **salidas reales** (R7): capturas de `cargo test` (todo ok; con un fallo escondido entre 400 líneas `ok`;
  con `panicked`), de `cargo build` con avisos y de `cargo clippy -D warnings` con errores, sacadas de este repositorio con
  un `CARGO_TARGET_DIR` temporal; la propiedad central (ninguna línea con `error`, `FAILED`, `panic`, `warning:`
  desaparece) sobre todas; salida desmesurada; formato no reconocido (`--message-format json`) pasa sin tocar.

## Tarea 3 — `git` y gestores de paquetes

**Ficheros:** `output_filter/git.rs` (`status`, `log`; un `diff` es salida estructurada: solo reglas generales) y
`output_filter/packages.rs` (`npm`/`pnpm install`: barras de progreso y avisos de deprecación repetidos).
- [ ] Pruebas con capturas reales (R7), propiedad de señal, `git diff` intacto salvo ANSI, formato desconocido pasa.

## Tarea 4 — Dedup de lecturas en el historial

**Riesgo anotado:** reescribir un resultado antiguo cambia el prefijo que el proveedor tiene en caché; solo se sustituye la
copia antigua cuando el fichero **no ha cambiado** y la nueva lectura está en el historial. **Se hace después de medir** las
tareas 1–3; si el ahorro no lo justifica, se documenta y se cierra sin implementarla.
**Ficheros:** `agent/read_dedup.rs` (módulo nuevo).
- [ ] Pruebas: misma lectura dos veces sin cambios → la antigua queda como referencia breve; con el fichero cambiado → no se
  toca; referencia a una lectura inexistente → no se sustituye; el número de mensajes y su orden no cambian.

## Tarea 5 — Medición, auto-desactivación y documentación

**Ficheros:** `event/mod.rs` (`LiveEvent::OutputFiltered { rule, original_bytes, sent_bytes }`; no relevante para
subagentes; nota de cable en CHANGELOG), `output_filter/stats.rs` (ahorro por regla; re-petición = `raw: true` o repetir el
comando; regla con ≥ 10 usos y > 20 % de re-peticiones se desactiva sola y publica un `RuntimeNotice` una sola vez),
`docs/output-filter.md` (qué hace, cómo apagarlo, cómo medir), `CODE-MAP`, `CHANGELOG`, `INDEX`, y la sección «Estado tras
la implementación» de la spec con el resultado de la medición y la decisión sobre activarlo por defecto.
- [ ] Pruebas: el evento se publica con bytes correctos; 10 usos y 3 re-peticiones → se desactiva y avisa una sola vez;
  10 usos y 1 → no; el contador no lleva datos personales; volcado e2e con perfil aislado (`tui_pty_*`) que muestre la
  cabecera de filtrado en pantalla.
