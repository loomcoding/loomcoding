# Origen de las imágenes

Generadas por `scripts/capturas/generar.ps1` a partir de los volcados reales de `docs/mockups/`.
No se editan a mano: para cambiarlas, cambie `scripts/capturas/capturas.psd1` o el volcado y regenere.

| Imagen | Volcado de origen | Bloque y líneas | Prueba que lo produce | Comando para regenerar |
|---|---|---|---|---|
| `pantalla-principal.svg` | `docs/mockups/compresion-e2e.md` | bloque 1, completo | `cargo test -p loomcoding-cli --test tui_pty_compresion -- --nocapture` | `.\scripts\capturas\generar.ps1` |
| `cabecera-odd-flock.svg` | `docs/mockups/vigilancia-jobs-e2e.md` | bloque 1, líneas 1-10 | `cargo test -p loomcoding-cli --test tui_pty_vigilancia -- --nocapture` | `.\scripts\capturas\generar.ps1` |
| `aviso-job.svg` | `docs/mockups/vigilancia-jobs-e2e.md` | bloque 1, líneas 6-12 | `cargo test -p loomcoding-cli --test tui_pty_vigilancia -- --nocapture` | `.\scripts\capturas\generar.ps1` |
| `filtrado-compresion.svg` | `docs/mockups/compresion-e2e.md` | bloque 1, líneas 4-11 | `cargo test -p loomcoding-cli --test tui_pty_compresion -- --nocapture` | `.\scripts\capturas\generar.ps1` |
| `ajustes-ui.svg` | `docs/mockups/comandos-e2e.md` | bloque 1, completo | `cargo test -p loomcoding-cli --test tui_pty_comandos -- --nocapture` | `.\scripts\capturas\generar.ps1` |

Los volcados de `docs/mockups/` se copian a mano desde `target/tmp/e2e-shots/` tras correr la prueba.
El generador solo lee `docs/mockups/`; la prueba `imagenes.rs` comprueba que cada volcado existe.
