# loomcoding-workflows

Flujos de trabajo de Loomcoding. Hoy contiene `odd` (desarrollo orgánico): el documento de
feature, su formato Markdown, sus cotas y la herramienta `odd_track`, que lo crea y lo actualiza
en `.loomcoding/odd/<feature>.md` del proyecto. Guía de uso: `docs/odd.md`.

**Regla de dependencia:** `loomcoding-workflows` puede depender de `loomcoding-core` (y de
`thiserror`), pero `loomcoding-core` nunca depende de este crate ni nombra ningún flujo. Los modos que este crate
publica (`flow_modes()`) llegan a la interfaz por la configuración `[flows.<kind>]`, que el núcleo lee
sin conocer sus nombres. Así el núcleo se mantiene sin flujos concretos, según
`docs/specs/2026-11-05-loomcoding-extraer-flujos-del-core.md`.

El formato del documento (`odd::doc`, `odd::render`) es puro. Las operaciones (`odd::ops`) también:
no tocan disco. El disco (`odd::store`), la evidencia de las casillas (`odd::evidence`, que consulta
los 500 mensajes recientes de la sesión) y la herramienta (`odd::tool`) viven aparte.

`register(&mut ToolRegistry) -> OddState` añade `odd_track` (modo `Exclusive`, `parent_only`). El
`OddState` devuelto se comparte con tres piezas que `loomcoding-cli` engancha **solo al agente
principal**:

- `OddGate` (`TurnCloseGate`): bloquea el cierre del turno mientras queden tareas pendientes.
- `OddObserver` (`ToolObserver`): infiere la fase de las herramientas y publica `FlowChanged`.
- `OddPrompt` (`PromptContributor`): la sección de prompt, estática.

`OddState` guarda en memoria la feature activa por sesión, el interruptor, el cerrojo de E/S, la
última publicación, el avance y los contadores de medición (`OddStats`, sin datos personales).
La fase es una etiqueta propia de Loomcoding, no un estado A2A.
