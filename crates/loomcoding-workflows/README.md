# loomcoding-workflows

Flujos de trabajo de Loomcoding. Hoy contiene `odd` (desarrollo orgánico): el documento de
feature, su formato Markdown y sus cotas.

**Regla de dependencia:** `loomcoding-workflows` puede depender de `loomcoding-core` (y de
`thiserror`), pero `loomcoding-core` nunca depende de este crate ni nombra ningún flujo. Así el
núcleo se mantiene sin flujos concretos, según `docs/specs/2026-11-05-loomcoding-extraer-flujos-del-core.md`.

La lógica de este crate es pura: sin E/S de ficheros, sin eventos y sin herramientas.
