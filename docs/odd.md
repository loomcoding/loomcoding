# Desarrollo orgánico (ODD)

ODD es el modo ligero de Loomcoding para trabajar en una feature: viene **activo por defecto** y solo se nota cuando
el trabajo es grande. Es un flujo propio de Loomcoding; no es un estado ni una tarea A2A, y no cambia el ciclo de vida
de ninguna tarea A2A (subagentes locales ni tareas servidas).

Spec: `docs/specs/2026-11-07-loomcoding-odd-desarrollo-organico.md`. Plan: `docs/plans/2026-11-07-loomcoding-odd-desarrollo-organico.md`.

## Qué hace

1. **Prueba del resumen.** Si el trabajo cabe en un resumen de pocas líneas que se pueda retomar desde la petición y
   `git diff`, el modelo lo hace sin más y no se crea nada.
2. **Documento de feature, solo para lo grande.** El modelo lo crea con la herramienta `odd_track` en
   `.loomcoding/odd/<feature>.md` del proyecto: objetivo y tareas como casillas. El nombre de la feature admite
   `a-z`, `0-9`, `_` y `-`.
3. **Casillas con evidencia.** Marcar una tarea como hecha exige `evidence {text, call_id}`, donde `call_id` es una
   llamada real de `bash`, `job_start` o `job_output` de esta sesión (de los últimos 500 mensajes). El runtime comprueba
   que la llamada existe y es de esa clase; **no** comprueba que el texto sea cierto. Si no se puede avanzar, la tarea
   se bloquea con un motivo.
4. **Cierre bloqueante.** Si el modelo intenta cerrar el turno con tareas sin hacer, el runtime lo devuelve al modelo
   con la lista de pendientes. Es duro pero acotado: **3 reinyecciones por turno**, ninguna si el conjunto de pendientes
   no cambió («sin progreso»), plazo de 2 s para la consulta, y la cancelación de la persona usuaria gana siempre.
   Cuando todo lo no hecho está bloqueado, el turno cierra con «Necesita tu decisión». Un documento ausente o ilegible
   nunca bloquea.
5. **Fase visible.** La cabecera muestra `ODD · exploring`, `ODD · implementing (2/5)`… La fase se infiere de las
   herramientas usadas (leer = exploring, escribir = implementing, un `cargo test`/`build`/`clippy`/`fmt` conocido =
   checking, preguntar = deciding, `odd_track` = planning). Es una **etiqueta propia de Loomcoding, no un estado A2A**.
   Mientras corre un `gauntlet` o un `development_workflow`, la cabecera muestra el suyo.
6. **Lista de tareas.** La lista de tareas de la TUI es una proyección del documento. Las bloqueadas salen pendientes
   con ` [bloqueada: motivo]`. Si el modelo usa además `todo_write`, gana el último en escribir en pantalla; la verdad
   es el documento.

## Ajustes

```toml
# loomcoding.toml del proyecto
[flows.odd]
enabled = true   # por defecto; false lo apaga: sin fase, sin bloqueo del cierre, sin sección de prompt
```

`/flow` lista los modos de flujo y su estado. `/flow odd on` y `/flow odd off` guardan el valor en
`[flows.odd] enabled`; **aplica al reiniciar Loom** (la TUI es un cliente separado del daemon). La TUI no conoce
ODD: recibe la lista de modos de `loomcoding-workflows` a través del CLI, y un modo «permanente» vuelve a su
etiqueta base al acabar el turno. Los subagentes no ven `odd_track` ni heredan el cierre bloqueante.

## Medición

Los contadores viven en memoria del daemon y no llevan rutas ni texto: documentos creados, tareas marcadas y
bloqueadas, cierres bloqueados, cierres sin progreso, cierres por decisión, cierres por documento ilegible y rechazos
de evidencia. Salen por `tracing` con el destino `odd` (por ejemplo, `RUST_LOG=odd=info`). Con esa variable definida al
arrancar, las líneas quedan en `<perfil>/.loomcoding/logs/loomcoding-daemon.log` (registro acotado a 5 MiB; sin
`RUST_LOG` no se escribe nada). Como ODD viene activo, la medición del final de la spec es un seguimiento: si los
números salen mal, se desactiva.

## Límites conocidos

- La evidencia comprueba que la llamada existe, no que el texto sea cierto.
- Tras reiniciar, si quedan tareas pendientes de una feature anterior, el primer turno puede recibir una reinyección
  aunque la petición no tenga relación; después no insiste.
- La fase vive en memoria: tras reiniciar vuelve a `idle` hasta la primera herramienta.
- `/flow odd on|off` no actúa en caliente.
