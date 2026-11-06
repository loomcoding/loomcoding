# E2E visual de la TUI

Dos formas de ver y probar la TUI sin una persona delante. Diseño en
`docs/specs/2026-10-30-codeloom-e2e-visual.md`.

| | B — PTY en `cargo test` | C — `ttyd` + Chrome |
|---|---|---|
| Qué es | el binario real en un pseudo-terminal (ConPTY) leído con un emulador | la TUI servida por web (xterm.js) |
| Valida | entrada de teclas, pegado, ciclo de vida, texto y estilos de la pantalla | solo el aspecto (no es Windows Terminal) |
| Cuándo | en cada cambio de comportamiento; entra en `cargo test` | revisión visual puntual y de otras herramientas |

## Regla de oro: nunca arrancar `loomcoding` sin `LOOMCODING_HOME`

El cliente se conecta al daemon local de la persona usuaria si comparte perfil. En
Windows `dirs::home_dir()` ignora `USERPROFILE`, así que **solo** `LOOMCODING_HOME` aísla
el perfil (sesiones, token, tubería del daemon). Un E2E sin él escribe sesiones en
`~/.loomcoding/sessions.db` real (ocurrió el 2026-10-30; véase el plan).

## B — E2E con PTY

```bash
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.35.32215\bin\Hostx64\x64\link.exe'
export CARGO_TARGET_DIR='E:\workspace\personal\loom\target\e2e' CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p loomcoding-cli --test tui_pty
```

- Un `loomcoding.exe` abierto en `target/debug` bloquea el enlazado: por eso el
  `CARGO_TARGET_DIR` propio. `DEBUG=0` y sin incremental ahorran gigas en disco.
- Cada test crea un perfil y un proyecto temporales (`Workspace`), un proveedor
  `wiremock` (`mock_provider`) y mata sus daemons al terminar (`kill_daemons_of`).
- Los volcados de pantalla quedan en `target/e2e/tmp/e2e-shots/<nombre>.{txt,json}`.
  Para verlos como imagen:

  ```bash
  python scripts/visual/render_screen.py target/e2e/tmp/e2e-shots/turn.json turn.png
  ```

- Escenarios de subagentes: `subagent_without_final_text_fails_instead_of_completing_empty`
  (hijo sin texto final) y `a_child_question_is_answered_from_the_composer_with_ctrl_g` (el hijo
  pregunta con `ask_user`, `Ctrl+G` apunta el composer a él y la respuesta llega sin turno del
  padre). Sus guiones del proveedor están en `crates/loomcoding-e2e/src/mock.rs`.
- Comandos de ajustes: `crates/loomcoding-cli/tests/tui_pty_comandos.rs` cubre `/ui` (y que los alias ocultos no salgan
  en el popup), el selector de `/effort`, `/model <id>` y `/provider` contra servidores `wiremock` que simulan llama-server
  (`up`), una clave rechazada (`sin acceso`) y un puerto cerrado (`down`). Volcados en `docs/mockups/comandos-e2e.md`.
- Carril del transcript: `crates/loomcoding-cli/tests/tui_pty_carril.rs` arranca la TUI con cada valor de `rail` en la
  disposición `minimal` (sin paneles laterales, cuyos marcos falsearían las aserciones de glifos) y comprueba que `box` dibuja
  `╭╮╰╯` y que el resto no. Volcados en `docs/mockups/carril-e2e.md`.
- Para añadir un escenario: `Session::start(respuesta, filas, columnas)`, `s.expect(READY, BOOT)`,
  `s.term.send_text / press / paste`, y `s.expect(texto, TURN)` o `s.expect_line(...)`.

## C — `ttyd` + Chrome

Sirve cualquier comando de terminal solo en `127.0.0.1` (instalar: `winget install tsl0922.ttyd`).

```powershell
# Perfil aislado: define estas variables ANTES de lanzar el script (ttyd y su hijo las heredan).
$env:LOOMCODING_HOME = $env:USERPROFILE = $env:HOME = "$env:TEMP\loomcoding-live-home"
$env:LOOMCODING_E2E_KEY = "sk-test"
pwsh -NoProfile -File scripts/visual/serve-tui.ps1 -Port 7691 -Cwd <proyecto-temporal> `
  -Run 'E:\workspace\personal\loom\target\e2e\debug\loomcoding.exe'
# → http://127.0.0.1:7691
pwsh -NoProfile -File scripts/visual/serve-tui.ps1 -Port 7691 -Stop
```

El proyecto temporal necesita un `loomcoding.toml` (véase `Workspace::new` en
`crates/loomcoding-e2e/src/mock.rs`) y el perfil un `.loomcoding/models-dev.json` con `{}`
para que no intente descargar el catálogo.

Con las herramientas de Chrome: `tabs_context_mcp` → `tabs_create_mcp` → `navigate` a la URL
→ `resize_window` para fijar el ancho (las columnas salen del tamaño de la ventana)
→ `computer` (`screenshot`, `type`, `key`). Al acabar, `-Stop` y cierra la pestaña.

Limpieza del daemon aislado, sin tocar el del usuario (se filtra por la ruta del ejecutable):

```powershell
$exe = 'E:\workspace\personal\loom\target\e2e\debug\loomcoding.exe'
Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $exe } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
```

Para otras herramientas (el showcase de `cloth`, `herdr`, etc.) basta con cambiar `-Run`.

### Limitaciones conocidas

- xterm.js no es Windows Terminal: el pegado y algunas teclas pueden diferir. Eso se valida con B.
- El 2026-10-30 `Page.captureScreenshot` agotó el tiempo (30 s) con la extensión de Chrome, incluso
  con una página estática. Ciertamente, hay una vía sin extensión que sí funciona (véase abajo).

### Captura sin extensión (Chrome sin cabeza por CDP)

```bash
node scripts/visual/capture-web.mjs http://127.0.0.1:7681 salida.png 4000 1000 500
```

Cabe señalar que `chrome --headless --screenshot` fotografía antes de que `ttyd` pinte (sale el
fondo vacío); el script espera `espera_ms` tras navegar y captura por CDP. Probado con `cmd /k`
servido por `ttyd`: el texto de xterm.js sale pintado. La variable `CHROME_PATH` permite indicar otra ruta de Chrome.

### ¿Y un backend wasm de ratatui?

Existen backends de ratatui para el navegador (por ejemplo `ratzilla` y `webatui`, que compilan
a wasm y pintan en un lienzo o en el DOM). Cabe señalar que obligan a recompilar la aplicación
contra ese backend, de modo que no ejercitan el binario real, ni ConPTY, ni el bucle de entrada
de la terminal, ni a `herdr`. Su utilidad queda reservada a widgets aislados de `cloth`; para
cualquier herramienta de terminal tal cual es se mantiene `ttyd` con xterm.js, que sirve el
ejecutable sin modificar. Esta valoración procede de conocimiento general y no se ha probado aquí.

### Qué hace falta para ampliar estas pruebas

- Una captura de píxeles estable con Chrome (véase la limitación anterior); mientras tanto, B
  más el PNG de `render_screen.py` cubren la revisión visual.
- Nuevos escenarios en `crates/loomcoding-cli/tests/tui_pty.rs` para composer multilínea, imágenes
  adjuntas y respuesta final vacía de subagente.
