# Plan — Artefactos de progreso publicados por el hijo

> Estado: aprobado (2026-11-01). En implementación; tareas hechas: 1 a 5.
> Spec: `docs/specs/2026-11-01-codeloom-child-progress-artifacts.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real: proveedor simulado.
- `codeloom-core` no depende de `ratatui` ni `crossterm`.
- Toda salida de herramienta y todo bucle con cota (decálogo).
- Sin hijos que publiquen, comportamiento y pruebas existentes idénticos.

## Tareas

1. **Almacén y marca (núcleo).** Pruebas primero (criterios 2, 4 y 8): `SessionStore::append_subagent_artifact(child,
   artifact)` con reemplazo por nombre, devolviendo el descriptor; implementación SQLite y la del almacén de pruebas;
   `complete_subagent` conserva los de progreso; `result_text` y `artifacts_text` del resultado ignoran los marcados
   `kind: progress`. Luego el código en `session/mod.rs` y `session/sqlite.rs`.
2. **Herramienta `artifact_publish`.** Pruebas primero (criterios 1, 3 y 7): publica, publica `SubagentChanged`, rechaza
   cada cota sin cambiar nada, no se ofrece a la raíz. Localizar dónde se compone el registro de herramientas del hijo y
   registrarla solo ahí. Luego `tools/artifact_publish.rs`.
3. **Lectura del padre y aviso.** Pruebas primero (criterio 4 y 5): `subagent_list` lista `{name, bytes}`;
   `artifact_read` limita al llamador y acota; el aviso de finalización añade la línea de artefactos de progreso. Luego
   `tools/artifact_read.rs`, `subagent_list.rs` y el constructor del aviso.
4. **Flujo A2A.** Prueba (criterio 6): un artefacto publicado en marcha produce `TaskArtifactUpdateEvent` con
   `append = false` y `lastChunk = true` por el flujo existente. Solo cambia código si la prueba lo exige.
5. **Interfaz.** Pruebas primero: el nodo del hijo muestra de 1 a 3 líneas del último artefacto de progreso (nombre y
   arranque del contenido, truncado por ancho) y su recuento; sin artefactos no añade líneas; el contenido completo no
   se pinta. Luego `codeloom-tui` (`NodeView` de `batten` y el carril).
6. **Cierre.** Cuatro comprobaciones, medir el aumento de tokens del esquema en la petición del hijo y dejarlo en la spec,
   actualizar `agent-autonomy` (fase B hecha), el inventario y esta spec. Commit.
