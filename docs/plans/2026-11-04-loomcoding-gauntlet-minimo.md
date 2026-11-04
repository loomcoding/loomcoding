# Plan — Gauntlet mínimo

> Estado: **aprobado** (2026-11-04). Pendiente de ejecutar desde el paso 1. `max_rounds` por defecto 2; barra solo
> con rutas de texto; comprobación de sesgo solo tras `no_gap`.

Spec: `docs/specs/2026-11-04-loomcoding-gauntlet-minimo.md`.

Se sigue el patrón de `development_workflow` (`workflow.rs`, `tools/development_workflow/`, métodos del
`SessionStore`). Cada paso con prueba en rojo primero y los cuatro checks del repo al terminar.

## Pasos

1. [x] (2026-11-04) **Modelo puro** `crates/loomcoding-core/src/gauntlet.rs`: `GauntletPhase` (`Building`, `Judging`,
   `Rebuilding`, `AwaitingFinalAcceptance`, `Completed`, `Stopped`, `Inconclusive`, `Cancelled`, `Failed`),
   `can_transition_to`, `is_terminal`, `Gauntlet` (identidades, rutas, ronda, orden A/B por ronda, veredictos),
   `Verdict`. Sin E/S ni reloj; el sorteo del orden entra por parámetro (semilla inyectada) para que sea
   determinista en pruebas.
2. [x] (2026-11-04) **Parseo acotado del veredicto**: texto del crítico → `Verdict`; malformado o fuera de cota →
   `inconclusive`. Cotas de bytes según el decálogo.
3. [ ] **Persistencia**: tabla y métodos `create/update/get/active/latest/events` en `SessionStore` y SQLite,
   con migración, siguiendo `update_development_workflow`. Prueba de reapertura del almacén.
4. [ ] **Herramienta `gauntlet`** (`tools/gauntlet/`, `ExecutionMode::Exclusive`): acciones `start`, `advance`,
   `accept`, `cancel`, `status`. Validaciones del §8.1 y rutas legibles. Constructor con
   `start_local_workflow_agent`; crítico con `start_read_only_review`.
5. [ ] **Prompts de ronda**: al crítico solo rutas A/B, la barra no rotulada y el formato de veredicto; al
   constructor el objetivo y la brecha. Prueba de que ninguno recibe lo que no debe (resumen del constructor,
   rótulos de posición).
6. [ ] **Comprobación de sesgo**: primer `no_gap` → segundo juicio con el orden invertido; contradicción →
   `inconclusive`. Pruebas de ambos caminos.
7. [ ] **Aceptación humana**: `accept` con `ask_user` y huella del artefacto; silencio, timeout y cancelación no
   aceptan. Si cambia el artefacto tras presentarlo, se invalida.
8. [ ] **Observabilidad**: eventos al bus y al transcript con actor; la TUI muestra la fase propia y, entre
   paréntesis, el estado A2A del hijo. Revisión visual en terminal real.
9. [ ] **E2E con proveedor simulado**: un gauntlet completo (constructor → crítico `gap` → reconstrucción →
   crítico `no_gap` → sesgo coherente → aceptación) y el camino del tope de rondas. Sin red real.
10. [ ] **Documentación**: `ARCHITECTURE.md`, `CODE-MAP.md` e `INVARIANTS.md` si procede, README del crate, fila
    en `docs/INDEX.md`; la idea original pasa a «recortada en…».

## Riesgos

- Que la persistencia duplique demasiado `development_workflow`: si pasa de un tercio de código repetido, se
  evalúa extraer lo común en un plan aparte, no en este.
- Que el crítico, aun ciego, deduzca cuál es la barra por el contenido: se mide con el orden invertido y se
  documenta como limitación.
- Coste: ronda máxima 5 y cotas de salida; sin ellas el flujo no arranca.

## Verificación

```text
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```
