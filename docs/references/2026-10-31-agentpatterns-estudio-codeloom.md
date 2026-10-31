# Estudio de agentpatterns.ai aplicado a CodeLoom (primera pasada)

**Fecha de corte:** 31 de octubre de 2026
**Fuente:** [agentpatterns.ai](https://agentpatterns.ai), catálogo de patrones y antipatrones para agentes de código.
Licencia del corpus: [CC BY 4.0](https://github.com/agentpatterns-ai/website/blob/main/LICENSE). Este documento es un
**resumen propio con interpretación y cambios**; cada fila enlaza la página original. El corpus descargado
(1.926 páginas, 17,6 MB, de `llms-full.txt`) no se versiona en este repositorio.
**Relacionado:** `2026-10-31-agent-first-fuentes-externas-y-auditoria.md`, spec
`2026-10-31-codeloom-decalogo-agent-first.md`.

## 1. Cobertura y límites (leer primero)

- El sitio tiene 1.926 páginas; `patterns/` suma 615 (336 de diseño de agentes, 220 antipatrones, 59 multiagente).
- Se puntuaron las 615 por palabras clave relacionadas con CodeLoom (heurística mía, no juicio de valor) y se
  leyeron **22 páginas, truncadas a las primeras 2.700–4.200 caracteres** cada una. No se han leído enteras, ni se han
  leído `security/`, `verification/` ni `workflows/`.
- Las cifras son las que cada página atribuye a un artículo o informe. **No se han contrastado con las fuentes
  primarias.** Varias son estudios únicos, de un solo modelo o con la advertencia de la propia página de que el margen no
  está replicado.
- El contraste con el código se hizo con lecturas puntuales (`config/mod.rs`, `subagent.rs`, `permission/mod.rs`,
  `tools/edit.rs`, `workflow.rs`, nombres de tests). Donde pone «a verificar» no se comprobó.
- Esto es una primera pasada. No concluye que CodeLoom «aplique bien» ningún patrón: solo señala coincidencias,
  huecos y preguntas.

## 2. Matriz patrón → CodeLoom

Estado: **Aplicado** (hay mecanismo en código), **Parcial**, **Ausente**, **A verificar**.

### Delegación y topología

| Patrón (página original) | Qué sostiene | CodeLoom | Estado |
|---|---|---|---|
| [Over-Orchestrated Agent Architecture](https://agentpatterns.ai/patterns/anti-patterns/prefer-simplest-agent-architecture/) | Un solo bucle antes que multiagente; el multiagente rinde entre +80,8 % y -70,0 % según la tarea (Kim et al.) | Los hijos no reciben la herramienta `subagent` (`subagent.rs`, comentario sobre `max_depth`): profundidad efectiva 1 | Parcial: límite estructural sí; falta justificar por datos cuándo delegar |
| [Delegation Threshold Calibration](https://agentpatterns.ai/patterns/agent-design/delegation-threshold-calibration/) | La delegación cuesta coordinación, unos 15 veces más tokens (Anthropic) y revisión; Codex expone `disabled`, `explicit-request-only` y `proactive` | `[agents] confirm_delegation = doubt/always/never` controla **cuándo se pregunta**, no **cuánto se delega** | Ausente el mando de agresividad |
| [Recursive Sub-Agent Delegation: Depth Limits](https://agentpatterns.ai/patterns/multi-agent/recursive-sub-agent-delegation-depth/) | Claude Code limita a 5 niveles; Codex `max_depth` por defecto 1 | Profundidad 1 de hecho; el campo `max_depth` del perfil se lee y no se aplica (comentario en `subagent.rs`); concurrencia fija en 8 (`Semaphore::new(8)`) | Parcial: tope no configurable |
| [Difficulty-Aware Topology Selection](https://agentpatterns.ai/patterns/agent-design/difficulty-aware-topology-selection/) | En 614 problemas, la colaboración mejora 2,4 puntos en fáciles y 21,1 en difíciles, al mismo precio de 5 a 10 veces | Sin selección por dificultad | Ausente. La propia página exige medir antes sobre carga propia |
| [Task Shape Decides What a Heavier Harness Buys](https://agentpatterns.ai/patterns/agent-design/task-shape-harness-payoff/) | El arnés pesado ayuda más con modelos débiles en tareas fijas y menos en abiertas; **los componentes no son independientes** | — | Condiciona el diseño del benchmark (ver sección 4) |
| [Minimum-Sufficient Control Ladder](https://agentpatterns.ai/patterns/agent-design/minimum-sufficient-control-ladder/) | Empezar por uso de herramientas y subir de peldaño solo ante un fallo nombrado; no vale para acciones irreversibles ni superficies adversas | El flujo de desarrollo es el peldaño más pesado (humano en el circuito y evaluador-optimizador) | A verificar cómo y cuándo se arranca |

### Contexto heredado por los hijos

| Patrón | Qué sostiene | CodeLoom | Estado |
|---|---|---|---|
| [Forked vs Fresh Subagents](https://agentpatterns.ai/patterns/multi-agent/forked-vs-fresh-subagents/) | Revisor con contexto limpio; fork cuando el modelo mental del padre es un activo | El hijo arranca con el mensaje de tarea (`subagent.rs`, `Message::user`); no hay fork | Aplicado el «fresco»; el fork no existe |
| [Role-Declared Context Mode](https://agentpatterns.ai/patterns/multi-agent/role-declared-context-mode/) | El modo de contexto debe declararlo el rol, no cada despacho; con contexto heredado, la revisión pierde F1 (28,6 % frente a 23,8 % en un estudio) | El perfil fija modelo, esfuerzo, turnos y skills, pero no un modo de contexto | Parcial: falta el campo declarado |
| [Subagent Inheritance Contract](https://agentpatterns.ai/patterns/multi-agent/subagent-inheritance-contract/) | Documentar qué cruza hacia abajo sin pedirlo | El hijo hereda presupuesto y, sin perfil, el modelo; hay tests (`a_child_inherits_the_budget…`) | Parcial: no existe un contrato escrito |

### Aprobación, permisos y límites

| Patrón | Qué sostiene | CodeLoom | Estado |
|---|---|---|---|
| [Binding an Agent's Effect to the Approval It Claims](https://agentpatterns.ai/patterns/agent-design/interaction-effect-obligations/) | La aprobación debe ligarse a la revisión de la tarea, no solo a argumentos válidos | Aprobación de spec y plan ligada a SHA-256; cambiar el artefacto la invalida (`changing_an_approved_artifact_invalidates_its_hash`) | Aplicado en el flujo; no en permisos de herramienta |
| [Trusting Claimed Prior Approval](https://agentpatterns.ai/patterns/anti-patterns/trusting-claimed-prior-approval/) | Un revisor que da por buena una aprobación afirmada en el texto deja pasar código malicioso (hasta 55 % en el peor caso) | La aprobación llega por respuesta exacta de `ask_user`, no por texto; los mensajes llevan origen y los antiguos fallan cerrados | Aplicado en lo central; **a verificar** si un artefacto que afirme estar «ya aprobado» engaña al revisor |
| [Prompt-Only Tool Access Control](https://agentpatterns.ai/patterns/anti-patterns/prompt-only-tool-access-control/) | Prohibir por prompt deja pasar del 4 % al 37 % de llamadas; hace falta filtrar el descubrimiento y comprobar la invocación | Herramientas bloqueadas se ocultan al proveedor (`active_workflow_hides_blocked_tools_from_the_provider`) y se bloquean en el despacho | Aplicado, en las dos capas |
| [Enforcement Modes in Spec-First Frameworks](https://agentpatterns.ai/patterns/agent-design/spec-first-framework-enforcement-modes/) | Ordenar los controles por si el agente puede editarlos: persuasión, estructura inicial, control fuera de su alcance | Las transiciones del flujo viven en código (`can_transition_to`). En cambio, `confirm_delegation` solo cambia el texto del prompt, como admite su comentario | Mixto: el flujo es del tercer modo; la delegación es del primero |
| [Specification Authority Boundary](https://agentpatterns.ai/patterns/agent-design/specification-authority-boundary/) | El agente propone; solo evidencia validada escribe el estado de «hecho»; los modelos declararon completo y el verificador rechazó 28,7 %–37,9 % | El agente no puede mover fases; la aceptación final es del usuario | Aplicado en lo esencial |
| [Deterministic Precondition Gates](https://agentpatterns.ai/patterns/agent-design/deterministic-precondition-gates/) | Predicado puro sobre el estado antes de escribir | `edit` exige coincidencia única; no hay «leer antes de escribir» | Parcial |
| [bypassPermissions Overrides allowedTools](https://agentpatterns.ai/patterns/anti-patterns/bypass-permissions-overrides-allowlist/) | Con el modo de omisión, la lista de herramientas permitidas no restringe | Existe `--yolo` (`codeloom-cli`) | **A verificar** si `--yolo` afecta a los bloqueos del flujo |
| [Classifier-Subagent Run Mode](https://agentpatterns.ai/patterns/agent-design/classifier-subagent-run-mode/) | Lista, caja de arena y clasificador; con tres veredictos (permitir, sugerir alternativa, escalar) | El enrutador `classifier` elige agente, no decide permisos | Ausente. La propia página recomienda lista + sandbox + humano si no se cumplen sus condiciones |
| [Unbounded Agent Feedback Paths](https://agentpatterns.ai/patterns/anti-patterns/unbounded-agent-feedback-paths/) | 68 bucles infinitos en 47 proyectos; el 95,6 % agotó coste | `max_steps = 30`, compactaciones por turno 3, dos rondas de corrección | Aplicado |

### Roster, plan y fronteras

| Patrón | CodeLoom | Estado |
|---|---|---|
| [Static Roster vs Runtime Definition](https://agentpatterns.ai/patterns/multi-agent/static-roster-vs-runtime-subagent-definition/): lista estática como opción segura | Perfiles en ficheros (`.agents/`), con aprobación de los de proyecto | Aplicado |
| [Plan as Control Flow](https://agentpatterns.ai/patterns/agent-design/plan-as-control-flow/): solo el 22 %–45 % de las trayectorias conserva la estructura del plan si va en el prompt | El flujo despacha por fases (planificador, implementador, revisor), pero dentro del implementador el plan va en contexto | Parcial |
| [Stochastic-Deterministic Boundary](https://agentpatterns.ai/patterns/agent-design/stochastic-deterministic-boundary/): proponente, verificador, confirmación y rechazo estructurado | Herramienta, permiso, despacho y resultado de error | A verificar que el rechazo sea estructurado |
| [Delegation as Routing, Not Authorization](https://agentpatterns.ai/patterns/anti-patterns/agent-delegation-as-routing/): A2A no propaga la identidad entre saltos | Agentes remotos con tarjeta aprobada (hash y endpoint); la profundidad local es 1 | Parcial: el salto remoto del remoto queda fuera de control |

## 3. Qué dicen sobre «modos»

Las páginas respaldan un conjunto concreto, con una condición que no conviene saltarse:

- **Modos explícitos de delegación**, ya en producto: Codex ofrece `disabled`, `explicit-request-only` y `proactive`
  ([ver página](https://agentpatterns.ai/patterns/agent-design/delegation-threshold-calibration/)).
- **Escalar el control por fallo nombrado**, no por catálogo ([peldaños](https://agentpatterns.ai/patterns/agent-design/minimum-sufficient-control-ladder/)).
- **Preajustes por forma de tarea**, pero **la elección automática exige estimar la dificultad antes de ejecutar y
  haber medido cada estructura sobre carga propia** ([topología según dificultad](https://agentpatterns.ai/patterns/agent-design/difficulty-aware-topology-selection/)).

CodeLoom ya tiene los mandos sueltos: `confirm_delegation`, `routing`, `effort`, `max_steps`, `trust_project_agents`,
`--yolo` y el flujo de desarrollo. **No tiene un concepto de «modo» que los agrupe**. Un modo sería un preajuste con
nombre sobre mandos existentes, sin semántica nueva. Habría que rotularlo como diseño propio, no de A2A.

Borrador de partida, **sin calibrar con datos**:

| Modo | Delegación | Permisos | Revisión |
|---|---|---|---|
| Directo | desactivada | los actuales | ninguna |
| Asistido | solo a petición expresa; `confirm_delegation = doubt` | los actuales | a petición |
| Riguroso | flujo completo con aprobaciones | los actuales | independiente, dos rondas |
| Desatendido | acotada por presupuesto | `--yolo` con denegación por defecto de lo no contemplado | verificador automático |

La elección automática de modo debe quedar **apagada** hasta tener medición propia.

## 4. Implicación para la evaluación

- [Isometric Harness Ablation](https://agentpatterns.ai/patterns/agent-design/isometric-harness-ablation/) describe
  casi exactamente el diseño ya propuesto: modelo fijo, quitar un subsistema, medir la caída.
- [Task Shape](https://agentpatterns.ai/patterns/agent-design/task-shape-harness-payoff/) añade un aviso: al activar
  cinco componentes a la vez se obtuvo más que con cualquiera por separado, aunque dos de ellos perdían puntos solos,
  y no se probaron subconjuntos. **Una ablación componente a componente puede subestimar o invertir el efecto real**;
  hay que incluir combinaciones.
- Es un estudio único, con modelos de pesos abiertos. No es base suficiente para fijar umbrales de CodeLoom.

## 5. Preguntas que salen y no se han resuelto

1. ¿Cómo se arranca el flujo de desarrollo y qué evita que se use en tareas triviales? (a verificar)
2. ¿Afecta `--yolo` a los bloqueos del flujo y a la ocultación de herramientas?
3. ¿Engaña a un revisor un artefacto del propio agente que diga «ya aprobado»? Es comprobable con un test.
4. ¿Se aplica `max_depth` o se retira del perfil? Hoy se lee y no se usa.
5. ¿Debe convertirse `confirm_delegation = always` en un control de código (el agente no puede saltárselo) en vez
   de una frase del prompt?
6. Falta leer `security/`, `verification/` y `workflows/`, donde probablemente estén los patrones más ligados al
   decálogo (verificación, aprobación y evidencia).
