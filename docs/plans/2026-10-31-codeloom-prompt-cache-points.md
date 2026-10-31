# Plan — Puntos de caché de prompts

> Estado: implementado (2026-10-31).
> Spec: `docs/specs/2026-10-31-codeloom-prompt-cache-points.md`.

1. Pruebas de `apply_cache_points` (criterios 1 a 5) con una función vacía; verlas fallar.
2. Implementar la función, `with_prompt_cache` y llamarla desde `build_body`.
3. Encendido en el CLI con el interruptor de entorno.
4. Medición real (criterio 6) ampliando la prueba `#[ignore]`, con tope de gasto: quedan unos 0,46 USD de los 0,50
   autorizados. Se aborta al superarlo.
5. Cuatro comprobaciones, doc de resultados y commit.
