# Filtro de salida de herramientas (`output_filter`)

> **Estado:** implementado y **apagado por defecto**. Spec: `docs/specs/2026-11-06-loomcoding-compresion-de-salida.md`
> (sección «Estado tras la implementación»). Plan: `docs/plans/2026-11-07-loomcoding-compresion-de-salida.md`.

## Qué hace

Antes de que el resultado de una herramienta llegue al modelo, el filtro quita el ruido que no aporta (secuencias de
color, barras de progreso, líneas repetidas, líneas en blanco) y, para algunos comandos, conserva solo la señal
(errores, avisos, fallos y resumen final). Lo que se quita queda anunciado en una cabecera y el original íntegro se guarda
en disco.

Actúa solo sobre `bash` y `job_output`. Nunca toca `read`, `grep` ni `glob`, ni la respuesta final de un subagente o de
una tarea A2A.

Ejemplo de salida filtrada:

```
[filtrado: 2 líneas omitidas de 5 — regla general; el original sigue en <ruta del volcado>]
inicio

fin
```

## Encenderlo y apagarlo

El filtro viene **apagado**. Para encenderlo, en el `loomcoding.toml` del proyecto:

```toml
[output_filter]
enabled = true
```

- Se lee al arrancar: hay que reiniciar Loomcoding tras cambiarlo.
- `LOOMCODING_NO_FILTER=1` lo apaga siempre, aunque la sección lo encienda.
- Una regla desactivada sola (véase más abajo) queda apagada hasta reiniciar el proceso.

## Cómo medir

Hay tres señales, de menos a más directa:

1. **Evento del bus.** Cada vez que el filtro actúa se publica `LiveEvent::OutputFiltered { rule, original_bytes,
   sent_bytes }`: regla aplicada, bytes que produjo la herramienta y bytes que recibe el modelo. La interfaz lo ignora
   por ahora.
2. **Aviso.** Cuando una regla se desactiva sola se publica un `RuntimeNotice`, una sola vez por regla y proceso.
3. **Trazas.** Cada uso emite una línea de `tracing` con destino `output_filter` (regla y bytes). Para verla, define
   `RUST_LOG=output_filter=info` al arrancar: el cliente y el daemon escriben en
   `<perfil>/.loomcoding/logs/loomcoding-cliente.log` y `loomcoding-daemon.log` (el de la sesión que filtra es el
   daemon). Sin `RUST_LOG` no se instala nada ni se crea fichero. El fichero rota a `.1` al superar 5 MiB. Nunca se
   escribe en stdout ni en stderr. Cómo medir de verdad, paso a paso:

   ```
   RUST_LOG=output_filter=info loomcoding
   ```

   Después busca `output_filter` en el registro del daemon y compara `original_bytes` con `sent_bytes`.

Los contadores por regla (usos, re-peticiones, bytes originales y enviados) se obtienen con
`FilterStats::resumen()` (`crates/loomcoding-core/src/output_filter/stats.rs`). Todavía no se muestran en la interfaz.

**Qué se guarda de la medición:** el nombre de la regla, cifras y una huella numérica de 64 bits del comando, solo en
memoria y solo para comparar dentro del proceso. Nunca el texto del comando, ni rutas, ni salidas.

## Re-peticiones y desactivación automática

Hay **re-petición** cuando, tras una salida filtrada de una regla, el modelo:

- pide la misma herramienta con `raw: true`, o
- repite exactamente el mismo comando.

Una regla con **al menos 10 usos** y **más del 20 %** de re-peticiones se desactiva sola para el resto del proceso y
se avisa una vez. Con menos de 10 usos nunca se desactiva.

## Recuperar el original

- La cabecera de cada salida filtrada lleva la ruta del volcado con el original íntegro. Los volcados se conservan
  7 días, como el resto de volcados de Loomcoding.
- El modelo puede pedir la salida sin filtrar con `raw: true` en `bash` y en `job_output`.

## Reglas

### Generales (cualquier salida de texto)

- Se quitan las secuencias de color y de control (ANSI).
- Una barra de progreso que se reescribe con retornos de carro deja solo su último estado.
- Las líneas idénticas consecutivas quedan en una, con «×N» (cuántas había).
- Las líneas en blanco consecutivas quedan en una.

### Por comando

Se aplica según el primer ejecutable del comando. Si ninguna regla encaja, o el formato no es el esperado, pasa solo
lo general.

- **`cargo test`, `cargo build`, `cargo check`, `cargo clippy`.** Se quitan las líneas de compilación en curso,
  descargas, los `Finished` anteriores al último y los `test … ... ok`. Se conservan errores, avisos, `FAILED`,
  `panicked`, el resumen y la línea de contexto de cada fallo. No se aplica con salida en otro idioma, con
  `--message-format json`, ni cuando el comando encadena o redirige (`|`, `&&`, `;`).
- **`git status`** (formato largo). Se quitan las pistas `(use "git …")` que se repiten bajo cada sección. Lo demás
  se conserva. `git diff` y `git log` no tienen regla propia: solo se aplican las reglas generales.
- **`npm install` y `pnpm install`.** Se quitan las barras de progreso de pnpm y las líneas `Progress:` salvo la última,
  y los avisos de deprecación repetidos (se cuentan con «×N»).

Con fallo (`is_error`) se aplica la misma lista blanca, y ninguna línea con señal se descarta.

## Principios (R1–R10) en lenguaje llano

| Regla | Qué significa en la práctica |
|---|---|
| R1 Nunca en silencio | Toda salida recortada dice cuántas líneas se han quitado y dónde está el original. |
| R2 El original se guarda | El texto completo queda en un volcado, y `raw: true` lo devuelve sin filtrar. |
| R3 Lista blanca de señal | Solo se quita lo que la regla reconoce como ruido. Lo que no reconoce se queda. |
| R4 Con fallo, menos filtro | Pensado para quitar menos cuando hay un error. En la práctica se usa la misma lista blanca, que ya protege las líneas con señal (véase la spec, «Desviaciones»). |
| R5 Colapsar siempre cuenta | Una línea repetida se anota con «×N»; nunca desaparece sin decir cuántas había. |
| R6 Lo desconocido pasa | Si no hay regla o el formato no coincide (otro idioma, JSON), la salida pasa tal cual. |
| R7 Pruebas con salidas reales | Las reglas se prueban con capturas reales, y una prueba de propiedad comprueba que ninguna línea con `error`, `FAILED`, `panic` o `warning:` desaparece. |
| R8 Tiempo y tamaño acotados | Una entrada de más de 2 MiB, o un filtro que tarda más de 50 ms, pasa sin filtrar. |
| R9 Lo mismo para todos | El transcript y el modelo ven exactamente lo mismo, con su cabecera. El ahorro se publica en el bus. |
| R10 Se puede apagar | `enabled = false` en la configuración o `LOOMCODING_NO_FILTER=1`. |

## Límites conocidos

- Las reglas dependen del texto que imprime cada herramienta. Un cambio de versión, de idioma o de opciones puede
  hacer que una regla deje de reconocer el formato; en ese caso la salida pasa sin tocar, pero conviene revisarlo.
- El ahorro real aún no está medido sobre sesiones de uso reales. Hasta tener esas cifras el filtro sigue apagado por
  defecto.
- La dedup de lecturas en el historial (tarea 4 del plan) está aplazada hasta medir.
