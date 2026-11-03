# Avisos de proveedor, spinner de estado y comando `/verbose`

## Objetivo

Que los avisos internos del runtime (`sys: provider request started for …` /
`sys: provider response received for …`) no ensucien el transcript durante las
llamadas al proveedor, y que la actividad se comunique con una animación en la
barra de estado. El usuario decide en caliente, desde el composer, si quiere ver
esos avisos en el transcript.

## Contexto

Cada turno emite dos `LiveEvent::RuntimeNotice` que hoy van siempre al
transcript como `LineKind::System`. Un turno con varias llamadas al proveedor
(una por paso del bucle de herramientas) llena la vista de líneas `sys:`. Son
información de diagnóstico, no conversación.

`batten` ya ofrece animaciones en `widgets::spinner`:

- `Anim::glyph(frames, theme)` — un glifo de una celda con color oscilante.
  Sets: `BRAILLE`, `DOT_RING`, `ARC`, `BOUNCE`, `PULSE`, `BARS`.
- `Anim::render(text, theme)` — «scramble» determinista del texto con degradado
  de acento a `tool_running` a lo largo de la línea; conserva los espacios, así
  que el layout no salta.

Además, `batten` ya tenía `dotmax` como dev-dependency para el showcase. Su
catálogo `progress` incluye 14 estilos de **spinner** braille, y el propio crate
documenta que son seguros a 1×1 celdas. Medido en este repo con una sonda
temporal, a una fila de alto animan bien `dot-ring`, `growing-arc`,
`spinner-bars`, `arc-sweep` y `pulse`; `braille-spin` y `hourglass-flip`
necesitan dos celdas de ancho. Por eso `dotmax` pasa a ser una **feature
opcional** de `batten` (`dotmax`), no una dependencia obligatoria: el crate sigue
compilando sin ella y cae al glifo propio.

`App` ya mantiene `animation: batten::widgets::Anim` y el bucle de `run.rs` lo
avanza cuando `animating` es cierto, así que no hace falta un reloj propio.

## Comportamiento

### Configuración persistente

Sección opcional `[ui]` de `codeloom.toml`:

```toml
[ui]
verbose_provider_notices = false
```

- Ausente o `false` (por defecto): los dos avisos de proveedor **no** van al
  transcript; la barra de estado muestra la animación mientras hay una petición
  en vuelo.
- `true`: los avisos van al transcript como hasta ahora y no hay animación de
  proveedor.

`UiConfig` vive aparte de `CodeLoomConfig` (que se construye en decenas de
sitios) y sigue el patrón de `AgentsConfig` / `ProviderCacheConfig`: `Default`,
`from_toml(&str)` y `load(&Path)`.

### Comando `/verbose`

- `/verbose` sin argumentos **alterna** `verbose_provider_notices` en la sesión
  en curso y deja constancia en `slash_feedback`
  (`Provider notices: on` / `Provider notices: off`).
- `/verbose on` y `/verbose off` fijan el valor de forma explícita; cualquier
  otro argumento produce `Usage: /verbose [on|off]` en `slash_feedback`.
- El cambio es **de sesión en memoria**: no reescribe `codeloom.toml`. Para que
  persista, el usuario edita el fichero.
- El comando aparece en el autocompletado con la descripción
  «Toggle provider request notices in the transcript».

### Detección de la actividad

`App` lleva un contador `provider_in_flight: usize`:

- `+1` al recibir un `RuntimeNotice` cuyo mensaje contiene
  `provider request started`.
- `-1` (saturado a cero) al recibir `provider response received`.

El resto de `RuntimeNotice` (subagentes, compactación, errores) se muestra
**siempre** en el transcript, con independencia de la preferencia.

### Animación

En los dos chasis, cuando `provider_in_flight > 0` y
`verbose_provider_notices == false`:

```
⣾ Provider working… deepseek/deepseek-v4.1-flash-1017   <- barra de estado clásica
⣾ > escribe aquí…                                      <- prompt del composer (crush)
```

- El chasis clásico lo muestra en la barra de estado; el chasis Crush lo pone en
  el prompt del composer, que es donde mira el usuario mientras escribe.
- Indicador de actividad **en el prompt del composer** (estilo Crush): mientras hay
  una petición en vuelo, el `> ` del composer se sustituye por un spinner braille
  de `dotmax` en el mismo ancho de dos celdas, así que el layout no se mueve y el
  texto que el usuario está escribiendo se queda donde estaba.
- Cada petición **elige un estilo distinto** de entre los que animan a una fila
  (`dot-ring`, `growing-arc`, `spinner-bars`, `arc-sweep`, `pulse`), mediante
  `pick_activity_style(seed)` con la semilla tomada del reloj al arrancar la
  petición. No se añade generador aleatorio: la semilla entra por parámetro y los
  tests la fijan.
- Si la feature `dotmax` está apagada o el estilo no existe, se cae al `> ` de
  siempre: el composer nunca queda sin prompt.
- Se usa `app.animation.frame`, el contador del `Anim` que avanza el bucle, que el
  render copia al composer con `set_activity_frame`: no se crea un reloj nuevo ni
  se consulta `Instant::now()` en el render (que daría siempre cero y dejaría el
  indicador congelado).
- Se usa `app.animation`, el mismo `Anim` que avanza el bucle: no se crea un
  reloj nuevo ni se consulta `Instant::now()` en el render (que daría siempre
  cero y dejaría el glifo congelado).
- `run.rs` considera la app «animando» también cuando
  `provider_in_flight > 0`, para que se repinte mientras dura la petición y el
  glifo se mueva.
- Con `verbose_provider_notices == true` la barra de estado no cambia.

## Límites

- Nada sin cota: el texto de la animación se recorta al ancho disponible; el
  modelo se pasa por el mismo sanitizado que el resto de la barra.
- Sin red en tests: la lógica se prueba aplicando `LiveEvent::RuntimeNotice` a
  un `App` en memoria.
- Sin dependencias nuevas obligatorias: `dotmax` entra como **feature opcional**
  de `batten` (`dotmax`), ya presente en el árbol como dev-dependency del
  showcase. Con la feature apagada todo sigue compilando y el indicador cae al
  glifo propio.

## Preguntas abiertas

- ¿Debe `/verbose` escribir la preferencia en `codeloom.toml`? Hoy se decide
  que no (el fichero es del usuario); si se quiere persistir desde la TUI hará
  falta un escritor de config que hoy no existe.
- ¿Merece la pena un `LineKind::Debug` para poder filtrar diagnóstico por nivel
  en vez de por texto? Queda para cuando haya más de un aviso ruidoso.

## Referencias

- `crates/codeloom-core/src/config/mod.rs` — `UiConfig`.
- `crates/codeloom-tui/src/app.rs` — `provider_in_flight`, `apply`.
- `crates/codeloom-tui/src/render.rs` — `draw_flat_footer`, barra clásica.
- `crates/codeloom-tui/src/commands.rs` — `Command::Verbose`.
- `crates/batten/src/widgets/spinner.rs` — `Anim::glyph`, `DOT_RING`.
