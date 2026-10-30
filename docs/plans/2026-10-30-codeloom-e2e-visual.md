# E2E visual de la TUI (PTY) y captura web — Plan de implementación

> Estado: **pendiente**. Aprobado el 2026-10-30; sin tareas iniciadas.

> **Para agentes:** sub-skill requerida: `superpowers:subagent-driven-development` (recomendada) o `superpowers:executing-plans`. Los pasos usan casillas `- [ ]`. Una tarea cada vez; commit al verificarla.

**Meta:** poder lanzar el binario real de CodeLoom en un PTY desde `cargo test`, enviarle teclas, leer la pantalla interpretada y convertirla en PNG; y servir cualquier herramienta de terminal por web para capturarla con Chrome.

**Arquitectura:** crate de apoyo `codeloom-e2e` (PTY con `portable-pty`, emulación con `vt100`, mock `wiremock`) usado como `dev-dependency` de `codeloom-cli`. Un script Python pinta los volcados JSON en PNG. Un script PowerShell sirve un comando con `ttyd` solo en loopback.

**Stack:** Rust 2021, `portable-pty`, `vt100`, `wiremock`, `tempfile`, `sysinfo`, `serde`; Python 3 + Pillow; `ttyd`.

**Spec:** `docs/specs/2026-10-30-codeloom-e2e-visual.md` (leerla antes).

## Global Constraints

- Los cuatro checks de `AGENTS.md` pasan en cada commit: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`.
- `codeloom-core` no depende de `ratatui` ni `crossterm`; `codeloom-e2e` no lo usa ningún crate de producción (`publish = false`).
- Sin red real en los tests: proveedor siempre `wiremock`; el catálogo models.dev se siembra en el perfil temporal.
- Los tests nunca tocan `~/.codeloom` del usuario ni su daemon.
- Entorno (Git Bash en Windows), en el mismo comando de cada `cargo`:
  `export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.35.32215\bin\Hostx64\x64\link.exe'` y
  `export CARGO_TARGET_DIR='C:\Users\negrero\AppData\Local\Temp\codeloom-e2e-target'` (el disco `E:` está al 98 %; un `codeloom.exe` abierto en `target/` bloquea el enlazado).
- No hacer `git push`. Los ficheros `crates/codeloom-cli/src/{ipc,server}.rs` tienen cambios del usuario sin commitear: **no editarlos** salvo que la tarea 4 lo exija, y entonces con `git add -p` solo de las líneas propias.
- Comentarios en español siguiendo el fichero; identificadores en inglés como el resto del workspace.

## Review Focus

- El hijo muere antes de pintar nada: `wait_for_text` debe fallar con la pantalla y el código de salida en el mensaje, no colgarse (tarea 2).
- Terminal pequeña (20×60): la TUI arranca sin `panic` y el volcado no se trunca (tarea 4).
- Pegado enorme (miles de líneas, CRLF): el composer lo acota y sigue respondiendo (tarea 4).
- Daemon huérfano tras el test: no debe quedar ningún `codeloom --server` del perfil temporal (tarea 4).
- Puerto ocupado o `ttyd` ausente en el script de captura: error claro y salida distinta de cero (tarea 6).

---

### Task 1: Crate `codeloom-e2e` y `Snapshot`

**Files:**
- Create: `crates/codeloom-e2e/Cargo.toml`, `crates/codeloom-e2e/src/lib.rs`, `crates/codeloom-e2e/src/snapshot.rs`
- Modify: `Cargo.toml` (añadir `"crates/codeloom-e2e"` a `members`)

**Interfaces:**
- Produces (`snapshot.rs`):
  - `enum ColorSnap { Default, Idx(u8), Rgb(u8, u8, u8) }`
  - `struct CellSnap { ch: String, fg: ColorSnap, bg: ColorSnap, bold: bool, inverse: bool, underline: bool }`
  - `struct Snapshot { rows: u16, cols: u16, cursor: (u16, u16), text: String, cells: Vec<Vec<CellSnap>> }`
  - `Snapshot::from_screen(screen: &vt100::Screen) -> Snapshot`, `Snapshot::to_json(&self) -> String`, `Snapshot::row_text(&self, row: usize) -> String` (sin espacios finales).
  - Todos con `Serialize, Deserialize, Clone, Debug, PartialEq`.

