# Invariantes duras y cómo detectar su violación

Cabe señalar que una regla sin comprobación no es una regla, es un deseo. Cada fila lleva el
comando (desde la raíz del repo, en Git Bash) o el procedimiento exacto. Los comandos de esta
tabla se ejecutaron el 2026-11-05 y dieron el resultado indicado.

| Invariante | Comprobación | Resultado esperado |
|---|---|---|
| `loomcoding-core` no depende de `ratatui` ni `crossterm` | `grep -nE 'ratatui\|crossterm' crates/loomcoding-core/Cargo.toml` y `grep -rnE 'use (ratatui\|crossterm)' crates/loomcoding-core/src` | Sin salida |
| Las tools que escriben son `Exclusive` | `grep -c 'ExecutionMode::Exclusive' crates/loomcoding-core/src/tools/{write,edit,multiedit,bash}.rs` | Cada archivo con 1 o más |
| Una tool nueva que escribe es `Exclusive` | Procedimiento: al añadirla, abre su `execution_mode` y confirma `Exclusive` si modifica disco, procesos o estado externo | Revisión en el PR |
| Errores de librería con `thiserror` | `grep -c thiserror crates/loomcoding-core/Cargo.toml` | 1 o más |
| Una sola tabla de transiciones de `SubagentPhase` | `grep -rn 'fn can_transition_to' crates/loomcoding-core/src/subagent/phase.rs crates/loomcoding-core/src/subagent/watchdog.rs` | Solo `subagent/phase.rs` |
| Una sola tabla de transiciones de `GauntletPhase` | `grep -rn 'fn can_transition_to' crates/loomcoding-core/src/gauntlet.rs crates/loomcoding-core/src/tools/gauntlet` | Solo `gauntlet.rs` |
| Solo una persona completa un gauntlet: ni un veredicto del crítico ni un informe malformado aprueban; un veredicto sin `view` de los dos candidatos no cuenta, y una comprobación (`check`) fallida no gasta crítico | `cargo test -p loomcoding-core --lib gauntlet` (`tools::gauntlet::tests::decide`, `tests::rounds` y `subagent::tests::gauntlet_e2e`) | Todas en verde |
| El núcleo no nombra ningún flujo de `loomcoding-workflows` (única excepción: el interruptor `[odd]` en `config/`) | `grep -rniE '\bodd(_track)?\b\|odd_' crates/loomcoding-core/src \| grep -v 'config/'` | Sin salida |
| Un subagente local no hereda cierre bloqueante, observador, sección de prompt ni herramientas `parent_only`, y no añade estados A2A | `cargo test -p loomcoding-core --lib subagent::tests` (`no_herencia` y `recursion_y_skills`) | En verde |
| Sin red real en tests | `grep -rln 'api.openai.com\|api.anthropic.com' crates/*/tests` | Sin salida; los providers se simulan con `wiremock` y los `tests/*_live.rs` son opt-in |
| Nada sin cota llega al modelo | Procedimiento: toda tool, sección de prompt o bucle nuevo declara su tope y lo prueba (test de truncado); léase el decálogo | Revisión en el PR |
| Los cuatro checks en cada commit | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\checks.ps1 -TargetDir .verify-target` (Windows con daemons vivos) o `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` | Todo verde |
| No se ejecuta el binario real sin perfil aislado | Procedimiento: toda prueba o captura fija `LOOMCODING_HOME` a un directorio temporal (`docs/visual-e2e.md`) y mata sus daemons con `kill_daemons_of` | Revisión de la prueba |
| Un commit que mueve código no cambia comportamiento | Procedimiento: `git diff -M --stat` del commit muestra solo movimientos y los tests no cambian | Revisión del commit |

## Notas

- El guardarraíl automático de tamaño de archivo, de dependencias y de cobertura del índice
  llega en la Fase 1 del plan de organización
  (`docs/plans/hechos/2026-11-05-codeloom-organizacion-y-navegabilidad.md`); hasta entonces, las dos
  primeras filas se comprueban a mano con los comandos de arriba.
- Si una comprobación deja de dar el resultado esperado, o es la regla la que ha cambiado, o es
  el código el que la viola: no las silencies, decide cuál de las dos cosas y corrígela a
  propósito.
