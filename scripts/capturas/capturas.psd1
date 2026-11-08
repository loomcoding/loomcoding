# Tabla declarativa de las capturas de docs/user/img/, leída por generar.ps1.
#
# Cada entrada indica:
#   Archivo  nombre del SVG generado en docs/user/img/.
#   Volcado  volcado real de la TUI en docs/mockups/ (sale de una prueba del e2e).
#   Bloque   número del bloque ```text del volcado (contando desde 1).
#   Desde    primera línea del bloque incluida (desde 1). Opcional: sin ella, desde el principio.
#   Hasta    última línea del bloque incluida. Opcional: sin ella, hasta el final.
#   Prueba   prueba que produce el volcado (ORIGEN.md la cita para regenerarlo).
#   Titulo   título del SVG, en español (el volcado es de la TUI en español).
#   AltEn    texto alternativo de la página en inglés (docs/user/en/).
#   AltEs    texto alternativo de la página en español (docs/user/es/).
#
# Las páginas que embeben cada imagen están en docs/user/{en,es}/ con la misma ruta.
# Un volcado que falte en docs/mockups/ hace fallar el generador: no se inventa nada.

@{
    Imagenes = @(
        @{
            Archivo = 'pantalla-principal.svg'
            Volcado = 'docs/mockups/compresion-e2e.md'
            Bloque  = 1
            Prueba  = 'tui_pty_compresion'
            Titulo  = 'Pantalla principal de Loomcoding con el compositor'
            AltEn   = 'Loomcoding main screen in a terminal: a conversation with a bash call and its filtered output, the composer at the bottom with its hint, and the status bar with the keyboard shortcuts.'
            AltEs   = 'Pantalla principal de Loomcoding en una terminal: una conversación con una llamada a bash y su salida filtrada, el compositor abajo con su indicación y la barra de estado con los atajos.'
        },
        @{
            Archivo = 'cabecera-odd-flock.svg'
            Volcado = 'docs/mockups/vigilancia-jobs-e2e.md'
            Bloque  = 1
            Desde   = 1
            Hasta   = 10
            Prueba  = 'tui_pty_vigilancia'
            Titulo  = 'Cabecera con la etiqueta ODD y el flock'
            AltEn   = 'Top of the Loomcoding screen: the status line shows ready, the ODD flow label and the flock counters, 0 subagents running and 0 jobs, above the first messages of the session.'
            AltEs   = 'Parte superior de la pantalla de Loomcoding: la línea de estado muestra ready, la etiqueta del flujo ODD y los contadores del flock, 0 subagentes en marcha y 0 jobs, encima de los primeros mensajes de la sesión.'
        },
        @{
            Archivo = 'aviso-job.svg'
            Volcado = 'docs/mockups/vigilancia-jobs-e2e.md'
            Bloque  = 1
            Desde   = 6
            Hasta   = 12
            Prueba  = 'tui_pty_vigilancia'
            Titulo  = 'Aviso job: en el transcript'
            AltEn   = 'Transcript excerpt: a background job prints a line that matches the watch pattern, and the transcript shows it as a job notice with the line FAIL uno, followed by the system line that wakes the agent.'
            AltEs   = 'Fragmento del transcript: un job en segundo plano imprime una línea que casa con el patrón de vigilancia, y el transcript la muestra como aviso de job con la línea FAIL uno, seguido de la línea del sistema que despierta al agente.'
        },
        @{
            Archivo = 'filtrado-compresion.svg'
            Volcado = 'docs/mockups/compresion-e2e.md'
            Bloque  = 1
            Desde   = 4
            Hasta   = 11
            Prueba  = 'tui_pty_compresion'
            Titulo  = 'Cabecera de filtrado en la salida de bash'
            AltEn   = 'Transcript of a bash call with output compression on: the header that starts with filtrado, 39 lines omitted of 41, gives the rule and the path of the full original, and the repeated line appears as ruido ×40.'
            AltEs   = 'Transcript de una llamada a bash con la compresión de salida activa: la cabecera que empieza por filtrado, con 39 líneas omitidas de 41, indica la regla y la ruta del original completo, y la línea repetida aparece como ruido ×40.'
        },
        @{
            Archivo = 'ajustes-ui.svg'
            Volcado = 'docs/mockups/comandos-e2e.md'
            Bloque  = 1
            Prueba  = 'tui_pty_comandos'
            Titulo  = 'Panel de ajustes de la interfaz (/ui)'
            AltEn   = 'The interface panel opened with /ui over the main screen: each row shows its current value, for example the theme, the layout and the mouse switch.'
            AltEs   = 'Panel de interfaz abierto con /ui sobre la pantalla principal: cada fila muestra su valor actual, por ejemplo el tema, la distribución y el ratón.'
        }
    )
}
