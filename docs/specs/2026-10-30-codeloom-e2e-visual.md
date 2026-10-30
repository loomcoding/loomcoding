# E2E visual de la TUI (PTY) y captura web de otras herramientas

> Estado: **aprobada el 2026-10-30** (el usuario eligió B+C del análisis de opciones).
> Plan: `docs/plans/2026-10-30-codeloom-e2e-visual.md`.

## Problema

Varios documentos cierran con «falta probarlo en la TUI real»: composer multilínea,
pegado grande, imágenes adjuntas, atribución `You`, widgets de capacidades. Hoy solo
hay pruebas con `ratatui::TestBackend`, que validan `render()` pero no el bucle de
entrada ni el comportamiento de la terminal de Windows (ConPTY, VT input, pegado).
Quien revisa (persona o agente) no puede ver la pantalla.

## Objetivos

1. **B — E2E con PTY.** Un test lanza el binario real en un pseudo-terminal, le envía
   teclas y lee la pantalla ya interpretada por un emulador (texto, colores, cursor).
   Debe poder ejecutarse con `cargo test` y no tocar la sesión ni el daemon del usuario.
2. **Ver la pantalla.** Cada test puede volcar la pantalla a texto y JSON, y un script
   las convierte en PNG que un agente puede abrir como imagen.
3. **C — Captura de píxeles de cualquier herramienta de terminal.** Un script sirve un
   comando arbitrario por web (`ttyd`, xterm.js) solo en `127.0.0.1`, para abrirlo con
   Chrome y sacar capturas de la UI y de sus componentes (CodeLoom, el showcase de
   `batten` u otras herramientas que se incorporen).

## No objetivos

- No sustituye a las pruebas con `TestBackend`; las complementa.
- C no valida entrada de teclado ni pegado: xterm.js no es Windows Terminal. Es una
  revisión de aspecto.
- No se prueba contra proveedores reales: el proveedor es siempre `wiremock`
  (regla de `AGENTS.md`).
- No hay comparación de imágenes ni snapshots de píxeles en esta primera entrega.

## Diseño

### Crate de apoyo `codeloom-e2e`

Miembro nuevo del workspace, `publish = false`. No lo usa ningún crate de producción;
solo `dev-dependencies` de `codeloom-cli`. Contiene:

- `Term`: abre un PTY (`portable-pty`, ConPTY en Windows), lanza un comando, alimenta
  un `vt100::Parser` desde un hilo lector y expone `send_text`, `send_bytes`,
  `paste` (con *bracketed paste*), `resize`, `wait_for_text`, `screen_text`,
  `snapshot` y `kill`.
- El hilo lector **responde a la consulta de posición del cursor** (`ESC[6n`) con
  `ESC[fila;columnaR`: ConPTY la emite al arrancar y, sin respuesta, algunos hijos
  se quedan esperando.
- `Snapshot`: texto de la pantalla más una rejilla de celdas (`char`, `fg`, `bg`,
  `bold`, `inverse`, `underline`) y posición del cursor, serializable a JSON.
- `mock`: servidor `wiremock` que responde SSE tipo `/chat/completions`, y un
  `Workspace` temporal con `codeloom.toml` apuntando a ese servidor.

### Aislamiento

El test crea un directorio temporal que hace de perfil de usuario: pasa
`USERPROFILE` y `HOME` al hijo, de modo que `~/.codeloom` (sesiones, token, tuberías
con nombre derivadas del perfil, catálogo `models-dev.json`) no se comparte con el
daemon del usuario. La clave del provider se pasa por la variable de entorno que
declara `api_key_env`. Si `dirs::home_dir()` no respetase `USERPROFILE` en Windows,
la tarea 4 lo detecta (aserción sobre el árbol del perfil temporal) y se añade el
override en un único helper.

El catálogo de models.dev se descarga de la red si falta o está caducado; el test lo
evita sembrando un `models-dev.json` reciente en el perfil temporal.

### Binario y bloqueo en Windows

Con un `codeloom.exe` abierto, `cargo build/test` falla al reemplazarlo
(«Acceso denegado»). Los E2E se ejecutan con un `CARGO_TARGET_DIR` propio en `C:`
(el disco `E:` está casi lleno).

### Escenarios iniciales

1. **Arranque**: cabecera con `CodeL∞M`, composer visible y cursor en él.
2. **Turno**: escribir un prompt, `Enter`, el mock responde en SSE, la respuesta
   aparece en el transcript atribuida al asistente y el prompt como `You`.
3. **Pegado multilínea grande**: *bracketed paste* de un bloque largo; el composer
   muestra el marcador de pegado grande y no se rompe al seguir escribiendo.

### Script de PNG

`scripts/visual/render_screen.py` (Python 3 + Pillow): lee el JSON de `Snapshot` y
pinta una imagen con fuente monoespaciada del sistema. Sin dependencias nuevas
(Pillow ya está instalado).

### C — `ttyd` + Chrome

`scripts/visual/serve-tui.ps1 -Port N -Cwd DIR -- <comando> [args]` ejecuta
`ttyd -W -i 127.0.0.1 -p N <comando>`. La guía `docs/visual-e2e.md` describe cómo
abrirlo con las herramientas de Chrome, fijar el tamaño de ventana, enviar teclas y
capturar, y cómo cerrar el proceso.

## Preguntas abiertas

- Si ConPTY necesita además la respuesta a otras consultas (`ESC[c`, `ESC[?u`) se
  añadirán cuando un escenario cuelgue; hoy solo se conoce la de posición.
- Fuente del PNG: se usa la primera disponible entre Cascadia Mono, Consolas y
  Courier New; los glifos poco comunes (`∞`) pueden salir como recuadro.

## Verificación

- `cargo test -p codeloom-e2e` (unitarias del emulador y del lector).
- `cargo test -p codeloom-cli --test tui_pty` con `CARGO_TARGET_DIR` propio.
- Los cuatro checks de `AGENTS.md` al cerrar.
- Una captura real con C de la TUI y otra del showcase de `batten`.
