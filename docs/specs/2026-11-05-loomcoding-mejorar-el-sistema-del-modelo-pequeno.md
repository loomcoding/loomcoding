# Mejorar el sistema que rodea al modelo pequeño

Estado: propuesta (2026-11-05). Sin plan ni código. Cabe señalar que es un documento de **método**: fija
qué se mide y en qué orden, no qué se construye. Cada técnica candidata exigirá su propio spec antes de
tocar código.

## Problema

Los modelos locales (Qwen3.5-9B en `llama-server`, Qwen3.8-Flash-Next-Coder IQ1_M en Strata, Qwen3.5-4B en
la RX 580) fallan en tareas de código de varios pasos aunque acierten en las mecánicas. Ejemplos medidos
el 2026-11-05 con el piloto (`G:\temp\loom-qwen\autonomia`):

| Tarea | Qwen3.5-9B | Strata |
|---|---|---|
| Tarea 6 (documentación, una sola edición) | 69 s, diff idéntico al encargo | 1280 s, diff idéntico al encargo |
| A1 (`run_until_settled`, código con 5 pruebas) | fracaso: pruebas que no compilan, helper inventado | en curso |

Se trata de averiguar **hasta dónde llega un modelo pequeño cuando el agente que lo usa está bien
diseñado**, en lugar de resolverlo cambiando de modelo. Es coherente con el esquema de trabajo vigente:
el junior implementa, el revisor comprueba, y la oficina (Loomcoding) debe funcionar como un reloj.

## Qué existe ya en Loomcoding (cotejado con el repositorio)

- Edición textual: `edit`, `multiedit`, `write` (`crates/loomcoding-core/src/tools/`).
- Herramienta `lsp` (`tools/lsp.rs`): referencias y símbolos para quien la use.
- `effort` configurable en `loomcoding.toml` y batería de razonamiento
  (`2026-10-31-codeloom-reasoning-effort-hard-battery.md`).
- Topes de pasos, de lectura y de peticiones (`[limits]`), informe de turno sin respuesta final.
- Gauntlet mínimo (constructor y crítico ciego), `2026-11-04-loomcoding-gauntlet-minimo.md`.
- Lanzador de pilotos con vigilancia de RAM y tiempo (fuera del repositorio).
- **No existe**: análisis por árbol sintáctico (`tree-sitter`), ni ciclo automático de verificación tras
  editar, ni control adaptativo del presupuesto de razonamiento.

## Líneas candidatas

Las cifras que siguen proceden de un texto externo y **no se han cotejado con los artículos originales**;
sirven para priorizar, no para prometer resultados. Antes de implementar cada una se lee la fuente.

| # | Línea | Qué cambia | Reentrenar | Viable ya |
|---|---|---|---|---|
| 1 | Ciclo de ejecución y pruebas | tras editar, el agente ejecuta `cargo check`/`test` y recibe el error acotado | no | sí |
| 2 | Edición estructural | herramientas que direccionan funciones y tipos (`Servicio::metodo`) en vez de cadenas | no | sí |
| 3 | Contexto estructural | recuperar solo símbolos y dependencias relevantes (árbol sintáctico + `lsp`) | no | sí |
| 4 | Presupuesto adaptativo | poco razonamiento en lo sencillo, más intentos y verificación en lo difícil | no | sí |
| 5 | Decodificación especulativa | acelerar la generación | según método | depende del motor |
| 6 | Infilling, BitNet, razonamiento latente | cambian el modelo | sí | no (investigación) |

Las filas 1 a 4 mejoran el **sistema**; la 5 depende del servidor de modelos (que levanta el usuario) y la
6 queda fuera de alcance. Prioridad propuesta: 1, luego 2 y 3, luego 4.

La línea 4 tiene un dato local que la justifica: con Strata el tiempo se va en razonamiento a unos 18
tokens por segundo (16 243 tokens de salida frente a 2 754 del 9B en la tarea 6, con la caché de prefijo
funcionando). Queda por comprobar si el proveedor permite fijar el esfuerzo de razonamiento por petición.

## Método: una variable cada vez

Para no mezclar efectos, cada técnica se mide contra la **misma** línea base y con las mismas tareas.

- **Variantes**: base (sin cambios) · base + una técnica · (más adelante) combinaciones.
- **Tareas**: lote fijo de encargos del propio repositorio, escritos como `tarea-N.md`, con el resultado
  esperado comprobable por `cargo` y por `diff` (no por el resumen del modelo). Incluye al menos: una
  documental (tarea 6), una de código con pruebas (A1) y una de arreglo acotado. Se sustituye la
  propuesta externa de 30 incidencias de Node.js por tareas de este repositorio, que es en Rust.
- **Métricas por ejecución**: terminó o no (y cómo se cortó), tiempo total, rondas del modelo, tokens de
  entrada y salida, llamadas a herramientas, si compila, si pasan las pruebas, si el diff cumple el
  encargo y si el informe final es veraz.
- **Repeticiones**: tres por variante y tarea cuando el tiempo lo permita; una sola ejecución no basta
  (con Strata hubo tres vueltas rotas antes de una limpia).
- **Veredicto**: lo da quien revisa, con el diff y `cargo`, nunca la autoevaluación del junior.

## Fases

1. **Línea base**: congelar tareas, binario y configuración; medir 9B y Strata (y 4B cuando toque).
2. **Ciclo de ejecución y pruebas** (línea 1): primer experimento, el de menor coste y mayor efecto
   esperado sobre fallos como los de A1 (pruebas que no compilan).
3. **Edición y contexto estructurales** (líneas 2 y 3): requieren decidir antes el analizador
   (`tree-sitter` para Rust) y su coste en dependencias; spec propio.
4. **Presupuesto adaptativo** (línea 4): clasificador de dificultad y esfuerzo por petición.

## No objetivos

- Entrenar o afinar modelos.
- Cambiar la configuración de los servidores de modelos o lanzar cargas contra la RX 580 (GPU del monitor).
- Sustituir el Gauntlet: el crítico ciego y este banco miden cosas distintas.
- Pipeline definitivo: el esquema de la fuente externa (clasificador, contexto, generación, validación)
  es un diseño propuesto, no uno medido.

## Restricciones

- Toda salida, bucle y presupuesto con cota (decálogo de contexto y coste).
- Los resultados se guardan fuera del repositorio hasta que haya conclusiones; las conclusiones sí se
  documentan aquí.
- Los pilotos usan `LOOMCODING_HOME` aislado y el lanzador vigilado.

## Preguntas abiertas

- ¿Qué tamaño de lote da una conclusión creíble sin gastar horas de GPU por variante?
- ¿Debe el ciclo de verificación ser una herramienta del modelo o una acción de la oficina tras cada
  edición? Lo segundo no depende de que el modelo se acuerde.
- ¿Admite el servidor de Strata fijar el esfuerzo de razonamiento por petición?
- ¿Cuánto cuesta en dependencias y en tiempo de compilación incorporar `tree-sitter`?
