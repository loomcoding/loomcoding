# Medición real: esfuerzo de razonamiento por fase (C2)

> Fecha: 2026-10-31. Autorizada por el usuario con un tope de 1,00 USD; gasto ≈ 0,266 USD.
> Spec: `docs/specs/2026-10-31-codeloom-reasoning-effort-benchmark.md`. Plan: `docs/plans/2026-10-31-codeloom-reasoning-effort-benchmark.md`.
> Código: `crates/codeloom-core/tests/effort_by_phase_live.rs` (etapas 0 y 1) y `stage2_low_effort_on_execution` en
> `crates/codeloom-core/tests/reproduce_first_live.rs` (etapa 2). Ambas ignoradas por defecto.

## Resumen

Con Haiku 4.5 y tareas de esta dificultad, **el esfuerzo de razonamiento no mejora el resultado**: en revisión acierta
todo sin él, y en ejecución resuelve lo mismo con `low`. En revisión, `medium` y `high` multiplican el coste por diez.
La medición es de un solo modelo y de tareas fáciles (efecto techo), de modo que **no refuta** el patrón: solo muestra
que aquí no hay nada que ganar.

## Etapa 0: ¿llega `effort` al modelo?

Una misma revisión (inyección SQL) con y sin `high`: **36 → 477 tokens de salida** (0,00028 → 0,00252 $). La tabla de
`anthropic_budget` surte efecto en el servicio real, al menos con `high`. Gasto: 0,0028 $.

## Etapa 1: revisión sin herramientas

Seis fragmentos (cinco con un defecto sembrado, uno limpio) × 4 repeticiones = 24 llamadas por nivel.

| Esfuerzo | Aciertos | Falsos positivos | Tokens de salida | Coste |
|---|---|---|---|---|
| ninguno | 24 de 24 | 0 | 932 | 0,0072 $ |
| `medium` | 24 de 24 | 0 | 13.878 | 0,0726 $ |
| `high` | 24 de 24 | 0 | 15.543 | 0,0809 $ |

Se leyeron a mano las respuestas sin esfuerzo: describen de verdad cada defecto (no es la expresión regular la que
acierta por azar) y el caso limpio responde `NO_DEFECTS`. Es **efecto techo**: no hay margen para que el esfuerzo mejore
nada. Con razonamiento, el coste sube unas diez veces por el mismo resultado.

Desviación de la spec: se añadió `medium` y se pasó de 2 a 4 repeticiones, porque el coste por llamada resultó mucho menor
de lo previsto. Gasto: 0,1607 $.

## Etapa 2: ejecución con herramientas

La misma tarea y el mismo agente que en `2026-10-31-medicion-real-reproducir-antes-de-arreglar.md` (dos fallos en
`inventory.py`), con `effort = low`, cuatro ejecuciones.

| Variante | Resueltas | Llamadas a herramientas | Coste por ejecución |
|---|---|---|---|
| sin esfuerzo (medición anterior) | 3 de 3 | 9 | 0,0266 $ (0,0798 $ las tres) |
| `low` | 4 de 4 | 9 (36 en total) | 0,0255 $ (0,1022 $ las cuatro) |

Resuelven por igual, con las mismas llamadas y el mismo coste. Gasto: 0,1022 $.

Corrección: la spec y un resumen previo daban ≈ 0,079 USD «por ejecución» para la línea base; son 0,0798 USD **por las
tres** ejecuciones. Queda corregido en la spec.

## Conclusión (vocabulario del §4.5 de la spec)

- **Revisión:** el esfuerzo alto no mejora los aciertos (0 puntos, frente al umbral de 25), de modo que no compensa para
  este modelo y esta dificultad. Es **inconcluso** para revisiones difíciles, que esta batería no cubre.
- **Ejecución:** resuelve igual y no sube el coste, de modo que no se activa el criterio de «no recomendar»; tampoco hay
  ninguna ventaja que justifique fijar `low`.
- **Decisión:** no se fija ningún `effort` por defecto en perfiles ni en fases del flujo.

## Limitaciones

- Un solo modelo, pocas tareas y todas dentro de su capacidad: la ausencia de efecto no se extrapola a tareas difíciles.
- La línea base de la etapa 2 es la medición anterior, no una ejecución simultánea; con `thinking` la API omite la
  temperatura (0,0 sin él), así que ambos brazos no son idénticos en eso.
- La etapa 2 no recogió tokens de salida: no se sabe cuánto razonó realmente con presupuesto de 2.048.

## Siguiente paso posible

Si se quiere saber si el esfuerzo sirve de algo, hace falta una batería **más difícil** (revisión de defectos que Haiku
sin esfuerzo falle de verdad, o un modelo más pequeño), con presupuesto aparte. Quedan ≈ 0,73 USD de los 1,00 autorizados.
