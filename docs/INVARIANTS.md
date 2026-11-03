# Invariantes duras y cómo detectar su violación

Cabe señalar que una regla sin comprobación no es una regla, es un deseo. Cada fila lleva el
comando (desde la raíz del repo, en Git Bash) o el procedimiento exacto. Los comandos de esta
tabla se ejecutaron el 2026-11-05 y dieron el resultado indicado.

| Invariante | Comprobación | Resultado esperado |
|---|---|---|
| `codeloom-core` no depende de `ratatui` ni `crossterm` | `grep -nE 'ratatui\|crossterm' crates/codeloom-core/Cargo.toml` y `grep -rnE 'use (ratatui\|crossterm)' crates/codeloom-core/src` | Sin salida |
| Las tools que escriben son `Exclusive` | `grep -c 'ExecutionMode::Exclusive' crates/codeloom-core/src/tools/{write,edit,multiedit,bash}.rs` | Cada archivo con 1 o más |
| Una tool nueva que escribe es `Exclusive` | Procedimiento: al añadirla, abre su `execution_mode` y confirma `Exclusive` si modifica disco, procesos o estado externo | Revisión en el PR |
| Errores de librería con `thiserror` | `grep -c thiserror crates/codeloom-core/Cargo.toml` | 1 o más |
| Una sola tabla de transiciones de `SubagentPhase` | `grep -rn 'fn can_transition_to' crates/codeloom-core/src/subagent_phase.rs crates/codeloom-core/src/subagent_watchdog.rs` | Solo `subagent_phase.rs` |
| Sin red real en tests | `grep -rln 'api.openai.com\|api.anthropic.com' crates/*/tests` | Sin salida; los providers se simulan con `wiremock` y los `tests/*_live.rs` son opt-in |
| Nada sin cota llega al modelo | Procedimiento: toda tool, sección de prompt o bucle nuevo declara su tope y lo prueba (test de truncado); léase el decálogo | Revisión en el PR |
| Los cuatro checks en cada commit | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\checks.ps1 -TargetDir .verify-target` (Windows con daemons vivos) o `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` | Todo verde |
| No se ejecuta el binario real sin perfil aislado | Procedimiento: toda prueba o captura fija `CODELOOM_HOME` a un directorio temporal (`docs/visual-e2e.md`) y mata sus daemons con `kill_daemons_of` | Revisión de la prueba |
| Un commit que mueve código no cambia comportamiento | Procedimiento: `git diff -M --stat` del commit muestra solo movimientos y los tests no cambian | Revisión del commit |

## Notas

- El guardarraíl automático de tamaño de archivo, de dependencias y de cobertura del índice
  llega en la Fase 1 del plan de organización
  (`docs/plans/2026-11-05-codeloom-organizacion-y-navegabilidad.md`); hasta entonces, las dos
  primeras filas se comprueban a mano con los comandos de arriba.
- Si una comprobación deja de dar el resultado esperado, o es la regla la que ha cambiado, o es
  el código el que la viola: no las silencies, decide cuál de las dos cosas y corrígela a
  propósito.
