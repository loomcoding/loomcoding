# Plan — Interfaz de la delegación recursiva (fase 2)

> Estado: propuesta, pendiente de aprobación (2026-11-01).
> Spec: `docs/specs/2026-11-01-codeloom-recursive-delegation-ui.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real: proveedor simulado. Sin gasto.
- `codeloom-core` no depende de `ratatui` ni `crossterm`.
- Sin recursión activada, comportamiento y pruebas existentes idénticos.

## Tareas

1. **Reenvío ascendente (núcleo).** Pruebas primero, con el entorno recursivo de `subagent.rs`: con techo 2 la raíz recibe
   `SubagentChanged` y `ChildEvent` del nieto (criterio 1); con techo 1 no sube nada nuevo (criterio 2). Luego ampliar el
   filtro del bucle de `child_rx` en `arrancar` para dejar pasar esos dos eventos sin envolverlos. Commit.
2. **Descendientes en el estado de la interfaz.** Pruebas primero en `codeloom-tui/src/app.rs`: función pura
   `profundidad_y_camino(subagents, child) -> (usize, Vec<String>)` con cota 3 y descarte de padre desconocido
   (criterios 3 y 5). Luego aceptar `SubagentChanged` y `ChildEvent` cuyo `parent` sea un descendiente conocido, sin tocar
   `SubagentSettled` (criterio 6). Commit.
3. **Sangría y camino.** Pruebas primero: la cabecera del nodo del nieto lleva sangría y etiqueta `hijo › nieto`, con el
   estado A2A entre paréntesis y truncado por la izquierda (criterios 3 y 5). Luego `subagent_node_lines` aplica ambos.
   Commit.
4. **Diario.** Prueba de que las herramientas del nieto cuelgan de su nodo (criterio 4) y de que sin recursión nada cambia
   (criterio 7); ajustar `resync_tools` si hiciera falta. Commit.
5. **Cierre.** Cuatro comprobaciones, actualizar la spec de la fase 1 (§4.5 y riesgo de diario), este plan, inventario y
   candidatas. Captura visual con `CODELOOM_HOME` temporal (`docs/visual-e2e.md`) solo si el escenario simulado lo permite.
   Commit.
