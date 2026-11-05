# Despertar del padre sin TUI

Estado: hecho (2026-11-05): `run_until_settled` (`f22a06d`) cableado en `--print` con `run_print_turn`. Plan: `docs/plans/2026-11-05-loomcoding-despertar-sin-tui.md`.

## Problema

En ejecuciones sin TUI (`loomcoding --print`, usado por el Gauntlet y por los pilotos con modelos
locales) el agente principal lanza subagentes y nadie le avisa de que han terminado. El modelo,
sin forma de esperar, repite «los agentes no han terminado» consultando su estado en cada paso, y
gasta pasos de lectura sin avanzar.

## Causa (verificada en el código)

- La decisión de despertar al padre vive íntegramente en la TUI: `App::take_wake`
  (`crates/loomcoding-tui/src/app/turn.rs`) marca `wake_pending` al llegar `SubagentSettled`
  (completado o fallido) o `SubagentInputRequested`, y `dispatch_wake` envía `ClientCommand::Wake`
  al servidor, que ejecuta `AgentLoop::run_woken`.
- El modo `--print` (`daemon.rs`, rama `ClientFrame::Print`) ejecuta **un único** `agent.run` y
  devuelve el texto. No hay cliente que despierte a nadie: los hijos que sigan vivos quedan
  abandonados y sus avisos no se entregan jamás.

## Decisión

Se añade al núcleo un bucle acotado, `AgentLoop::run_until_settled`, que tras el turno del
usuario espera a los hijos vivos y despierta al padre con `run_woken` mientras haya causa. La rama
`Print` del daemon lo usa en lugar de `run`. La política de la TUI no se toca: seguirá decidiendo
ella, con su presupuesto `MAX_AUTO_TURNS`.

Causas de despertar (las mismas que en la TUI, para no inventar semántica): hijo `Completed` o
`Failed`, y hijo que pide entrada. `Canceled` y `Rejected` no despiertan.

## Cotas (decálogo: toda cota declarada)

- Máximo de turnos de despertar por ejecución: 8 (igual que `MAX_AUTO_TURNS`).
- Espera máxima total a hijos: 30 minutos, tras los cuales el bucle termina y devuelve el último
  resultado con un aviso `RuntimeNotice`.
- La cancelación del token interrumpe la espera de inmediato.
- Sin hijos vivos ni causa pendiente, el bucle no espera: se comporta como el `run` de hoy.

## No objetivos

- No cambiar la política de la TUI ni mover `take_wake` al servidor.
- No añadir la herramienta de aviso programado (cron): es la tarea B, posterior.
- No tocar `COMPACT_RATIO` ni `MAX_COMPACTIONS_PER_TURN`.

## Preguntas abiertas

- ¿Debe el tope de 30 minutos ser configurable en `LimitsConfig`? Por ahora no: constante.
