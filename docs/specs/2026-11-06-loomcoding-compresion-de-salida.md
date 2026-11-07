# Compresión de la salida de las herramientas — spec

Estado: **propuesta** (sin plan ni código). Nace de una pregunta del usuario: si Loomcoding podría aplicar reglas de
compresión como las de `rtk` o `headroom`. Complementa el decálogo de contexto y coste
(`2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`, reglas 1 a 4) y la medición de
`2026-10-14-loom-kpis-prompt-compression.md`.

## Problema

Hoy la salida de una herramienta se acota **a lo bruto**: `limit_bytes` conserva cabeza y cola y avisa del corte
(`MAX_TOOL_OUTPUT_BYTES`, 20 000 bytes). Eso evita el desastre, pero no mira qué se descarta. Una salida de `cargo test`
con 400 líneas `... ok` y un único fallo en medio gasta 20 000 bytes de contexto y, con mala suerte, **el corte se lleva
el fallo**. Cada paso posterior reenvía esa salida entera.

Lo que existe y no se toca: cota de bytes, limpieza de resultados antiguos (`clear_old_tool_results`) y resumen del
historial con el modelo (`/compact`). Lo que falta es la capa intermedia: **reducir la salida con criterio antes de
acotarla**.

## Intención

Una capa determinista, sin modelo y sin red, que transforma la salida de ciertas herramientas en otra más corta que
conserva la señal. Es la idea de `rtk` (filtros por comando) aplicada dentro del núcleo, donde el transcript y lo
enviado siguen siendo lo mismo.

### Qué entra

1. **Reglas generales**, para cualquier salida de texto:
   - quitar secuencias ANSI y retornos de carro de barras de progreso;
   - colapsar líneas idénticas consecutivas en una sola con «×N»;
   - colapsar líneas en blanco consecutivas.
2. **Reglas por comando**, reconocidas por el primer ejecutable de `bash` / `job_start`: `cargo` (`build`, `test`,
   `clippy`), `git` (`status`, `diff`, `log`), gestores de paquetes (`npm`/`pnpm` install). Cada regla declara qué es
   señal (errores, avisos, fallos, resumen final) y qué es ruido (compilando…, `ok`, progreso).
3. **Dedup de lecturas en el historial**: si el modelo lee el mismo fichero dos veces sin que haya cambiado, la copia
   antigua se sustituye por una referencia breve a la lectura nueva.

### Qué no entra

- **Un proxy tipo `headroom`.** Duplicaría esto y rompería la observabilidad: el transcript dejaría de coincidir con lo
  enviado al proveedor. Además, la versión 0.37 daba `400 system.4.type` con Haiku.
- Cualquier compresión que use un modelo (eso ya es `/compact`).
- Compresión con pérdida de lo que el usuario escribe, o del código que el modelo lee a propósito (`read`, `grep`
  devuelven lo pedido; solo se aplica la dedup, nunca un filtro de contenido).

## Ojo con lo que perdemos

> Esta sección es la razón de que haya spec. Un filtro que esconde justo la línea que importaba es peor que no filtrar:
> el modelo decide con una verdad recortada y **no sabe que le falta algo**.

**Riesgos conocidos**

1. **El filtro tira la línea decisiva.** Una regla de `cargo test` que descarta `ok` pero mal escrita descarta también
   `test x ... FAILED`; un patrón de «progreso» demasiado ancho se come un mensaje real. Es el fallo principal.
2. **Señal que parece ruido.** Un aviso (`warning:`) que hoy no importa y mañana es el motivo del fallo; la línea
   repetida ×300 que es precisamente el síntoma (un bucle de reintentos). Colapsar oculta la **frecuencia** si no se
   anota.
3. **Depender del formato.** Las reglas por comando se apoyan en el texto que imprime la herramienta, que cambia entre
   versiones, idiomas (`LANG`) y configuraciones (`--message-format`, colores, `RUST_LOG`). Un cambio de formato
   convierte un filtro correcto en uno que descarta de más **sin avisar**.
4. **Mentir por omisión.** Si el modelo cita «no hay errores» porque el filtro los quitó, el fallo es nuestro, no suyo.
5. **Romper la observabilidad.** Si el usuario ve una cosa en el transcript y el modelo recibió otra, o al revés, ya no
   se puede depurar a un agente (valor central de Loomcoding). Tampoco se puede reproducir un turno.
6. **Salida estructurada.** JSON, diffs o tablas filtrados línea a línea quedan inválidos o sin sentido.
7. **Rendimiento en el peor caso.** Un filtro con expresiones regulares sobre una salida de megabytes no puede colgar el
   composer (prioridad de suavidad).
8. **Falsa economía.** Si el modelo tiene que volver a pedir lo que se ocultó, se gasta más que sin filtrar.

**Reglas para no perder señal (obligatorias)**

- **R1. Nunca en silencio.** Toda salida filtrada lleva una cabecera: `[filtrado: N líneas omitidas de M — regla
  <nombre>; el original sigue en <ruta o job>]`. Es la regla 4 del decálogo («recortar avisa y orienta»).
- **R2. El original se conserva y se puede recuperar.** La salida íntegra (acotada por el tope de retención de jobs, no
  por el del modelo) queda accesible: en `job_output` para jobs, y para `bash` en un fichero temporal de sesión cuya
  ruta va en la cabecera. El modelo puede pedir «sin filtro» con un parámetro explícito (`raw: true`).
