# Tareas para un modelo local: tarjetas de un plan y verificación objetiva — spec del experimento

Estado: **propuesta** (borrador, sin plan ni código). Nace de los resultados del laboratorio
`agent-task-engineering-poc` (fuera de este repositorio): un Qwen2.5-Coder 7B resolvió el bug de prueba el 100 % de las
veces con una sola llamada, de modo que **ninguna descomposición pudo mostrar mejora**, solo sobrecoste. Esta spec
define qué probar a continuación y qué tendría que salir para que Loomcoding cambie algo. Relacionadas:
`2026-11-04-loomcoding-gauntlet-minimo.md` (ejecutor con `check`), `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`
y `2026-11-06-loomcoding-compresion-de-salida.md` (la salida de las herramientas también cuesta contexto a un modelo
pequeño).

## Problema

Un modelo local de 3B a 8B (ventana efectiva de unos 4 000 tokens) no puede ejecutar un plan entero como lo hace un modelo
grande. El PoC probó métodos en los que **el propio modelo pequeño planifica** (EffGen, Agentless, descomposición
estática): añadieron llamadas, tokens y tiempo sin mejorar el éxito, en una tarea donde el monolítico ya acertaba. Eso
no refuta la descomposición: la tarea estaba **en el techo**. Tampoco prueba nada sobre el caso que nos interesa, que
es otro: *quién* descompone.

## Hipótesis (a demostrar o refutar, no a asumir)

- **H1 — Descomponer arriba, ejecutar abajo.** Si un modelo fuerte descompone **una vez** (spec → plan → tarjetas
  pequeñas) y el modelo local solo ejecuta tarjetas, el local resuelve más tareas difíciles que con la spec entera,
  incluso contando el coste de planificar.
- **H2 — La verificación objetiva vale más que el crítico.** Con tests o un comando `check` como autoridad y reintentos
  que **cambian de enfoque** tras un fallo repetido, el éxito sube sin necesitar un crítico (modelo) que juzgue.
- **H3 — Quién descompone importa.** Las tarjetas hechas por el propio modelo local rinden menos que las del fuerte.
  Es lo que el PoC no separó.

Cualquiera puede salir falsa; el resultado «no ayuda» es válido y se publica igual.

## La tarjeta de tarea

Unidad mínima que se entrega al modelo local. Cabe en el presupuesto de contexto (prompt ≤ 3 000 tokens medidos, el
resto para la respuesta). Campos:

1. **Objetivo** en una o dos frases y criterio de aceptación observable.
2. **Ficheros permitidos** para escribir (el runtime rechaza cualquier otro) y **ficheros de contexto** de solo lectura,
   ya recortados a lo necesario.
3. **Interfaz**: firmas exactas de lo que la tarjeta consume y de lo que debe producir (lo que otras tarjetas
   necesitan).
4. **Prueba** que debe pasar, y el comando `check` que la ejecuta.
5. **Lo que no se hace** (no tocar X, no añadir dependencias).

La tarjeta **no incluye la solución**. Es la regla que más cuida el experimento: un plan con el código completo (como
los que usamos con Haiku) convierte la tarea en copiar, y no mide nada. El nivel de detalle (solo firmas, firmas más
esquema del cuerpo, cuerpo completo) es una variable declarada, no un accidente.

### Cómo se produce y se valida

El modelo fuerte escribe el plan; un paso **determinista** lo compila a tarjetas y las rechaza si:
- el prompt resultante supera el presupuesto;
- un fichero permitido o de contexto no existe;
- la prueba **no falla antes** de la tarjeta (se ejecuta y se registra el rojo) o ya pasa;
- hay una tarjeta que consume una interfaz que ninguna anterior produce.

Una tarjeta mal formada se devuelve al planificador, no se manda al modelo local.

## Brazos del experimento

Mismas tareas, mismo modelo local (versión, cuantización, ventana, temperatura y semilla fijas), mismo evaluador.

| Brazo | Qué recibe el modelo local | Qué mide |
|---|---|---|
| **A. Monolítico** | La spec entera y el contexto del repositorio recortado | La línea base; solo es válida la tarea con éxito inferior al umbral (ver abajo) |
| **B. Tarjetas** | Una tarjeta cada vez, en orden, sin reintento | El efecto de descomponer arriba (H1) |
| **C. Tarjetas + `check`** | Igual, con reintento guiado por la salida del `check` y aviso de «cambia de enfoque» si el fallo se repite | El efecto de la verificación objetiva (H2) |
| **D. Tarjetas del propio local** | Tarjetas hechas por el modelo local a partir de la spec | Quién descompone (H3) |

