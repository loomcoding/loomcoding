# Loomcoding — Menú izquierdo: logo con efectos, secciones limpias y diff de la sesión

> Estado: **propuesta** (2026-11-06). Plan: `docs/plans/2026-11-06-loomcoding-menu-izquierdo.md`.
> Parte del dashboard limpio (`docs/specs/2026-11-05-loomcoding-dashboard-limpio.md`); aquí se trata
> solo el menú izquierdo del layout `Workspace` (23 columnas, a partir de 105 de ancho).

## 1. Problema

El menú izquierdo apila `AGENT`, `PROJECT`, `FLOCK` y `SESSION` en mayúsculas y sin respiro, con cuatro
contadores de `FLOCK` aunque estén a cero; la marca es discreta; y la lista «ficheros modificados» solo da
ruta y `+N −M`: no hay forma de ver **qué** ha cambiado el agente sin salir de la TUI.

## 2. Diseño

1. **Logo de cabecera** al estilo Crush: «LOOM» en bloques, una franja de `╱` en el acento y `loomcoding`
   tenue. Se reutiliza `loomcoding_cloth::widgets::logo` (`Wordmark`, `FONT_ROWS`, `compact`).
2. **Efectos del logo, de uno en uno y cada cierto tiempo.** Cada `logo_fx_interval` (por defecto 20 s) se
   elige **al azar** uno de un catálogo y se aplica unos segundos sobre el rectángulo del logo; entre
   ráfagas el logo está quieto y **no corre ningún reloj**. Catálogo (todo sale de librerías ya presentes):
   - *Lluvia binaria* de fondo: `MatrixRain` con `GlyphSet::binary()` en el rectángulo del logo.
   - *Reflejo*: una banda clara que barre el logo (`tachyonfx::fx::sweep_in` / `sweep_out` con `hsl_shift`).
   - *Linterna*: un haz de luz recorre el logo de lado a lado, como si alguien lo iluminara con una
     linterna: un foco elíptico con borde suave que aclara el color de las celdas que cubre (el resto
     queda atenuado mientras dura) y se desplaza con aceleración suave (`tachyonfx::fx::effect_fn` con
     `lighten_fg`/`darken_fg`, o un `Shader` propio si hace falta).
   - *Tipográfico*: las letras se desordenan y se recomponen (`coalesce`, `evolve`, `dissolve`).
   - *Glitch*: `tachyonfx::fx::Glitch` sobre las letras.
   - *Pulso*: cambio de tono ida y vuelta (`hsl_shift` + `ping_pong`).
   Nunca se repite el mismo efecto dos veces seguidas. Preferencia `logo_fx` (por defecto `true`) y
   `logo_fx_interval` en `ui.toml`; `/logofx` la alterna. Con `NO_COLOR` no hay efectos.
3. **Secciones limpias**: estado y modelo·esfuerzo juntos, ruta corta, una línea de actividad
   (`◌ 1 herramienta · ◐ 2 agentes`; lo que vale cero no se pinta), tokens abreviados y una fila en
   blanco entre bloques; títulos en negrita, sin mayúsculas. Trazabilidad A2A: si una línea agrupa
   estados, el original A2A va entre paréntesis.
4. **Diff de la sesión dentro del menú.** Bloque `Ficheros N +A −B` con una fila por fichero tocado en la
   sesión. Con el foco en el menú, `↑/↓` eligen, `Enter` despliega o pliega su diff unificado (hunks con
   `@@`, `-` y `+` en error/éxito, líneas recortadas con `…`). Solo uno desplegado a la vez, como mucho
   12 líneas por fichero y `… N líneas más`.
5. **Origen del diff**: `git diff --no-color -U2 -- <ficheros de la sesión>` en un hilo aparte con plazo de
   2 s y tope de 64 KiB de salida; sin git o fuera de un repositorio, el bloque lo dice y queda la lista.

## 3. Objetivos y no objetivos

**Objetivos:** menú legible de un vistazo; ver el diff sin salir de la TUI; cero coste de CPU en reposo.
**No objetivos:** tocar los paneles de la derecha del `Workspace` ni el rail del layout por defecto; editar
desde el diff; diff de ficheros no tocados en la sesión; descargar fuentes o logos.

## 4. Reglas

- `loomcoding-cloth` dibuja y no conoce el producto: el logo y el planificador de efectos reciben el
  reloj y la semilla, no el estado de la app. El azar es un generador mínimo propio con semilla inyectable
  (el repositorio no depende de `rand`) para que las pruebas sean deterministas.
- El reloj de 50 ms solo corre durante una ráfaga; el planificador publica el instante de la siguiente
  (`deadline`) para que el bucle despierte sin sondear.
- Decálogo de contexto y coste: salida de `git`, líneas por fichero, ficheros listados y duración de cada
  efecto, todos con cota.
- La fluidez del compositor es lo primero: nada de `git` ni de efectos en el hilo de teclado.
- Ficheros nuevos de 400 líneas como máximo; los cuatro chequeos en cada commit.

## 5. Criterios de aceptación

1. Con el layout `Workspace`, el menú izquierdo empieza por el logo y no hay secciones en mayúsculas ni
   contadores a cero.
2. Cada `logo_fx_interval` entra un efecto distinto del anterior; en reposo no hay reloj (prueba del
   bucle) y con `NO_COLOR` o `logo_fx = false` no hay ninguno.
3. Los ficheros tocados salen con su diff desplegable; el diff respeta las cotas y no bloquea la entrada.
4. Los cuatro chequeos del repositorio en verde en cada commit.

## Preguntas abiertas

- ¿Mostrar también el diff en el rail del layout por defecto? Se deja fuera de este plan.
- ¿Efectos también cuando el agente trabaja (en vez de solo por temporizador)? Se deja fuera.