- [ ] **Step 1: Escribir las pruebas** en `snapshot.rs` (`#[cfg(test)]`)
  - `captures_text_and_colors`: `Parser::new(5, 20, 0)`, `process(b"\x1b[31mhola\x1b[0m x")`; assert `snap.row_text(0) == "hola x"`, `snap.cells[0][0].fg == ColorSnap::Idx(1)`, `snap.cells[0][5].fg == ColorSnap::Default`.
  - `json_round_trips`: `serde_json::from_str::<Snapshot>(&snap.to_json()).unwrap() == snap`.
  - `records_cursor_and_size`: tras `process(b"ab")`, `snap.cursor == (0, 2)` y `(snap.rows, snap.cols) == (5, 20)`.
- [ ] **Step 2:** `cargo test -p codeloom-e2e` → falla por símbolos inexistentes.
- [ ] **Step 3: Implementar** `Snapshot` leyendo `screen.cell(r, c)` (`contents()`, `fgcolor()`, `bgcolor()`, `bold()`, `inverse()`, `underline()`), `screen.cursor_position()` y `screen.size()`. Versión de `vt100` fijada con `cargo add vt100@0.15`; si la API difiere, adaptar sin cambiar la interfaz de arriba.
- [ ] **Step 4:** `cargo test -p codeloom-e2e` → 3 pasan.
- [ ] **Step 5: Commit** `feat(e2e): crate de apoyo con volcado de pantalla` (`git add Cargo.toml Cargo.lock crates/codeloom-e2e`; si `Cargo.lock` tiene cambios ajenos del usuario, `git add -p`).

### Task 2: `Term` (PTY, lector, teclas, esperas)

**Files:**
- Create: `crates/codeloom-e2e/src/term.rs`
- Modify: `crates/codeloom-e2e/src/lib.rs` (reexportar), `crates/codeloom-e2e/Cargo.toml` (`portable-pty`, `anyhow`)

**Interfaces:**
- Consumes: `Snapshot::from_screen`.
- Produces:
  - `struct TermConfig { program: PathBuf, args: Vec<String>, cwd: PathBuf, env: Vec<(String, String)>, rows: u16, cols: u16 }`
  - `enum Key { Enter, Esc, Tab, Backspace, Up, Down, Left, Right, CtrlC, CtrlD, F1 }` con `fn bytes(&self) -> &'static [u8]`
  - `Term::spawn(cfg: TermConfig) -> anyhow::Result<Term>`
  - `Term::send_text(&mut self, s: &str)`, `send_bytes(&mut self, b: &[u8])`, `press(&mut self, k: Key)`, `paste(&mut self, s: &str)` (envuelve en `ESC[200~ … ESC[201~`), `resize(&mut self, rows: u16, cols: u16)`, todos `-> anyhow::Result<()>`
  - `Term::snapshot(&self) -> Snapshot`, `Term::wait_for_text(&self, needle: &str, timeout: Duration) -> Result<Snapshot, WaitError>`
  - `Term::save(&self, dir: &Path, name: &str) -> anyhow::Result<()>` (escribe `name.txt` y `name.json`)
  - `Term::exit_code(&mut self) -> Option<u32>`; `Drop` mata al hijo.
  - `pub fn cpr_reply(chunk: &[u8], cursor: (u16, u16)) -> Option<Vec<u8>>`
  - `WaitError` (`thiserror`) con `Timeout { needle, screen: String }` y `ChildExited { code: Option<u32>, screen: String }`.

