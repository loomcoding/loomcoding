# CodeLoom — Benchmark del esfuerzo de razonamiento por fase

> Estado: **borrador para aprobar** (no se ha lanzado ninguna llamada real).
>
> Fecha: 2026-10-31
>
> Origen: candidata C2 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
>
> Patrón afín (agentpatterns.ai, CC BY 4.0):
> [Reasoning Budget Allocation](https://agentpatterns.ai/patterns/agent-design/reasoning-budget-allocation/).
> Relacionado: `2026-10-31-codeloom-delegation-control.md`, `docs/references/2026-10-31-medicion-real-reproducir-antes-de-arreglar.md`.

## 1. Problema

El patrón sostiene que conviene más esfuerzo de razonamiento al planificar y revisar, y menos al ejecutar. Su evidencia
es una sola comparación en un banco (66,5 % frente a 63,6 % y 53,9 %). El perfil de agente ya admite `effort`, pero los
perfiles de demostración no lo fijan, y la tabla que traduce un nivel al presupuesto de razonamiento de Anthropic
(`anthropic_budget`: 1.024, 2.048, 8.192, 16.384 y 32.000 tokens) está marcada en el código como **suposición pendiente de
contrastar con el servicio real**. No debe fijarse ningún valor por defecto sin medir antes.

## 2. Objetivo

Responder con datos propios, a pequeña escala y con coste acotado, a tres preguntas:

1. ¿`effort` llega de verdad al modelo y produce razonamiento? (comprobación de la tabla.)
2. En una fase de **revisión**, ¿detecta más defectos con esfuerzo alto que sin él, y a qué coste por acierto?
3. En una fase de **ejecución** con herramientas, ¿un esfuerzo bajo cambia el resultado o solo el coste?

## 3. No objetivos

- No se cambian valores por defecto de perfiles ni del código; el resultado solo informa una decisión posterior.
- No se concluye una regla general: la muestra es pequeña y de un solo modelo. Sirve para ver magnitudes y descartar
  efectos enormes, como la medición anterior.
- No se mide la fase de planificación aparte: se asimila a la revisión (ambas son juicio sobre texto sin herramientas).
- No se compara entre proveedores ni modelos. Un solo modelo: Haiku 4.5 por OpenRouter, adaptador de Anthropic.

## 4. Diseño

### 4.1 Variables

- **Esfuerzo:** `none` (sin `effort`, comportamiento actual), `low` (2.048), `high` (16.384). `medium` solo se añade si
  la etapa 1 muestra señal entre `none` y `high`.
- **Modelo:** `anthropic/claude-haiku-4.5` por `https://openrouter.ai/api` con `AnthropicProvider`, como en la medición
  anterior.
- **Confusión conocida:** con `thinking` la API omite la temperatura; sin él se usa 0,0. Se anota, no se corrige.

### 4.2 Etapa 0: comprobación (≈ 2 llamadas)

Una misma petición de revisión con y sin `effort = high`. Se exige que con esfuerzo suban los tokens de salida respecto
al caso sin él. Si no suben, la tabla no surte efecto en este servicio y el benchmark se **detiene**: el hallazgo es ese.

### 4.3 Etapa 1: revisión sin herramientas

Seis fragmentos Python breves, cada uno con una petición de revisión que pide una lista de hallazgos o la marca literal
`NO_DEFECTS`. Cinco llevan un defecto sembrado, sutil pero localizable; uno está limpio.

| Caso | Defecto sembrado |
|---|---|
| 1 | Paginación con un elemento de más (desfase de uno) |
| 2 | Argumento mutable por defecto compartido entre llamadas |
| 3 | Consulta SQL construida con una cadena formateada (inyección) |
| 4 | `except Exception: pass` que oculta un fallo y devuelve éxito |
| 5 | Comparación `<=` donde la docstring exige estricto |
| 6 | Limpio: la respuesta correcta es `NO_DEFECTS` |

Puntuación determinista, sin juez: un caso con defecto cuenta como **acierto** si la respuesta casa una expresión
regular fijada de antemano para ese defecto; el limpio cuenta como acierto si responde `NO_DEFECTS` y como **falso
positivo** si no. Las respuestas se guardan para revisarlas a mano: la regex es una cota, no el juicio final.

Matriz: 6 casos × {`none`, `high`} × 2 repeticiones = 24 llamadas.

### 4.4 Etapa 2: ejecución con herramientas

Los dos fallos de Python reales de `reproduce_first_live` con el agente completo (`view`, `edit`, `bash`), solo con
`low`. El punto de comparación sin esfuerzo es la medición ya publicada (3/3 resueltas, 9 llamadas, ≈ 0,079 USD por
ejecución); se reutiliza porque el modelo, los ficheros y la tarea son los mismos. Matriz: 2 fallos × 2 repeticiones =
4 ejecuciones. Se registra: resuelto, llamadas a herramientas, coste.

### 4.5 Criterios de decisión (fijados antes de medir)

- **Revisión:** se considera que el esfuerzo alto «compensa» si mejora los aciertos en al menos 25 puntos porcentuales
  sin subir los falsos positivos, y se anota el coste por acierto. Menos que eso, o con n tan pequeño, se declara
  **inconcluso**, no negativo.
- **Ejecución:** si ambas variantes resuelven por igual y `low` sube el coste un 30 % o más, **no se recomienda** subir
  el esfuerzo en ejecución. Si no resuelve más, el esfuerzo en ejecución no se justifica con estos datos.
- Cualquier recomendación se redacta como hipótesis a validar con una muestra mayor, no como regla.

### 4.6 Acotación del gasto

- Tope global de la tarea: **1,00 USD** (autorización de la persona usuaria el 2026-10-31).
- Topes por etapa: 0,05 (etapa 0), 0,40 (etapa 1), 0,35 (etapa 2); reserva de 0,20 para repetir o ampliar.
- El código acumula el coste comunicado tras cada llamada y se detiene antes de la siguiente si se supera el tope de su
  etapa. Nada se lanza sin autorización expresa de ejecutar el plan.
- La clave se lee solo del entorno del proceso de la prueba y no se imprime.

## 5. Criterios de aceptación

1. La prueba está marcada `#[ignore]` y no hay red real en el resto de la suite.
2. Las expresiones regulares y el caso limpio tienen pruebas unitarias sin red (acierta con el defecto, falla sin él).
3. El acumulador de coste detiene la etapa al superar su tope (prueba sin red con costes simulados).
4. Cada etapa imprime una línea de resumen por variante: aciertos, falsos positivos, coste, tokens de salida.
5. Se publica un documento de medición con datos, confusiones y una conclusión en el vocabulario del §4.5.
6. Las cuatro comprobaciones del proyecto pasan.

## 6. Riesgos y preguntas abiertas

- Con `n` = 12 llamadas por variante, un acierto de más o de menos mueve ocho puntos: es muy fácil quedar en «inconcluso».
- Haiku 4.5 puede resolver todos los casos sin esfuerzo (efecto techo, como en la medición anterior). Sería un resultado
  válido: indicaría que para este modelo y esta dificultad el esfuerzo no aporta.
- Los defectos sembrados son escolares; no representan revisiones reales de código ajeno.
- El coste del razonamiento depende del uso real del presupuesto, no de su tope: el gasto estimado es un orden de
  magnitud, y el tope de etapa es el límite duro.
