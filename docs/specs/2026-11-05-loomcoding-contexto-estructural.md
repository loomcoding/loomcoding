# Contexto estructural: más contexto con menos llamadas

Estado: propuesta (2026-11-05). Plan: `docs/plans/2026-11-05-loomcoding-contexto-estructural.md`. Es la línea 3 (con la 2 como segundo paso) de
`2026-11-05-loomcoding-mejorar-el-sistema-del-modelo-pequeno.md`. Complementa, sin sustituirla, a
`2026-11-05-loomcoding-verificar-tras-editar.md` (línea 1).

## Problema

Un modelo pequeño gasta casi todo el turno en **orientarse**, no en trabajar. Medido con el piloto del
2026-11-05 (llamadas a herramientas del junior):

| Ejecución | Llamadas | Reparto |
|---|---|---|
| Tarea 6, Strata (1 edición, documental) | 28 | 10 `bash`, 8 `grep`, 4 `view`, 2 `glob`, 4 `edit` |
| Tarea 6, Qwen3.5-9B | 10 | 6 `bash`, 3 `view`, 1 `edit` |
| A1, Strata (a los 862 s, sin ninguna edición) | 26 | 15 `view`, 7 `grep`, 2 `bash`, 1 `glob`, 1 `todo_write` |

Cada `view` o `grep` es una ronda de modelo: con Strata, entre 10 y 110 s; y el contexto crece con cada
resultado, de modo que cuesta más la siguiente. Con ventanas pequeñas (8192 en el 4B, 32768 en el 9B)
además se compacta antes. El modelo no sabe **qué fichero y qué símbolo le importan**, y lo descubre a
base de leer.

## Idea

Dar al modelo, en **una sola llamada**, el contexto estructural que hoy reconstruye con seis o quince:
los símbolos relevantes para su encargo, con firma y ubicación, lo que los usa y lo que usan, sus
pruebas, y la sección del mapa del repositorio que corresponda. Y hacer que la oficina lo ofrezca sin
depender de que el modelo se acuerde.

## Decisión

Dos piezas, en este orden:

1. **Herramienta `context`** (de solo lectura, `Parallel`, como `grep`).
   - Entrada: `query` (nombre de símbolo, ruta o frase corta) y, opcional, `budget` (tope de caracteres,
     con un máximo duro).
   - Salida: un paquete **acotado** con, por símbolo relevante: ruta y línea, firma, un extracto de su
     cuerpo con tope, quién lo llama y a quién llama (nombres con ruta y línea, no cuerpos), pruebas
     que lo mencionan y, si existe, la sección de `docs/CODE-MAP.md` que lo cita («Quiero tocar…»).
   - Fuente: un índice de símbolos propio, construido con un analizador sintáctico (propuesta:
     `tree-sitter` para Rust), en memoria y con caché por fecha de fichero. **No** se reutiliza
     `rust-analyzer` por llamada: `lsp` arranca desde cero cada vez (10 s de inicialización) y mira
     un solo fichero. El `lsp` actual se conserva; `context` puede consultarlo después para referencias
     exactas si la medición lo justifica.
   - Fuera de Rust, o sin índice, degrada a una búsqueda textual acotada y lo dice.
2. **Orientación previa en la oficina** («obligar»). Al empezar un turno de usuario o de subagente, la
   oficina calcula el paquete a partir del encargo (rutas y nombres que cita, más la sección del
   `CODE-MAP.md` que mejor encaja) y lo adjunta como sección acotada del prompt, marcada como
   orientación y con su origen. El modelo recibe de entrada lo que hoy gasta 10 rondas en encontrar.
   Es configurable (`[context]`), acotada por el decálogo y apagable.

Un tercer paso, **edición por símbolo** (`edit_symbol`, línea 2: direccionar `Tipo::método` en vez de
cadenas de texto), queda fuera de este spec y se abre aparte cuando haya medición de los dos primeros.

## Por qué sin entrenar y por qué en la oficina

El «obligar» no es de prompt: la orientación se inyecta siempre, y la herramienta `context` figura en la
guía de herramientas del prompt con una instrucción corta («antes de `grep`/`view` para localizar, usa
`context`»). Con un modelo de 4B o de 9B, lo que no está en el contexto de entrada no se hace.

## Criterios de aceptación

1. `context` devuelve, para un símbolo existente del repositorio, firma, ubicación, usos y pruebas, y
   nunca supera su tope; para uno inexistente lo dice y propone los más parecidos.