Cada brazo cuenta con **el coste del modelo fuerte** que planificó (tokens y tiempo, una sola vez por tarea), aunque el
local sea gratis: una mejora que cuesta más que ejecutar la tarea con el fuerte no es mejora.

## Tareas

- **Criterio de entrada**: el brazo A acierta **menos de la mitad de las veces** en una muestra de calibración. Una tarea
  en el techo se descarta y se anota (es justo lo que falló en el PoC).
- **Tipos**: funcionalidades de varios ficheros y refactors, no solo errores de una línea.
- **Origen**: (a) tareas generadas de forma procedural sobre fixtures desechables, con revisión independiente, como
  prevé el plan transversal del PoC; (b) **commits reales de este repositorio** con spec, plan y pruebas, partiendo del
  commit anterior, porque traen una barra de calidad ya escrita.
- **Fuga**: ni las pruebas privadas, ni el parche de referencia, ni los nombres de la solución aparecen en lo que ve el
  modelo. Para los commits reales se comprueba que el parche no esté ya en el contexto entregado.
- **Piloto**: seis tareas (dos de un fichero, cuatro de varios), tres repeticiones por brazo. Es exploratorio, no
  estadístico.
- **Divisiones**: una de desarrollo (para afinar el formato de tarjeta) y otra congelada de evaluación, que se fija antes
  de ver resultados.

## Métricas

Resolución de la tarea completa con **pruebas privadas** (la decide el runtime, nunca el modelo); acierto por tarjeta;
**errores de composición** (tarjetas que pasan solas y rompen la integración); reintentos y tasa de «el mismo error
repetido»; tokens y tiempo (local y fuerte, por separado); fallos de formato de la respuesta; y, por tarea, cuántas
tarjetas hizo falta repartir.

## Criterio de decisión (se fija antes de correr)

Se declaran por escrito, antes de la primera ejecución, el umbral de mejora que haría adoptar B o C (diferencia mínima de
resolución sobre A y coste total máximo) y el que las descartaría. Con tres repeticiones a temperatura 0 y una semilla
no se afirma significancia: se informa de tamaño de muestra y variación, y un resultado cercano al umbral se repite con
más tareas, no se interpreta.

## Si la hipótesis se sostiene (no antes)

- Un **compilador de tarjetas** determinista en un crate opcional, no en el núcleo de la interfaz.
- Un **ejecutor** con el bucle de `gauntlet` sin crítico para código con `check`: hoy `gauntlet` exige crítico y
  aceptación humana (reglas 1 y 7 del spec mínimo), así que sería un modo nuevo o una herramienta hermana, **a decidir**.
- Cada tarjeta se lanza como subagente local y sigue siendo una tarea A2A: no se inventan estados; lo propio
  («tarjeta 3 de 7», reintento) se rotula como propio.
- La salida de herramientas de ese ejecutor pasa por la capa de compresión, con sus reglas de no perder señal.

## Riesgos

1. **Volver al techo**: tareas fáciles otra vez. Se evita con el criterio de entrada.
2. **Plan que trampea**: tarjetas que filtran la solución. Se evita con la regla de no incluirla y la comprobación de fuga.
3. **Composición**: cada tarjeta pasa y el conjunto no. Se mide aparte y hay una prueba de integración al final.
4. **Sobreajuste a la prueba** del `check`: el modelo satisface el test sin resolver el problema. Las pruebas privadas de
   evaluación son otras.
5. **El planificador fuerte se equivoca** y propaga un error a todas las tarjetas. Cuenta como fallo del brazo, no se
   corrige a mano.
6. **Muestra pequeña**: se declara, no se disimula.
7. **Medir lo equivocado**: ahorrar tokens del local no es objetivo; lo es resolver más tareas con menos coste total.

## Qué no es

No es construir otro agente genérico, ni usar el modelo local como crítico, ni cambiar el producto antes de tener
números. No sustituye el modelo fuerte en tareas donde este es barato y fiable.

## Preguntas abiertas

1. ¿Qué modelo fuerte planifica y cómo se contabiliza su coste (tokens, dinero, tiempo)?
2. ¿Qué nivel de detalle de tarjeta se prueba primero (solo firmas, firmas y esquema, cuerpo completo)?
3. ¿Se ejecuta el experimento en el laboratorio del PoC (Node, aislado) o dentro de Loomcoding con proveedores locales?
   Lo primero ya tiene evaluador aislado; lo segundo prueba el producto real.
4. ¿Modo sin crítico en `gauntlet` o ejecutor aparte?
5. ¿Umbral de adopción? Hay que fijarlo antes de correr nada.
6. Pendiente de leer: `2026-10-25-codeloom-agent-development-workflow.md`, el hermano más cercano de `gauntlet`; podría
   cubrir parte de esto.
