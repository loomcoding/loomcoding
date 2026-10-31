# Agent-first: fuentes externas y auditoría del flujo propio

**Fecha de corte:** 31 de octubre de 2026
**Propósito:** reunir lo que existe fuera del repositorio sobre diseño «agent-first», contrastarlo con
lo ya implementado en CodeLoom y alimentar el borrador de decálogo
(`docs/specs/2026-10-31-codeloom-decalogo-agent-first.md`).

## 1. Hallazgo principal

No se encontró un decálogo o manifiesto canónico «agent-first» equivalente al que existe, de manera
informal, para API-first. Tampoco hay un manifiesto canónico de API-first: lo que existe es práctica
repetida en guías de proveedores y un estándar corporativo (DHL) que exige diseñar la API antes de
implementarla, con el contrato como artefacto central. Lo más cercano a agent-first son tres fuentes
de naturaleza distinta, que se resumen abajo. La búsqueda fue acotada (cinco consultas y siete
páginas), de modo que «no se encontró» no equivale a «no existe».

## 2. Fuentes y grado de verificación

| Fuente | Naturaleza | Verificación del texto |
|---|---|---|
| Anthropic, *Building effective agents* (19-dic-2024) | Guía de ingeniería | Literal |
| agentpatterns.ai, *Agent-First Software Design* (revisada 2026-06-12) | Catálogo de patrones, sin autoría ni evidencia cuantitativa | Literal |
| Gorinova et al., *Position: Coding Benchmarks Are Misaligned with Agentic Software Engineering* (arXiv 2606.17799; taller SE 3.0 de KDD 2026) | Artículo de posición con datos de terceros | Literal, secciones 1 a 6 |
| Bandara et al., *Towards an Agent-First Web* (arXiv 2606.19116, jun-2026, preprint) | Propuesta para la web abierta, sin experimentos propios | **Parcial**: los diez principios proceden de un resumen, no del texto literal |

La última fuente trata de la web (acceso, economía y contenido), no del diseño de un arnés de
agentes. Se cita como antecedente del término, no como base del decálogo. Antes de citar sus diez
principios hay que cotejar su redacción con el PDF; la sección 6.1 (principios de contenido) no se leyó.

### 2.1 Anthropic — *Building effective agents*

Tres principios centrales, literales: mantener la simplicidad del diseño; priorizar la transparencia
mostrando los pasos de planificación; elaborar con cuidado la interfaz agente-ordenador (ACI) con
documentación y pruebas de las herramientas. Recomienda añadir complejidad solo cuando mejora de
forma demostrable los resultados, empezar llamando a la API directamente y medir con evaluación. En el
apéndice sobre herramientas: ponerse en el lugar del modelo, probar cómo usa las herramientas y diseñar
a prueba de errores (poka-yoke); su ejemplo es exigir rutas absolutas. La página carece de cifras.

### 2.2 agentpatterns.ai — *Agent-First Software Design*

Cuatro principios con nombre: estructurado sobre visual, interfaces autodescriptivas, herramientas a
prueba de errores y determinista sobre probabilístico. Afirma que agent-first no significa agent-only
y que la observabilidad humana exige inversión explícita. Declara como costes la menor observabilidad,
el mayor coste inicial de metadatos y el riesgo de optimización prematura. No aporta evidencia
cuantitativa.

### 2.3 Gorinova et al. — evaluación del arnés

Sostiene que los benchmarks de código mezclan modelo, arnés y entorno en una sola cifra. Propone
registrar metadatos de envío (modelo, versión del arnés, hash del entorno, versión del conjunto de
datos), incluir al menos una ablación sobre un eje que no sea el modelo, sustituir la solución de
referencia única por verificadores que admitan varias formas válidas, y tratar los componentes del
arnés como objetivos de evaluación. Dato citado: con el mismo modelo, en Terminal-Bench, la
clasificación va de 79,8 a 58,0 según el arnés. Es lo que más condiciona el diseño de una evaluación
propia.

## 3. Auditoría: qué parte del flujo agent-first está implementada

El flujo coordinado por agentes (spec `2026-10-25-codeloom-agent-development-workflow.md`, aprobada el
2026-10-27) tiene nueve criterios de aceptación. Estado según el plan: implementación y verificaciones
automáticas cerradas el 2026-10-29; revisión visual pendiente.

| Criterio | Cobertura localizada |
|---|---|
| 1 Spec y plan sin implementar | `planner_that_reports_missing_files_fails_without_starting_implementation` y los tests de planificador |
| 2 Sin aprobación no se delega | `non_exact_plan_approval_cancels_without_launching_implementer` |
| 3 Referencias a artefactos aprobados | `child_ids_are_kept_by_role_and_round`, hash de artefactos |
| 4 Revisor de solo lectura | `reviewer_cannot_execute_a_model_requested_write_tool`; `completed_workflow_resumes_from_store_and_requires_read_only_review` |
| 5 Dos rondas de corrección | `correction_rounds_stop_at_two_and_return_control_to_user` |
| 6 Revisión limpia no completa | aserción de `AwaitingFinalAcceptance` en `completed_workflow_resumes_from_store…` |
| 7 Fallos y cancelaciones no son éxito | `status_fails_a_completed_planner_with_an_empty_report`, test de aprobación no exacta |
| 8 Reconstruible desde el historial | `workflow_state_and_append_only_events_survive_reopen`; test de la TUI de fase y decisión pendiente |
| 9 Cambiar el artefacto invalida la aprobación | `changing_an_approved_artifact_invalidates_its_hash` |

Limitaciones de esta auditoría: se cotejaron los nombres de los tests y una aserción; no se leyó el
cuerpo de cada uno ni se ejecutó el flujo de extremo a extremo con un proveedor real. La verificación
del criterio 8 en pantalla sigue pendiente. Además, las transiciones válidas viven en código
(`WorkflowPhase::can_transition_to`), no en instrucciones al modelo, y el bloqueo de escrituras del
padre mientras hay flujo activo está en el despacho (`agent/dispatch.rs`).

## 4. Lo que contrasta con la práctica externa

- **Coincide:** transparencia (conversación como diario), simplicidad (sin motor genérico de
  workflows), ACI cuidada (decálogo de contexto y coste), determinismo en transiciones y límites.
- **Va más allá de lo hallado:** aprobación ligada a la huella del contenido con fallo cerrado, y
  registro de capacidades en el que lo desconocido nunca cuenta como soportado.
- **Falta:** una evaluación que separe el efecto del arnés del efecto del modelo (el punto de Gorinova
  et al.). No existe aún ningún dato propio que respalde que las puertas de aprobación o el registro de
  capacidades mejoren algo medible.
