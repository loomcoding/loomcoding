# loomcoding-workflows

Flujos de trabajo de Loomcoding. Hoy contiene `odd` (desarrollo orgánico): el documento de
feature, su formato Markdown, sus cotas y la herramienta `odd_track`, que lo crea y lo actualiza
en `.loomcoding/odd/<feature>.md` del proyecto.

**Regla de dependencia:** `loomcoding-workflows` puede depender de `loomcoding-core` (y de
`thiserror`), pero `loomcoding-core` nunca depende de este crate ni nombra ningún flujo. Así el
núcleo se mantiene sin flujos concretos, según `docs/specs/2026-11-05-loomcoding-extraer-flujos-del-core.md`.

El formato del documento (`odd::doc`, `odd::render`) es puro. Las operaciones (`odd::ops`) también:
no tocan disco. El disco (`odd::store`), la evidencia de las casillas (`odd::evidence`, que consulta
el historial de la sesión) y la herramienta (`odd::tool`) viven aparte.

`register(&mut ToolRegistry) -> OddState` añade `odd_track` (modo `Exclusive`). El `OddState`
devuelto guarda la feature activa por sesión en memoria; la persistencia la decide la tarea 3 del
plan.
