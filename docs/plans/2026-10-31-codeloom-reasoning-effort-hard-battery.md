# Plan — Batería difícil del benchmark de esfuerzo

> Estado: ejecutado en parte (2026-10-31); `high` sin datos válidos por HTTP 402. Resultado en `docs/references/2026-10-31-medicion-real-esfuerzo-por-fase.md`.
> Spec: `docs/specs/2026-10-31-codeloom-reasoning-effort-hard-battery.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real en la suite normal; la prueba viva va con `#[ignore]`.
- Tope de la etapa: 0,40 USD (autorización vigente de 1,00 USD; gastado ≈ 0,27 antes de esta etapa).
- La clave se carga solo en el proceso de la prueba y se filtra de los registros.

## Tareas

1. **Evidencias de las etapas 0 a 2** en `docs/references/evidencias/2026-10-31-esfuerzo-por-fase/` (hecho).
2. **Casos y regex (sin red).** Pruebas primero en `effort_by_phase_live.rs`: cada defecto casa su descripción y no una
   respuesta genérica; los dos correctos solo aciertan con `NO_DEFECTS`. Luego los datos.
3. **Etapa 3 (72 llamadas, ≤ 0,40 USD).** En segundo plano; registro bruto a `etapa3-bateria-dificil.log`.
4. **Revisión manual** de las respuestas fallidas y de los falsos positivos.
5. **Documento de medición** ampliado, candidatas, inventario y commit.
