# Menú izquierdo del `Workspace` — comprobación visual

Volcado real de la TUI en un pseudo-terminal (ConPTY, 130×40), con perfil aislado
(`LOOMCODING_HOME` temporal), sin `NO_COLOR` y con `layout = "workspace"` en `ui.toml`. Lo genera la prueba
`the_left_menu_shows_the_logo_and_survives_its_effects` (`crates/loomcoding-cli/tests/tui_pty.rs`); los
volcados completos quedan en `target/.../tmp/e2e-shots/left-menu-*.{txt,json}`.

Cabe señalar que el volcado de texto no refleja los colores: los efectos del logo (reflejo, linterna, pulso)
solo cambian estilos, así que la prueba comprueba que el logo reaparece íntegro tras la ráfaga y que la
pantalla sigue viva, no el efecto concreto, que es aleatorio.

## En reposo

```text
│█   ▄▀▀▄ ▄▀▀▄ █▀▄▀█
│█▄▄ ▀▄▄▀ ▀▄▄▀ █ ▀ █
│╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱
│loomcoding
│
│◌ ready
│mock-model
│…/.tmpk0dCL6
```

Arriba, el logo «LOOM» con la franja `╱` y `loomcoding` tenue; debajo, el bloque sin título: estado,
modelo (· esfuerzo, si lo hay) y ruta corta. `Actividad`, `Sesión` y `Ficheros` no aparecen mientras no
tienen nada que contar.

## Con el diff desplegado

Sin repositorio ni agente tocando ficheros no hay bloque `Ficheros`; su dibujo (fila `▸ nombre +A −B`,
`▾` desplegado, líneas `@@`/`-`/`+`, tope de doce líneas y `… N líneas más`) lo fijan las pruebas de
`crates/loomcoding-tui/src/render/tests/sidebar_files.rs`. Para verlo en vivo: `Shift+Tab` lleva al menú,
`↑`/`↓` eligen fichero y `Enter` lo despliega.

## Cómo reproducirlo

```powershell
Remove-Item Env:NO_COLOR -ErrorAction SilentlyContinue
cargo test -p loomcoding-cli --test tui_pty the_left_menu
```
