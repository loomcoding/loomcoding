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
3. **Trazas.** Cada uso emite una línea de `tracing` con destino `output_filter` (regla y bytes). Es la señal que
   sirve para medir sesiones reales, porque queda en un fichero. Se enciende con `RUST_LOG=output_filter=info` y se
   lee en `<perfil>/.loomcoding/logs/loomcoding-daemon.log`. Todo sobre ese registro (rutas, rotación a 5 MiB, que
   nunca escribe en la terminal, privacidad) está en [`docs/logging.md`](logging.md). La receta completa para
   medir está en «Cómo medir con una sesión real», más abajo.

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

## Cómo funciona por dentro

El camino de una salida de herramienta, de principio a fin:

```
herramienta (bash, job_output) ──► ToolOutput { content, is_error }
        │
        ▼   agent/turn.rs, un único enganche
output_filter::contenido_para_modelo(cfg, spill, call_id, (tool, input), out, recurso)
        │  1. ¿apagado, herramienta no filtrada, o `raw: true`?   ─► pasa tal cual (`recurso()`)
        │  2. stats.observa_llamada(...)   ← cuenta re-peticiones de la regla anterior
        │  3. filtrar(...):  entrada > 2 MiB?  ─► pasa   |  reglas generales
        │                    + regla por comando (la primera de `REGLAS` que reconoce el comando)
        │                    + ¿filtro > 50 ms?  ─► pasa
        │                    + guarda el ORIGINAL en ToolSpill; si no puede ─► pasa intacta (R2)
        │  4. antepone la cabecera R1 y acota a 20 000 bytes (`MAX_TOOL_OUTPUT_BYTES`)
        │  5. stats.registra_uso(regla, bytes)  → traza `output_filter` y, si toca, desactiva la regla
        ▼
Enviado { contenido, ahorro, desactivadas }
        │   turn.rs publica `LiveEvent::OutputFiltered` y, si una regla se apagó, un `RuntimeNotice`
        ▼
ToolResult que se guarda y se manda al modelo (lo mismo que ve el transcript)
```

Piezas, todas en `crates/loomcoding-core/src/output_filter/`:

| Fichero | Responsabilidad |
|---|---|
| `enviar.rs` | `contenido_para_modelo`: el único punto que llama el bucle. Une filtro, estadísticas y acotado. |
| `mod.rs` | `FilterConfig` (`[output_filter]`), `apply`/`filtrar`, la tabla `REGLAS` y `compactar_con_reglas`. |
| `general.rs` | Reglas generales (ANSI, progreso con `\r`, repetidas, blancos) y `es_senal`, el guardián de la señal. |
| `cargo.rs`, `git.rs`, `packages.rs` | Una regla por familia de comandos: `aplica(comando)` y `compactar(texto)`. |
| `comando_simple.rs` | Análisis del comando: solo uno simple tiene regla (sin tuberías, `&&`, `;`, `$`, redirecciones; se tolera `2>&1`). |
| `header.rs` | La cabecera R1. |
| `stats.rs` | Contadores por regla, re-peticiones, umbral y desactivación; la traza `output_filter`. |
| `fixtures/` | Capturas **reales y anonimizadas** de cada herramienta, usadas por las pruebas. |

Detalles que importan:

- **`job_output`** devuelve JSON: se compacta solo su campo `text` y el resto se conserva.
- **El original se guarda antes de anunciar su ruta.** Si no hay dónde guardarlo (volcado desactivado, disco lleno), la
  salida **pasa intacta**: perder información sin remedio sería peor que mandar ruido. Los originales viven en
  `<perfil>/.loomcoding/tool-output/`.
- **Estado de medición global por proceso.** Lo comparten el agente principal y los locales; en `cargo test` cada hilo
  tiene el suyo para que las pruebas no dependan del orden.
- **Nunca se filtra lo que no se reconoce.** Una línea que no está en la lista de ruido se queda. Es la regla de oro (R3).

## Cómo medir con una sesión real