2. El índice se actualiza al cambiar un fichero y no se reconstruye entero en cada llamada.
3. La orientación previa se adjunta una vez por turno, con tope, y no se repite al despertar al padre.
4. Con el índice apagado o sin Rust, todo sigue como hoy.
5. Medición con el banco del spec madre (tareas 6 y A1, 9B y Strata, tres repeticiones): **menos
   llamadas de orientación (`grep`+`view`+`glob`), menos tokens de entrada y no peor corrección**.
   Si no baja la orientación, se descarta.

## No objetivos

- Embeddings ni base vectorial: de entrada, índice sintáctico y búsqueda léxica.
- Otros lenguajes distintos de Rust en la primera entrega.
- Reescribir `lsp`, `grep` o `view`.

## Referencias estudiadas (clones en `G:\loom-references\`, anotados en su `REGISTRY.md`)

Cotejado leyendo el código de los clones, no el resumen de un tercero:

- **`aider` (`aider/repomap.py`, Apache-2.0)**: extrae definiciones y referencias de cada fichero con
  `tree-sitter` y las consultas de etiquetas (`tags.scm`), construye un grafo fichero a fichero, lo
  ordena con PageRank personalizado hacia los ficheros del encargo y pinta el resultado dentro de un
  presupuesto de tokens. Las etiquetas se cachean por fecha de fichero. Es el modelo más próximo de
  `context`: índice sintáctico, ranking y tope. En Rust el PageRank son unas decenas de líneas propias.
- **`ACAR` (`graph_retrieval/`, `ranking/`, MIT)**: recuperación híbrida. La parte estructural resuelve
  los identificadores cercanos al punto de trabajo hasta sus definiciones, sigue **una** llamada anidada
  (profundidad 2, hasta 10 cuerpos) y funde el resultado con la recuperación léxica mediante fusión de
  rangos recíprocos. Declara 87 a 95 ms de sobrecarga con cachés calientes (cifra suya, sin reproducir).
  Confirma que la profundidad y el número de cuerpos deben ir **acotados**.
- **`tree-sitter` y `tree-sitter-rust`**: la gramática trae `queries/tags.scm`, que ya define qué es una
  definición y qué una referencia en Rust; no hay que escribirlas.

Licencias: se estudian las ideas; si se llegara a copiar código de `aider` (Apache-2.0) se atribuye y se
declara en el commit.

## Sondeo de coste (2026-11-05, tarea 0 del plan)

Proyecto desechable fuera del repositorio, `-j 2`, mientras Strata servía otra tarea (CPU compartida):

- Dependencias: `tree-sitter` **0.25** (con la 0.24 la gramática `tree-sitter-rust` 0.24.2 falla: versión de
  lenguaje 15 frente a máximo 14), `tree-sitter-rust` y `streaming-iterator`. Compilación en limpio:
  40 s; binario de sondeo: 3,1 MB. `TAGS_QUERY` no es público en la gramática: se copia `queries/tags.scm`.
- Este repositorio: 635 ficheros `.rs`, 4,7 MB, 10 963 definiciones, 47 085 referencias, **0** ficheros con
  error sintáctico.
- Tiempo: 27 s la primera pasada (disco en frío) y 3,0 s con el disco caliente (análisis 1,7 s, consulta
  0,8 s, recorrido y lectura el resto). Sale **por encima del umbral de 2 s** del plan, aunque la medición está
  contaminada por la carga de Strata; se repite con el equipo en reposo.
- Consecuencia de diseño, válida con cualquier resultado: el índice no se reconstruye entero en cada
  llamada; se construye una vez, se **persiste** en disco por fecha de fichero (tarea 2) y se completa en
  segundo plano con plazo, devolviendo un paquete parcial declarado mientras tanto.

## Riesgos

- Un paquete mal elegido **distrae** a un modelo pequeño más de lo que ayuda: de ahí el tope bajo, el
  origen visible y la medición como puerta.
- Coste en dependencias y en tiempo de compilación de `tree-sitter`: se mide antes de aceptarlo.
- El índice puede quedar desfasado frente a ediciones del propio turno: se invalida por fecha de fichero.

## Preguntas abiertas

- ¿La orientación previa la calcula la oficina a partir de texto libre o solo de rutas y nombres
  explícitos? Lo segundo es más seguro; lo primero, más útil.
- ¿Qué tope por defecto para cada ventana (8192, 32768, 65536)?
- ¿Se ofrece también a los subagentes por defecto, con su contexto aún más pequeño?
