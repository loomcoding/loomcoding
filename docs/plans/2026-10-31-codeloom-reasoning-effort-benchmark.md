# Plan — Benchmark del esfuerzo de razonamiento por fase

> Estado: pendiente de aprobación. No se ejecuta ninguna llamada real hasta que la persona usuaria lo autorice.
> Spec: `docs/specs/2026-10-31-codeloom-reasoning-effort-benchmark.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`.
- Sin red real en la suite normal: la prueba viva va con `#[ignore]`.
- Tope de gasto: 1,00 USD en total (0,05 + 0,40 + 0,35 por etapa, 0,20 de reserva).
- La clave se carga solo en el proceso de la prueba y sin imprimirla: `set -a; . ./.env; set +a; cargo test ... -- --ignored`.
  La prueba no arranca el binario real, de modo que no interviene `CODELOOM_HOME`.

## Tareas

1. **Casos y puntuación (sin red).** Pruebas primero: cada caso sembrado casa su regex con una respuesta que describe el
   defecto y no la casa con una respuesta genérica; el caso limpio solo acierta con `NO_DEFECTS`. Luego los datos en
   `crates/codeloom-core/tests/effort_by_phase_live.rs`.
2. **Acumulador de gasto (sin red).** Prueba primero: con costes simulados, la etapa se detiene al superar su tope.
3. **Etapa 0 (≈ 2 llamadas, ≤ 0,05 USD).** Con y sin `high`; si los tokens de salida no suben, parar y documentar.
4. **Etapa 1 (24 llamadas, ≤ 0,40 USD).** Resumen por variante; guardar respuestas para revisión manual.
5. **Decidir `medium`.** Solo si la etapa 1 muestra señal; otro tope dentro de la reserva.
6. **Etapa 2 (4 ejecuciones, ≤ 0,35 USD).** `low` sobre los dos fallos reales; comparar con la línea base publicada.
7. **Documento de medición** en `docs/references/` con datos, confusiones y conclusión según el §4.5 de la spec; marcar C2
   en la lista de candidatas; inventario; commit.

## Puntos de parada

Tras la tarea 3 (si la tabla no surte efecto) y tras la tarea 4 (para revisar las respuestas a mano antes de gastar en la
etapa 2). En ambos se informa a la persona usuaria con el gasto acumulado.

## Fuera de esta tarea

Cambiar el `effort` de los perfiles de demostración o de las fases del flujo de desarrollo: se decide después, con los datos.
