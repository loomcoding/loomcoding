# Fondo «telar» de carga — spec

**Estado:** propuesta (2026-11-08).

## Problema

Mientras el modelo trabaja, la TUI pinta por detrás una lluvia Matrix (`MatrixRain`, en
`loomcoding-cloth`): arranca con `LiveEvent::TurnStarted`, se detiene con `TurnEnded` y las gotas
restantes se drenan. Es el único fondo posible. La marca del proyecto, en cambio, es otra: la
**tejedora** (`docs/assets/loomcoding-self-weaving.html`), donde hilos de código caen sobre un telar y
tejen en la tela la palabra `LOOMCODING`. Esa escena no existe en la TUI.

## Intención

Un segundo fondo, **«loomcoding»**, que reproduce en celdas de terminal la escena de la tejedora, y
un selector para elegir entre **matrix**, **loomcoding** y **off**. Ocupa **toda la pantalla, por
detrás** de todo lo demás, como hoy la lluvia.

Los caracteres **aparecen al azar** cuando empieza el turno del modelo y **desaparecen al azar**
cuando termina.

## Decisiones de la persona usuaria (2026-11-08)

1. La escena ocupa toda la pantalla por detrás (no solo el hueco libre del transcript).
2. El fondo se elige en el menú de la TUI entre «loomcoding» y «matrix rain».
3. Aparición y desaparición de caracteres en orden aleatorio.

## Decisiones de diseño (mías; la persona usuaria las puede revocar)

1. **Tercera opción «off»** y **valor por defecto «matrix»**: quien no toque nada ve lo de siempre.
2. **Un fondo por turno.** Cambiar de fondo con un turno en curso detiene el anterior (se drena) y
   arranca el nuevo.
3. **Fila nueva `background` («Fondo») en el panel `/ui`**, que se cicla con `←/→` como las demás, y se
   guarda en `ui.toml` (`background = "matrix" | "loomcoding" | "off"`).
4. **Duraciones fijas**, no configurables: aparición de 0,9 s (18 ticks de 50 ms) y desaparición de
   1,2 s (24 ticks). Nada de ajustes que nadie pidió.
5. **Sin retícula tenue de hilos** (la del HTML pinta todos los glifos en verde oscuro): en una
   terminal sería ruido. Solo se pintan los tramos activos de cada hilo.
6. **Tres niveles según el tamaño** (celdas, ancho × alto): *completo* desde 56×20; *compacto*
   (hilos y barra, sin tela) desde 40×10; por debajo, el fondo no se pinta.
7. **Sin color (`NO_COLOR`) el fondo no se pinta.** Con 16 colores se pinta con los tonos del tema.

## Qué se reproduce de la escena

| Elemento del HTML | En la TUI |
|---|---|
| Hilos que bajan en abanico y convergen sobre el telar | Hasta 30 hilos de glifos katakana/dígitos que caen por una curva suave desde todo el ancho hasta una franja central; cabeza clara y cola que se apaga; ~10 % dorados y ~12 % azules |
| Barra del telar con postes y la lanzadera | Marco de caja de 3 filas con postes en los extremos, púas `│` por dentro y un punto luminoso que va y viene (ida y vuelta de 1,6 s) |
| Rótulo `CODE GENERATION` | Una fila bajo la barra |
| Tela en perspectiva | Filas de glifos `[]{}<>` y dígitos, más estrechas arriba y más anchas abajo (relación 0,87) |
| Palabra `LOOMCODING` tejida | Fuente de 5 filas (`L O M C D I N G`); la palabra baja **una fila por pasada de la lanzadera** (0,8 s) y se repite cada 12 filas |
| Rótulo `WEAVING SIGNALS FROM CODE` | Última fila de la pantalla |

**Se pierde** respecto al HTML: el halo de la lanzadera, el degradado fino de color y el temblor
sub-celda de las filas. Son efectos de píxel, no de celda.

## Aparición y desaparición al azar

Cada celda de la pantalla tiene un umbral aleatorio. Estado por celda: **visible o no**.

- **Empieza el turno:** un nivel `0 → 1` en 18 ticks hace visible cada celda cuando el nivel alcanza
  su umbral de aparición (semilla nueva en cada turno).
- **Termina el turno:** otro nivel `0 → 1` en 24 ticks oculta cada celda visible cuando el nivel
  alcanza su umbral de desaparición (otra semilla, por tanto **otro orden**).
- **Cambio de dirección a mitad:** continúa desde las celdas que estén visibles en ese instante. Nunca
  hay saltos ni parpadeo, porque cada dirección parte de su nivel 0 sobre el estado actual.
- Solo se recorre la rejilla mientras dura el fundido; en régimen permanente el coste es el de dibujar
  lo visible.
- Cuando termina la desaparición, el fondo libera la pantalla y el reloj de 50 ms puede pararse.

## Qué no cambia

- `MatrixRain` y su configuración (`[effects.matrix]`, panel Efectos) quedan intactos.
- El composer, el transcript y los paneles se dibujan **encima**: el fondo no toca celdas que no
  pinta (como hoy la lluvia).
- La TUI ignora este fondo en A2A, en el motor y en la sesión: es presentación pura.

## Restricciones (de `AGENTS.md`, `docs/INVARIANTS.md` y el decálogo)

- **El composer nunca parpadea, se cuelga ni se retrasa.** El fondo no usa el reloj en el render
  (como `logo_fx`): el avance lo da `advance()` en el tick de 50 ms.
- **Nada sin cota:** el fondo no se pinta si el área supera 40 000 celdas; el estado se reserva solo
  al cambiar de tamaño, nunca por fotograma; hilos ≤ 30.
- **`loomcoding-core` no depende de ratatui**; esto vive en `loomcoding-cloth` (widget) y
  `loomcoding-tui` (selección y enganche).
- Ficheros ≤ 400 líneas; sin red en pruebas; sin dependencias nuevas.

## Riesgos

- **Coste por fotograma** en pantallas grandes. Mitigación: se dibuja por elementos (hilos, barra,
  tela), nunca recorriendo toda la rejilla; el fundido sí la recorre, pero solo ~1 s.
- **Ilegibilidad:** el fondo compite con el texto. Mitigación: pintado por debajo, tonos oscuros y
  cola atenuada como la lluvia; el usuario puede poner `off`.
- **Anchos de glifo:** katakana de ancho 1 (`GlyphSet::katakana`); nunca emoji en esta escena.
- **Terminales sin truecolor:** los tonos se derivan del tema y degradan con él.

## Preguntas abiertas

1. ¿Conviene un atajo para alternar el fondo sin abrir `/ui`? No se hace ahora.
2. ¿La palabra `LOOMCODING` debe verse también en el nivel compacto? Se decide que no (no hay tela).