La compresión se activa por defecto solo cuando haya cifras. Estas son las dos formas de obtenerlas.

**A. Uso diario (la representativa).** En el `loomcoding.toml` del proyecto donde trabajes de verdad, pon
`[output_filter] enabled = true`, y arranca siempre con `RUST_LOG=output_filter=info`. Pasada una semana, lee
`<perfil>/.loomcoding/logs/loomcoding-daemon.log` (y `.log.1` si rotó). Cada línea es un uso:

```
INFO output_filter: salida filtrada regla="cargo" original_bytes=2258 sent_bytes=653
```

Ahorro por regla = suma de `original_bytes` − suma de `sent_bytes`. Las re-peticiones no están en el registro: salen en
el aviso de desactivación (`RuntimeNotice`) y en `FilterStats::resumen()`. Si el modelo ha estado pidiendo `raw: true`,
lo verás en el transcript como llamadas `bash` con ese parámetro justo después de una cabecera `[filtrado: …]`.

**B. Prueba rápida de humo (sin tu proyecto).** Sirve para comprobar que toda la tubería funciona con un modelo real; no
para decidir nada. En PowerShell, con un modelo local en `127.0.0.1:18082` (cambia el modelo y la URL por los tuyos):

```powershell
$r = Join-Path $env:TEMP ('loom-medicion-' + [guid]::NewGuid().ToString('N'))
$p = "$r\proyecto"; New-Item -ItemType Directory "$p\src", "$r\home" | Out-Null
Set-Content "$p\Cargo.toml" "[package]`nname = `"juguete`"`nversion = `"0.1.0`"`nedition = `"2021`"`n"
$t = (1..60 | ForEach-Object { "    #[test] fn caso_$_() { assert_eq!($_ + 1, $($_ + 1)); }" }) -join "`n"
Set-Content "$p\src\lib.rs" "pub fn suma(a: i32, b: i32) -> i32 { a + b }`n#[cfg(test)]`nmod pruebas {`n$t`n}"
Set-Content "$p\loomcoding.toml" "model = `"<modelo>`"`nmax_steps = 6`n`n[provider]`nkind = `"openai_compat`"`nbase_url = `"http://127.0.0.1:18082/v1`"`n`n[output_filter]`nenabled = true`n"
$env:LOOMCODING_HOME = "$r\home"; $env:RUST_LOG = 'output_filter=info'; $env:CARGO_TARGET_DIR = "$r\target"
Set-Location $p
loom --yolo --print 'Usa la herramienta bash para ejecutar exactamente: cargo test   y dime cuantas pruebas pasaron.'
Get-Content "$r\home\.loomcoding\logs\loomcoding-daemon.log"
```

Notas de higiene de esta prueba:

- Usa **siempre** un `LOOMCODING_HOME` temporal: en Windows `dirs::home_dir()` ignora `USERPROFILE` y, sin él, el
  cliente se conectaría a tu daemon real.
- `--print` deja vivo el daemon del perfil temporal. Antes de borrar la carpeta, cierra ese `loom --server` (comprueba
  que lo arrancó tu prueba por su hora de inicio); si no, `sessions.db` queda bloqueado.
- `--yolo` da al agente todos los permisos: úsalo solo en un directorio de juguete aislado, nunca en un proyecto real.
- El `bash` de Loomcoding en Windows corre bajo `cmd.exe`, no `sh`: escribe los comandos de prueba en ese dialecto.

**Primera medición (2026-11-07, una sola muestra, proyecto de juguete con 60 tests, modelo local):**
`cargo` → 2258 bytes originales, 653 enviados (−71 %); el modelo contestó bien con la salida compactada. Una muestra
con un proyecto de juguete **no es representativa** y no decide nada: demuestra que la tubería funciona de extremo a
extremo (regla, cabecera, original, traza), nada más.

## Cómo decidir qué viene después

Con las cifras del uso diario (forma A):

| Pregunta | Criterio | Si se cumple |
|---|---|---|
| ¿Activar una regla por defecto? | Ahorro claro en bytes **y** re-peticiones por debajo del 20 % tras al menos 10 usos | Cambiar su valor por defecto a encendido; documentarlo en `CHANGELOG.md` y en la spec. |
| ¿Quitar o ajustar una regla? | Re-peticiones por encima del 20 % o el modelo se equivoca tras ver la salida filtrada | Corregir la regla (casi siempre se quita demasiado) y volver a medir. |
| ¿Hacer la dedup de lecturas (tarea 4)? | Las lecturas repetidas pesan una parte relevante del historial **y** el ahorro compensa perder el prefijo en la caché del proveedor | Escribir su spec propia: reescribir un mensaje antiguo invalida la caché de prefijo. |
| ¿Poner `git log`/`git diff` bajo regla propia? | Que el registro muestre que pesan en las sesiones | Spec y fixtures reales antes de nada; hoy no hay líneas de ruido que R3 permita omitir. |

## Añadir una regla nueva (lista de pasos)

1. **Spec primero.** Cada regla nueva respeta R1–R10 (`docs/specs/2026-11-06-loomcoding-compresion-de-salida.md`). Si
   dudas de que una línea sea ruido, es señal: se conserva.
2. **Capturas reales.** Guarda en `output_filter/fixtures/` salidas reales de la herramienta (no escritas a mano; si es
   imposible, di en la prueba que es una reproducción del formato). **Anonimízalas:** nada de nombre de usuario, rutas
   absolutas de tu máquina, correos ni nombres de personas (usa `C:\work\…`, `autor@example.invalid`,
   `Autor Ejemplo`) y comprueba con `grep`.
3. **Pruebas en rojo primero.** En `<familia>_tests.rs`: la regla se aplica; el formato desconocido pasa; el comando
   encadenado pasa; y una **prueba de la guarda de señal** (una línea de ruido que además lleva `error`, `failed`,
   `panic` o `warning:` debe conservarse). Comprueba a mano que quitar la guarda hace fallar esa prueba.
4. **El módulo de la regla.** `output_filter/<familia>.rs` con `aplica(comando: &str) -> bool` (usa
   `comando_simple::tokens`) y `compactar(texto: &str) -> Option<Compactado>`. Devuelve `None` si no hay nada que
   omitir o el formato no se reconoce. Usa `general::es_senal` para proteger líneas.
5. **Regístrala** en la tabla `REGLAS` de `output_filter/mod.rs` con su nombre (el que sale en la cabecera y en la
   traza). La primera regla que reconoce el comando decide.
6. **Pruebas sin reloj.** No dependas del tiempo: bajo `cfg(test)` el límite de 50 ms está relajado a 30 s precisamente
   para que la carga de la máquina no vuelva intermitentes las reglas. El límite por tamaño (2 MiB) sí se prueba.
7. **Documenta.** Esta guía (sección «Por comando»), `CHANGELOG.md`, la spec («Estado tras la implementación») y, si
   cambia un invariante, `docs/INVARIANTS.md`.
8. Cuatro chequeos: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build`,
   `cargo test --workspace`.

## Límites conocidos

- Las reglas dependen del texto que imprime cada herramienta. Un cambio de versión, de idioma o de opciones puede
  hacer que una regla deje de reconocer el formato; en ese caso la salida pasa sin tocar, pero conviene revisarlo.
- **Sin cifras representativas todavía.** Hay una sola muestra (arriba), de juguete. Hasta tener el uso diario de
  una semana el filtro sigue apagado por defecto.
- La dedup de lecturas en el historial (tarea 4 del plan) está aplazada hasta medir, y es la única pieza que
  reescribiría mensajes ya enviados.
- Las re-peticiones no se ven en el registro, solo en el aviso de desactivación y en `FilterStats::resumen()`; la
  interfaz aún no muestra los contadores.
- Los comandos con tuberías, `&&`, `;`, sustituciones o redirecciones (salvo `2>&1`) no tienen regla por comando: solo
  se les aplican las reglas generales.
- El filtro no consulta el código de salida del proceso, solo `is_error` de la herramienta.
