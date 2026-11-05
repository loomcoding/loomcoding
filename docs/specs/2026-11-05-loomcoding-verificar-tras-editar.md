# Verificar tras editar: herramienta `check` y puerta de cierre

Estado: propuesta (2026-11-05). Plan: pendiente. Es la línea 1 de
`2026-11-05-loomcoding-mejorar-el-sistema-del-modelo-pequeno.md`.

## Problema

El piloto A1 con el Qwen3.5-9B entregó pruebas que no compilan (variable sin crear, ayudante inventado,
campo privado) y un resumen que no lo decía. Nadie ejecutó el compilador antes de dar la tarea por
buena; el modelo tampoco, porque nada se lo exige. Pedírselo en el prompt no basta con un modelo pequeño.

## Qué hay y qué falta (cotejado con el código)

- `lsp` (`crates/loomcoding-core/src/tools/lsp.rs`) ya existe: lanza `rust-analyzer` **en cada llamada**
  (arranque en frío, 10 s de espera al inicializar y 3 s para los diagnósticos), solo ficheros `.rs`, y
  devuelve diagnósticos de **un fichero**. Sirve para consultar símbolos; como comprobación de «¿compila
  el crate y pasan sus pruebas?» es insuficiente y lenta. Está por medir cuántos errores de tipos y de
  visibilidad (justo los de A1) detecta de verdad.
- `bash` permite correr `cargo`, pero depende de que el modelo lo quiera, acote la salida y la lea.
- No hay nada que **obligue** a comprobar.

## Decisión

Dos piezas, ambas en el núcleo y sin depender de que el modelo se acuerde:

1. **Herramienta `check`** (como `bash` o `write`, de solo lectura salvo el directorio de `target`).
   Parámetros: `scope` (`build` | `test`), `package` opcional. Ejecuta `cargo check` o
   `cargo test --no-run` con salida corta (`--message-format short`), **acotada** (decálogo: tope de
   caracteres y de errores, los primeros errores primero), con plazo y cancelación, y devuelve
   «compila» o la lista compacta de errores con fichero y línea.
2. **Puerta de cierre en la oficina.** El bucle del agente marca como «sin verificar» cualquier turno
   en el que se haya editado un `.rs` (`edit`, `multiedit`, `write`) y no haya habido después un `check`
   en verde. Si el modelo intenta dar respuesta final en ese estado, la oficina no la acepta: inyecta un
   aviso («has editado X y no has comprobado que compila; ejecuta `check`») y continúa. Acotada: como
   mucho `N` avisos por turno (propuesta: 2); agotados, el turno termina y el informe final lo declara
   («terminó sin verificar»), de modo que nunca se oculta.

La puerta es el «obligar»: no depende del prompt. Convive con los topes de pasos y con el informe de
turno sin respuesta final (`TurnOutcome::unfinished_report`).

## Alcance

- Rust, con `cargo`. Otros lenguajes quedan fuera hasta que haya otro verificador.
- Los permisos siguen el modelo existente: `check` pide permiso como `bash` cuando corresponda.
- Los avisos de la puerta aparecen en el transcript como el resto de avisos (la conversación es el
  diario).

## Criterios de aceptación

1. `check` devuelve «compila» en un proyecto sano y, en uno roto, los errores acotados con fichero y
   línea; la salida nunca supera el tope.
2. Un turno que edita un `.rs` y quiere terminar sin `check` recibe el aviso y continúa.
3. Tras un `check` en verde, o sin ediciones de `.rs`, la puerta no interviene.
4. Agotados los avisos, el turno termina y el informe dice que quedó sin verificar.
5. La cancelación interrumpe `check` y la puerta.
6. Medición con el banco del spec madre: A1 con el 9B, con y sin puerta, comparando si el diff compila.

## No objetivos

- Sustituir al revisor: la puerta comprueba que compila, no que esté bien.
- Ejecutar toda la batería de pruebas del repositorio en cada edición (coste): `scope=test` solo
  compila las pruebas (`--no-run`); ejecutarlas queda como opción posterior.

## Preguntas abiertas

- ¿Cuántos avisos antes de rendirse? ¿Cambia con el esfuerzo o con el tamaño del modelo?
- ¿Debe la puerta lanzar el `check` por su cuenta, sin que el modelo lo pida? Ahorra un paso del modelo,
  pero esconde una acción; hay que decidirlo a la vista de la medición.
- ¿Se reutiliza `rust-analyzer` persistente en vez de `cargo`, si la medición de `lsp` lo justifica?