- [ ] **Step 1: Pruebas** (`#[cfg(test)]` en `term.rs`)
  - `cpr_reply_answers_cursor_query`: `cpr_reply(b"x\x1b[6ny", (0, 0)) == Some(b"\x1b[1;1R".to_vec())`; `cpr_reply(b"nada", (0, 0)) == None`.
  - `key_bytes`: `Key::Enter.bytes() == b"\r"`, `Key::Esc.bytes() == b"\x1b"`, `Key::CtrlC.bytes() == b"\x03"`.
  - `spawn_echo_and_wait`: `TermConfig` con `program = COMSPEC`, `args = ["/c", "echo hola"]`, 24×80; `wait_for_text("hola", 10 s)` devuelve `Ok` y `snap.text.contains("hola")`.
  - `child_exit_before_text_fails_fast`: mismo `cmd /c exit 3`, `wait_for_text("nunca", 10 s)` → `Err(WaitError::ChildExited { code: Some(3), .. })` en menos de 5 s.
  - `paste_wraps_in_bracketed_paste`: función pura `fn paste_bytes(s: &str) -> Vec<u8>` devuelve `ESC[200~` + s + `ESC[201~`.
- [ ] **Step 2:** `cargo test -p codeloom-e2e` → fallan.
- [ ] **Step 3: Implementar** `Term`:
  - `native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`; `CommandBuilder` con `program`, `args`, `cwd`, `env`; `pair.slave.spawn_command`.
  - Hilo lector sobre `master.try_clone_reader()`: por cada bloque, `parser.lock().process(&buf)` y, si `cpr_reply(&buf, parser.screen().cursor_position())` devuelve `Some`, la escribe por el writer compartido (`Arc<Mutex<Box<dyn Write + Send>>>` de `take_writer()`).
  - `wait_for_text`: bucle cada 25 ms sobre `Snapshot`; si el hijo ya salió (`try_wait`), devuelve `ChildExited` con la pantalla; al vencer el plazo, `Timeout`.
  - Tras `spawn`, soltar `pair.slave` para que el lector vea EOF al morir el hijo.
- [ ] **Step 4:** `cargo test -p codeloom-e2e` → todas pasan.
- [ ] **Step 5: Commit** `feat(e2e): terminal PTY con emulación y respuesta a la consulta de cursor`.

### Task 3: Mock del provider y `Workspace`

**Files:**
- Create: `crates/codeloom-e2e/src/mock.rs`
- Modify: `crates/codeloom-e2e/src/lib.rs`, `Cargo.toml` del crate (`wiremock = "0.6"`, `tempfile`, `tokio` con `macros`/`rt`)

**Interfaces:**
- Produces:
  - `async fn mock_provider(reply: &str) -> wiremock::MockServer` — responde `POST /chat/completions` con SSE (`delta.content` = `reply`, luego `finish_reason: "stop"`), igual que `crates/codeloom-cli/tests/e2e.rs`.
  - `struct Workspace { pub home: tempfile::TempDir, pub project: tempfile::TempDir }`
  - `Workspace::new(base_url: &str) -> std::io::Result<Workspace>`: escribe `project/codeloom.toml` con `model = "mock-model"`, `max_steps = 3` y `[provider] kind = "openai_compat"`, `base_url = <base_url>`, `api_key_env = "CODELOOM_E2E_KEY"`; siembra `home/.codeloom/models-dev.json` con un catálogo mínimo válido (misma forma que usan las pruebas de `crates/codeloom-core/src/catalog.rs`; su fecha de modificación es «ahora», así que no está caducado).
  - `Workspace::env(&self) -> Vec<(String, String)>`: `USERPROFILE`, `HOME` (= `home`), `CODELOOM_E2E_KEY=sk-test`.

- [ ] **Step 1: Pruebas** `workspace_writes_config_and_catalog`: tras `Workspace::new("http://127.0.0.1:1")`, `project/codeloom.toml` contiene `base_url = "http://127.0.0.1:1"` y `CodeLoomConfig::load` del core (si es accesible; si no, `toml` parse) lo acepta; `home/.codeloom/models-dev.json` existe y `codeloom_core::catalog` lo lee sin refrescar (reutilizar su función de lectura de caché; añadir `codeloom-core` como `dev-dependency` solo para esta prueba). `mock_provider_serves_sse`: `reqwest`/`wiremock` `received_requests` tras un `POST` manual devuelve 200 con `text/event-stream`.
- [ ] **Step 2:** fallan. **Step 3:** implementar. **Step 4:** pasan.
- [ ] **Step 5: Commit** `feat(e2e): provider simulado y espacio de trabajo temporal`.

