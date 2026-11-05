# Despertar del padre sin TUI: plan

Spec: `docs/specs/2026-11-05-loomcoding-despertar-sin-tui.md`.

Reparto: el junior (Qwen3.5-9B dentro de Loomcoding) implementa; el revisor (Claude) verifica cada
tarea con los cuatro checks antes de commitear.

## Restricciones globales

- `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- `loomcoding-core` no depende de `ratatui` ni `crossterm`.
- Ningún fichero de más de 1000 líneas (`guardarrailes.rs`).
- Sin red real en pruebas; las pruebas se escriben y se ven fallar antes de implementar.

## Tarea A1: bucle `run_until_settled` en el núcleo

Ficheros: nuevo `crates/loomcoding-core/src/agent/settle.rs` (declarado en `agent/mod.rs`), nuevo
`crates/loomcoding-core/src/agent/tests/settle.rs`.

Interfaz:

```rust
pub const MAX_WAKE_TURNS: u32 = 8;
pub const MAX_SETTLE_WAIT: Duration = Duration::from_secs(30 * 60);

impl AgentLoop {
    pub async fn run_until_settled(
        &self, session: SessionId, prompt: String, cancel: CancellationToken,
    ) -> Result<TurnOutcome>;
}
```

Comportamiento: se suscribe al bus antes del primer turno; ejecuta `run`; después, mientras queden
turnos de despertar y haya hijos no terminales o una causa ya recibida, espera la siguiente causa
(con plazo y cancelación) y ejecuta `run_woken`. Devuelve el resultado del último turno.

Pruebas (nombres fijos):
1. `without_children_it_behaves_like_run`
2. `a_completed_child_wakes_the_parent_once`
3. `a_canceled_child_does_not_wake_the_parent`
4. `wake_turns_are_capped`
5. `cancellation_interrupts_the_wait`

## Tarea A2: usar el bucle en `--print`

Fichero: `crates/loomcoding-cli/src/server/daemon.rs`, rama `ClientFrame::Print`: sustituir
`agent.run` por `agent.run_until_settled`. Prueba en `loomcoding-cli` con lanzador falso: un hijo
que termina durante el turno produce un segundo turno antes de `PrintResult`.

## Tarea A3: documentación

`docs/INDEX.md` (spec y plan), `docs/CODE-MAP.md` (sección de límites del agente: dónde se decide
el despertar) y cabecera de este plan con las desviaciones.
