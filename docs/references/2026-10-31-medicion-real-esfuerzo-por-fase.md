# Medición real: esfuerzo de razonamiento por fase (C2)

> Fecha: 2026-10-31. Autorizada por el usuario con un tope de 1,00 USD; gasto ≈ 0,266 USD.
> Spec: `docs/specs/hechos/2026-10-31-codeloom-reasoning-effort-benchmark.md`. Plan: `docs/plans/hechos/2026-10-31-codeloom-reasoning-effort-benchmark.md`.
> Código: `crates/codeloom-core/tests/effort_by_phase_live.rs` (etapas 0 y 1) y `stage2_low_effort_on_execution` en
> `crates/codeloom-core/tests/reproduce_first_live.rs` (etapa 2). Ambas ignoradas por defecto.

## Resumen

**Actualización (batería difícil, ver más abajo):** cuando el defecto exige razonar, `medium` sí lo encuentra y el modo sin
esfuerzo no (3 de 3 frente a 0 de 3), pero `medium` inventa defectos en código correcto (5 de 6 frente a 0 de 6) y cuesta
unas 24 veces más. `high` no pudo medirse por falta de crédito. Lo que sigue es el resumen de la primera batería.

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

## Batería difícil (etapa 3)

Spec y plan: `docs/specs/2026-10-31-codeloom-reasoning-effort-hard-battery.md`, `docs/plans/2026-10-31-codeloom-reasoning-effort-hard-battery.md`.
Ocho fragmentos: seis con un defecto que exige razonar (intervalos, búsqueda binaria, reintento, cubo de fichas, contador con
candado, año bisiesto) y dos correctos de apariencia sospechosa (Fibonacci con caché global documentada, generador de trozos).
Tres repeticiones por caso y nivel.

**Datos válidos** (registro en bruto: `evidencias/2026-10-31-esfuerzo-por-fase/etapa3-intento1-invalido.log` para `medium`;
`etapa3-bateria-dificil.log` repite `ninguno` con el mismo resultado):

| Esfuerzo | Defectos hallados (18 llamadas) | Código correcto aceptado (6 llamadas) | Falsos positivos | Tokens de salida | Coste |
|---|---|---|---|---|---|
| ninguno | 15 de 18 | 6 de 6 | 0 | 750 | 0,0070 $ |
| `medium` | 18 de 18 | 1 de 6 | 5 | 32.995 | 0,1689 $ |
| `high` | sin datos válidos | — | — | — | — |

- **Lo que sí ayuda:** el año bisiesto (`!= 0` donde debía ser `== 0`) lo falla Haiku **sin esfuerzo las tres veces**, contestando
  `NO_DEFECTS`, y con `medium` lo encuentra las tres veces explicando el caso de 2000. Es el primer efecto real del esfuerzo.
- **Lo que perjudica:** con `medium`, 5 de 6 revisiones de código correcto devuelven hallazgos inventados (falta de validación
  de tipos, desbordamiento de pila con `n` grande, «condición de carrera» de la caché). Sin esfuerzo, ninguna.
- **Coste:** unas 24 veces más (0,169 $ frente a 0,007 $) por 3 hallazgos adicionales sobre 18.
- **Criterio del §4 de la spec** (mejora de al menos 25 puntos sin subir los falsos positivos): **no se cumple**, porque los
  falsos positivos suben de 0 a 5. Los defectos hallados mejoran 17 puntos (83 % a 100 %).

**Datos inválidos, descartados:**

- `high`, primera pasada: casi todas las llamadas devolvieron respuesta vacía (0 tokens, 0 $) porque la prueba ignoraba el
  error de la ejecución. Una llamada suelta posterior con `high` funcionó, de modo que no se sabe la causa con certeza.
- Repetición con registro de errores: **HTTP 402 de OpenRouter** («requires more credits… can only afford 7162 tokens»). 138
  llamadas rechazadas, `medium` sin datos salvo dos llamadas y `high` sin ninguna. Mi tope de 1,00 USD no era el límite: el
  límite era el crédito de la cuenta o la clave de OpenRouter, que hay que revisar fuera de este repositorio. Además, los
  reintentos de 30 s alargaron la prueba a 52 minutos; ahora un 402 aborta de inmediato.
- Corrección: antes sospeché de un límite de ritmo; no hay prueba de eso, y la evidencia apunta al crédito.

## Conclusión ampliada

- **Revisión de defectos que exigen razonar:** el esfuerzo medio encuentra lo que el modo sin esfuerzo se pierde, pero a un
  coste muy alto y generando falsos positivos. No compensa como valor por defecto; podría tener sentido solo como mando
  explícito para una revisión concreta, si se tolera revisar los hallazgos a mano.
- **Ejecución:** sigue sin efecto (primera batería).
- **Sin medir:** `high` en la batería difícil. Con `n` = 24 por nivel y un caso que explica casi toda la diferencia, es
  orientativo, no concluyente.

## Siguiente paso posible

Repetir `high` de la batería difícil (≈ 0,2 USD) cuando haya crédito en OpenRouter, con `cargo test -p codeloom-core --test effort_by_phase_live stage3_hard_battery_high_only -- --ignored --nocapture` (solo `high`, tope 0,40 USD; aborta ante un 402). El gasto de esta tarea por mi cuenta ronda
0,5 USD, pero el saldo real de la cuenta es el que manda.