### Task 4: Escenarios contra el binario real

**Files:**
- Create: `crates/codeloom-cli/tests/tui_pty.rs`
- Modify: `crates/codeloom-cli/Cargo.toml` (`[dev-dependencies] codeloom-e2e = { path = "../codeloom-e2e" }`), `crates/codeloom-e2e/src/lib.rs` (`kill_daemons_of(home: &Path) -> usize` con `sysinfo`)

**Interfaces:**
- Consumes: `Term`, `TermConfig`, `Key`, `mock_provider`, `Workspace`.
- Produces: `fn kill_daemons_of(home: &Path) -> usize`: termina los procesos `--server` cuyo `USERPROFILE` sea `home` y devuelve cuántos mató. Cada test llama a esto en su cierre (patrón RAII `struct Session` con `Drop`).

- [ ] **Step 1: Reconocimiento (sin código)**: leer `run_client` en `crates/codeloom-cli/src/server.rs` y anotar en el spec (apartado «Hallazgos») cómo se lanza el daemon y si `dirs::home_dir()` respeta `USERPROFILE`. Si no lo respetara, añadir un helper `home()` con override `CODELOOM_HOME` y sustituir las seis llamadas (`ipc.rs:128`, `ipc.rs:237`, `server.rs:663`, `:1893`, `:3044`, `:3051`) con `git add -p` de solo esas líneas.
- [ ] **Step 2: Pruebas** (`#[tokio::test(flavor = "multi_thread")]`, `#[ignore]` no: se ejecutan siempre, con `env!("CARGO_BIN_EXE_codeloom")`):
  - `starts_and_shows_header_and_composer`: espera `"CodeL∞M"`; guarda `target-e2e/start.{txt,json}` con `Term::save`; assert cursor dentro del tercio inferior de la pantalla.
  - `a_turn_shows_prompt_as_you_and_streamed_reply`: `mock_provider("respuesta simulada")`; `send_text("hola")`, `press(Enter)`; espera `"respuesta simulada"`; assert que la pantalla contiene `"You"` y `"hola"`.
  - `large_multiline_paste_keeps_composer_alive`: `paste` de 200 líneas `linea N`; espera el marcador de pegado grande (texto exacto del spec `2026-10-29-codeloom-large-paste-placeholder`); luego `send_text("x")` y comprueba que el composer sigue aceptando entrada.
  - `tiny_terminal_does_not_panic` (20×60): arranca, `wait_for_text("CodeL")`, el hijo sigue vivo (`exit_code() == None`).
  - `huge_crlf_paste_is_bounded`: pegado de 5000 líneas con `\r\n`; el hijo sigue vivo y responde a `send_text("y")` en menos de 5 s.
  - `no_orphan_daemon_after_session`: al soltar `Session`, `kill_daemons_of(home) >= 0` y una segunda llamada devuelve `0`.
- [ ] **Step 3:** `cargo test -p codeloom-cli --test tui_pty` → falla (no compila / no arranca).
- [ ] **Step 4: Implementar** `kill_daemons_of` con `sysinfo` (refrescar procesos con `cmd` y `environ`; la firma exacta depende de la versión resuelta) y los helpers de la `Session`. Ajustar esperas y textos hasta que pasen; no relajar las aserciones de la lista.
- [ ] **Step 5:** `cargo test -p codeloom-cli --test tui_pty` → 6 pasan; `tasklist | grep codeloom` no muestra procesos del perfil temporal.
- [ ] **Step 6:** los cuatro checks completos (con el `CARGO_TARGET_DIR` de Global Constraints).
- [ ] **Step 7: Commit** `test(e2e): la TUI real se prueba en un PTY de Windows`.

### Task 5: PNG desde el volcado

**Files:**
- Create: `scripts/visual/render_screen.py`

**Interfaces:**
- Produces: `python scripts/visual/render_screen.py <snapshot.json> <salida.png> [--cell 9x18]`; código de salida distinto de cero si el JSON no tiene `cells`.

