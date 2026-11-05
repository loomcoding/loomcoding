# Loomcoding — Dashboard limpio y muñeco de estado

> Estado: **propuesta** (2026-11-05). Plan: `docs/plans/2026-11-05-loomcoding-dashboard-limpio.md`.
> Boceto aprobado: demos «Dashboard limpio» del showcase (`crates/loomcoding-cloth/examples/showcase/demos/dashboard_clean.rs`)
> y `docs/mockups/dashboard-limpio.md`.

## 1. Problema

La pantalla de Loomcoding se percibe saturada frente a Crush, Claude Code, Codex, Pi o Mastra Code: cabecera de
dos líneas con datos que el rail repite (modelo, ruta, MCP/LSP/skills), un rail con seis secciones en mayúsculas y
borde, un pie con ocho atajos. Falta, además, algo propio: el diferenciador del producto es que los agentes sean
**observables**, y hoy lo único que anima el estado es un spinner de una celda.

## 2. Diseño (aprobado en el boceto)

1. **Tres disposiciones seleccionables**, como valores nuevos del `Look` del tema (`/theme` y `ui.toml` ya existen):
   - **B · `ContextRail` adelgazado** — por defecto. Transcript sin cajas; panel a la derecha, **sin borde**, con tres
     bloques: *Agentes* (glifo de estado, nombre, tiempo), *Tokens* (entrada, salida, caché) y una línea tenue de
     MCP/LSP. **Un bloque vacío no se pinta.** Títulos en negrita, no en mayúsculas.
   - **A · `LayoutStyle::Minimal`** — sin panel; el estado del flujo va en la cabecera.
   - **C · `LayoutStyle::Cards`** — A más una tarjeta plegable de *Agentes* sobre el compositor, con columnas
     alineadas (modelo · esfuerzo, tokens, tiempo) y tope de filas (`… N more`).
2. **Cabecera de una línea** en las tres: `loom · modelo · esfuerzo` a la izquierda; `ctx 12k/64k ██░░` y, en A y C,
   la etiqueta del flujo activo (`gauntlet #3 · ronda 2/3`) a la derecha. Ruta y estado de MCP/LSP/skills salen de la cabecera.
3. **Un solo acento** (el del tema) y tonos apagados para lo secundario; glifos de estado `✓ ✗ ◐ ○ ◌` en lugar de cajas.
4. **Pie de cuatro atajos** (`enter envía · ctrl+o pliega · ctrl+b panel · ? atajos`); `?` abre la lista completa.
5. **Muñeco de estado** (braille, 6×2 celdas, función pura de `(postura, tick)`): franja de tres filas sobre el
   compositor con el muñeco a la izquierda y «qué hace» a la derecha. Posturas: pensando, ejecutando (corre),
   buscando (corre con lupa), esperando respuesta humana, terminado, fallido. Preferencia `mascot = true|false` en
   `ui.toml`; apagado, queda una línea de texto. Con `NO_COLOR` o sin Unicode braille cae al glifo de siempre.
6. **Ruido de proveedor**: ya está oculto por defecto (`verbose_provider_notices = false`); no se cambia. Los
   volcados del e2e antiguos lo mostraban porque lo activaban.

## 3. Objetivos y no objetivos

**Objetivos:** menos elementos por pantalla; el estado de cada agente legible de un vistazo; el aspecto actual
sigue disponible como preset; coste de CPU nulo (se redibuja con el reloj de 50 ms que ya existe).

**No objetivos:** cambiar el contenido del transcript, los datos o la lógica del agente; nuevos temas de color;
sprites a color o fotogramas descargables; la lista de tareas con dueño (otra propuesta).

## 4. Reglas

- `loomcoding-cloth` dibuja y no conoce el producto: el muñeco (`widgets/mascot.rs`) recibe una postura, no un
  agente. La traducción estado de agente → postura vive en `loomcoding-tui` y es una función pura testeable.
- Cada fichero nuevo, 400 líneas como máximo (regla de tamaño vigente).
- Los tests que afirman texto de pantalla se ajustan lo mínimo; ninguna prueba se borra sin reemplazo.
- Trazabilidad A2A: si una fila agrupa estados, el original A2A va entre paréntesis (`working`, `input-required`…).

## 5. Criterios de aceptación

1. Con el preset por defecto no hay borde en el rail ni títulos en mayúsculas, y un bloque sin datos no ocupa filas.
2. `Minimal` y `Cards` se eligen con `/theme` o `ui.toml` y se ven en el showcase y en un volcado e2e.
3. `mascot` apagado o `NO_COLOR` no pinta braille; encendido, la postura sigue al estado (pruebas de `pose_of`).
4. El pie muestra cuatro atajos y `?` los lista todos.
5. Los cuatro chequeos del repositorio en verde en cada commit.

## Preguntas abiertas

- ¿Personaje alternativo (tejedor/araña con hilo) como pack seleccionable? Se deja fuera de este plan.
- ¿Muñeco por fila de la tarjeta (uno por subagente) o solo el del agente activo? Se propone solo el activo.
