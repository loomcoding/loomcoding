# Plan — Delegación recursiva acotada por profundidad

> Estado: aprobado (2026-10-31). Hechas las tareas 1 a 4 (techo, regla de niveles, hijo que delega con lanzador anidado, tope, cancelación y presupuesto del árbol); pendientes 5 y 6.
> Spec: `docs/specs/2026-10-31-codeloom-recursive-delegation.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real: proveedor simulado en todas las pruebas. Sin gasto.
- `codeloom-core` sigue sin depender de `ratatui` ni `crossterm`.
- Sin configuración, comportamiento idéntico al actual (profundidad 1).

## Tareas (fase 1: motor)

1. **Configuración.** Pruebas primero: `[agents] max_depth` se lee, por defecto 1 y acotado a `1..=3`
   (`AgentsConfig::depth_ceiling()`). Luego el campo.
2. **Cálculo de niveles.** Función pura `levels_left(parent_left, profile_max_depth) -> u32` y `can_delegate(left)`. Pruebas
   de los criterios 1 a 4 de la spec antes del código. Cambio del aviso de perfil: `max_depth` deja de ser «declarado y no
   aplicado» y pasa a ser un valor que el techo puede recortar (se avisa el recorte al arrancar el hijo, no al leer el
   perfil).
3. **Hijo que delega.** El lanzador recibe sus «niveles restantes» y su `Arc` del semáforo. Al arrancar un hijo con
   `can_delegate`, se le construye un lanzador anidado (comparte semáforo, almacén, bus, registro, trabajos, presupuesto y
   cuenta de aprobación; `parent` = el hijo) y se registran en su `ToolRegistry` las tres herramientas de subagentes, nunca
   `development_workflow`. Pruebas de los criterios 2, 3 y 5.
4. **Tope, familia y cancelación.** Pruebas de los criterios 6, 7 y 8: semáforo compartido, `with_budget_family(raíz)` para
   los nietos, token de cancelación heredado.
5. **Aprobación.** Prueba del criterio 9 con el responder simulado.
6. **Cierre de fase 1.** Cuatro comprobaciones, actualizar spec, plan, `delegation-limits` (la decisión futura queda
   cumplida), inventario y candidatas; commit.

## Fase 2 (otra spec)

Presentación de más de un nivel en la interfaz: nieto bajo su padre con sangría y camino completo, y reflejo del trabajo
anidado en el diario de la sesión raíz.

## Puntos de parada

Tras la tarea 2 (la regla de niveles, que es lo que decide la seguridad del coste) y tras la 4 (compartición de tope,
presupuesto y cancelación), antes de cerrar.

## Fuera de esta tarea

Medir calidad o ahorro de la recursión con un modelo real (requiere batería, gasto y autorización propios).