- [ ] **Step 1:** guardar un fixture real: ejecutar `starts_and_shows_header_and_composer` y copiar `start.json` a `crates/codeloom-e2e/tests/fixtures/start.json`.
- [ ] **Step 2: Implementar** el script con Pillow: lienzo `cols*w × rows*h`; fondo por `bg` (Default = `#1e1e1e`), texto por `fg` (Default = `#d4d4d4`; `Idx` con la paleta xterm de 256; `Rgb` directo); `inverse` intercambia; `bold` usa la variante negrita si existe; subrayado con línea; cursor como bloque. Fuente: Cascadia Mono → Consolas → Courier New, la primera que exista en `C:\Windows\Fonts`.
- [ ] **Step 3: Verificar**: `python scripts/visual/render_screen.py crates/codeloom-e2e/tests/fixtures/start.json $TMP/start.png` termina con 0; abrir el PNG con `Read` y comprobar que se lee `CodeL∞M`. Con un JSON sin `cells` el script sale con 2.
- [ ] **Step 4: Commit** `feat(e2e): script que pinta el volcado de la pantalla en PNG`.

### Task 6: Captura web con `ttyd` (opción C)

**Files:**
- Create: `scripts/visual/serve-tui.ps1`, `docs/visual-e2e.md`

**Interfaces:**
- Produces: `serve-tui.ps1 -Port <int=7681> -Cwd <dir> -- <comando> [args…]`. Sale con 1 y mensaje claro si `ttyd` no está en el `PATH` o el puerto está ocupado. Escribe el PID en `$env:TEMP\serve-tui-<port>.pid`. Solo escucha en `127.0.0.1`.

- [ ] **Step 1:** instalar `ttyd` (`winget install tsl0922.ttyd`; si el paquete no existe, `scoop install ttyd`) y comprobar `ttyd --version`.
- [ ] **Step 2: Implementar** el script: validar `ttyd`, puerto libre (`Get-NetTCPConnection`), lanzar `ttyd -W -i 127.0.0.1 -p $Port <comando>` en segundo plano desde `-Cwd`, imprimir `http://127.0.0.1:<Port>`.
- [ ] **Step 3: Escribir `docs/visual-e2e.md`**: cuándo usar B y cuándo C; el flujo con Chrome (`tabs_context_mcp`, `tabs_create_mcp`, `navigate`, `resize_window` para fijar columnas, `computer` para teclas y captura); cómo parar (`Stop-Process -Id (Get-Content …pid)`); advertencia de que C no valida pegado ni teclas.
- [ ] **Step 4: Verificación en vivo**: servir `codeloom` con `-Cwd` de un `Workspace` temporal (perfil aislado) y capturar la TUI con Chrome; servir el showcase de `batten` (`cargo run -p batten --example showcase`) y capturar un componente. Guardar las capturas fuera del repo y enseñarlas.
- [ ] **Step 5: Verificar errores**: con el puerto ocupado el script sale con 1; con `ttyd` fuera del `PATH` (`$env:PATH` vacío en el subproceso) sale con 1 y mensaje.
- [ ] **Step 6: Commit** `docs(e2e): guía y script para capturar herramientas de terminal por web`.

### Task 7: Cierre documental

**Files:**
- Modify: `docs/inventario.md`, `docs/README.md`, este plan, el spec

- [ ] **Step 1:** cambiar la cabecera de este plan y del spec a `**hecho**` con la fecha y los commits.
- [ ] **Step 2:** en `docs/inventario.md`, anotar que el e2e en PTY existe y listar qué cabos sueltos pueden cerrarse con él (multiline-composer, large-paste, image-attachments, atribución `You`, subagent-empty-final-response) sin cerrarlos todavía: cada uno se cierra al añadir su escenario.
- [ ] **Step 3:** enlazar `docs/visual-e2e.md` desde `docs/README.md`.
- [ ] **Step 4:** mover spec y plan a `hechos/` y reescribir enlaces (mismo procedimiento que el traslado del 2026-10-30).
- [ ] **Step 5: Commit** `docs: cierre del e2e visual`.