- **R3. Lista blanca de señal, no lista negra de ruido.** Una regla por comando **conserva** lo que sabe que importa
  (errores, avisos, fallos, resumen) y, ante una línea que no reconoce, **la conserva**. En la duda, no se quita.
- **R4. Con fallo, menos filtro.** Si el comando termina con código distinto de cero, o la salida contiene alguna marca
  de fallo conocida, se aplican solo las reglas generales (ANSI, progreso): el contexto alrededor de un error se
  conserva entero dentro de las cotas.
- **R5. Colapsar siempre cuenta.** «×N» y, si hay un patrón de tiempo, el rango; nunca se descarta la repetición sin
  decir cuántas había.
- **R6. Un filtro desconocido no se aplica.** Si el ejecutable no tiene regla, o el formato no coincide con lo que la
  regla espera (la primera línea esperada no aparece), la salida pasa **sin tocar** (salvo reglas generales) y sin error.
- **R7. Cada regla tiene prueba con salida real.** Se guardan capturas de salidas verdaderas (de varias versiones de la
  herramienta cuando se pueda), incluida una con el fallo **escondido en el medio** y una con salida desmesurada
  (decálogo, regla 10). La prueba central es de propiedad: **ninguna línea que contenga `error`, `FAILED`, `panic`,
  `warning:` o similar desaparece**.
- **R8. Tiempo acotado.** Cada filtro corre con un límite de tiempo y de memoria; si lo supera, la salida pasa sin filtrar.
- **R9. Trazabilidad.** El transcript y el modelo ven **lo mismo**: la salida ya filtrada con su cabecera. El
  contador de ahorro (bytes originales / enviados, por regla) se publica en el bus para medirlo; no se confía en la
  intuición.
- **R10. Se puede apagar.** `[output_filter] enabled = false` y `LOOMCODING_NO_FILTER=1` desactivan toda la capa; cada
  regla por comando se puede desactivar por nombre.

## Medición antes de fiarse

No se activa por defecto hasta tener números propios, no los de `rtk`:

- Ahorro medido sobre sesiones reales (bytes y tokens estimados por regla).
- Tasa de **re-petición**: veces que el modelo pide `raw: true` o repite el comando tras un filtro. Si una regla supera
  un umbral, se desactiva sola y se anota.
- Revisión periódica de las salidas filtradas contra las originales (muestreo) para detectar deriva de formato.

## Orden de trabajo propuesto

1. **Reglas generales** (ANSI, progreso, líneas repetidas) + cabecera + conservación del original + `raw`. Es lo más
   seguro y ya recorta mucho.
2. **`cargo test` y `cargo build/clippy`** (las salidas más ruidosas de este proyecto), con las pruebas de R7.
3. **`git` y gestores de paquetes.**
4. **Dedup de lecturas en el historial.**
5. **Medición** y decisión de activarlo por defecto.

## Reglas del proyecto

- Núcleo sin `ratatui`; módulo nuevo del core (`output_filter/`), ficheros ≤ 400 líneas, sin dependencias nuevas salvo que
  `regex` ya basta (ya es dependencia del core).
- Sin red real en las pruebas. Tiempos con reloj simulado.
- Es información propia de Loomcoding: **no hay estados A2A** implicados.

## Preguntas abiertas

1. ¿Dónde se guarda el original de `bash`: fichero temporal de sesión o reutilizar el almacén de jobs?
2. ¿Activado por defecto tras la medición, o siempre opt-in?
3. ¿Se aplica también a la salida de los subagentes que vuelve al padre, o solo a la de herramientas?
4. ¿El umbral de re-petición que desactiva una regla sola: cuál, y se avisa al usuario?

## Decisiones de la persona usuaria (2026-11-07)

Las cuatro preguntas quedan resueltas así:

1. **Dónde se guarda el original.** Se reutiliza el volcado que ya existe, `ToolSpill`
   (`crates/loomcoding-core/src/tools/spill.rs`: un fichero por llamada, 7 días de retención, 256 MB de tope por
   directorio). Hoy solo vuelca cuando la salida pasa de `MAX_TOOL_OUTPUT_BYTES`; el filtro lo usa también cuando **omite
   algo** aunque quepa. `bash` en primer plano no es un job, así que no se usa el almacén de jobs; los jobs conservan su
   `job_output`.
2. **Activado por defecto: no.** `[output_filter] enabled = false` hasta tener los números de «Medición antes de fiarse»;
   entonces se activa por defecto solo lo que haya demostrado ahorro sin re-peticiones.
3. **Salida de los subagentes: no.** Solo herramientas. La respuesta de una tarea A2A es su resultado y no se toca.
4. **Umbral de re-petición.** Una regla se desactiva sola si, con al menos 10 usos, el modelo pide `raw: true` o repite el
   comando en más del 20 % de ellos. Se avisa una sola vez con un `RuntimeNotice` y queda anotada en un contador.

Punto de enganche: `agent/turn.rs`, justo antes de `contenido_de_herramienta`, con el nombre y los argumentos de la llamada
a mano (necesarios para elegir regla y leer `raw`).
