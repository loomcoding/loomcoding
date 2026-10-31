# CodeLoom — Batería difícil del benchmark de esfuerzo de razonamiento

> Estado: **ejecutada en parte** (2026-10-31): `none` y `medium` medidos; `high` pendiente por falta de crédito (HTTP 402).
>
> Origen: la medición `docs/references/2026-10-31-medicion-real-esfuerzo-por-fase.md` quedó en efecto techo (24 de 24
> en todos los niveles). Amplía `2026-10-31-codeloom-reasoning-effort-benchmark.md`; no lo sustituye.

## 1. Problema

La primera batería era demasiado fácil: Haiku 4.5 acertaba todo sin razonamiento, de modo que la medición no podía mostrar
ningún beneficio del esfuerzo. Sin una batería que el modelo falle sin esfuerzo, la pregunta «¿sirve de algo?» sigue
abierta.

## 2. Objetivo

Medir, con los mismos niveles (`none`, `medium`, `high`) y el mismo modelo, si el esfuerzo mejora los aciertos cuando los
defectos exigen razonar (trazar la ejecución), y si empeora o mejora los falsos positivos con código correcto que
**parece** sospechoso.

## 3. No objetivos

Los de la spec original: no se cambian valores por defecto, no se concluye una regla general, un solo modelo.

## 4. Diseño

Ocho fragmentos Python: seis con un defecto que obliga a razonar y dos correctos pero con apariencia sospechosa.

| Caso | Defecto sembrado | Por qué exige razonar |
|---|---|---|
| 1 | Fusión de intervalos que asigna el final en vez de tomar el máximo | Hay que probar un intervalo contenido en otro |
| 2 | Búsqueda binaria con `lo = mid` | Hay que seguir el bucle para ver que no termina |
| 3 | Reintento que, tras agotar los intentos, devuelve `None` en vez de relanzar | Hay que leer el contrato de la docstring contra el flujo |
| 4 | Cubo de fichas sin tope de capacidad | Hay que cotejar el invariante de la docstring |
| 5 | Contador «seguro» que escribe fuera del candado | Hay que ver que lectura y escritura no son atómicas |
| 6 | Año bisiesto con `!= 0` en la condición de 400 | Hay que probar el año 2000 |
| 7 | Correcto: Fibonacci memoizado con caché global documentada | Parece estado global peligroso |
| 8 | Correcto: generador de trozos con validación previa | Parece un posible desfase de uno |

- Niveles: `none`, `medium`, `high`. Repeticiones: 3 por caso y nivel (72 llamadas).
- Puntuación y revisión manual como en la spec original: expresión regular más específica por defecto (aquí una mención
  genérica no basta) y los dos casos correctos aciertan solo con `NO_DEFECTS`.
- Tope de gasto de la etapa: **0,40 USD**, dentro del presupuesto de 1,00 USD (≈ 0,73 disponibles al empezar).
- Criterios de decisión, fijados antes de medir: el esfuerzo «compensa» si mejora los aciertos en al menos 25 puntos
  porcentuales respecto de `none` sin subir los falsos positivos; se anota el coste por acierto. Si ya acierta todo sin
  esfuerzo, la batería sigue siendo demasiado fácil y se declara así, sin repetir indefinidamente.

## 5. Criterios de aceptación

1. Cada caso tiene prueba sin red: la regex casa con una descripción del defecto y no con una respuesta genérica; los
   correctos solo aciertan con `NO_DEFECTS`.
2. La prueba viva es `#[ignore]` y se detiene al superar su tope.
3. Los registros en bruto de la ejecución se guardan en `docs/references/evidencias/2026-10-31-esfuerzo-por-fase/`.
4. El documento de medición se amplía con la tabla, la revisión manual y la conclusión en el vocabulario del §4.
5. Las cuatro comprobaciones pasan.

## 6. Riesgos

- Una regex específica puede dar falsos negativos si el modelo describe bien el defecto con otras palabras: por eso se
  revisan a mano las respuestas fallidas antes de dar el resultado.
- Con 24 llamadas por nivel, un acierto mueve cuatro puntos; la conclusión sigue siendo orientativa.
- Si Haiku vuelve a acertarlo todo, no se sabrá nada sobre tareas aún más difíciles; se documenta y se para.
