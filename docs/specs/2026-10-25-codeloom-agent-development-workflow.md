# CodeLoom — Flujo de desarrollo coordinado por agentes

> Estado: **aprobada el 2026-10-27** junto con el plan detallado. Implementación en curso.
>
> Fecha: 2026-10-25
>
> Relacionado: `2026-10-14-loom-skills-ponytail-superpowers.md`,
> `2026-10-15-loom-agent-profile-a2a.md`,
> `2026-10-22-codeloom-a2a-delegation-choice-design.md`,
> `2026-10-22-codeloom-subagent-tree-design.md`.
>
> Patrones afines (agentpatterns.ai, CC BY 4.0; estado en `docs/references/2026-10-31-agentpatterns-estudio-codeloom.md`):
> [Specification Authority Boundary](https://agentpatterns.ai/patterns/agent-design/specification-authority-boundary/) (aplicado),
> [Binding an Agent's Effect to the Approval It Claims](https://agentpatterns.ai/patterns/agent-design/interaction-effect-obligations/) (aplicado),
> [Phase-Specific Context Assembly](https://agentpatterns.ai/context-engineering/phase-specific-context-assembly/) (parcial),
> [Plan as Control Flow](https://agentpatterns.ai/patterns/agent-design/plan-as-control-flow/) (parcial),
> [Role-Declared Context Mode](https://agentpatterns.ai/patterns/multi-agent/role-declared-context-mode/) (parcial).

## 1. Objetivo

Permitir que el agente padre coordine un flujo de desarrollo en fases, con
agentes especializados para especificar/planificar, implementar y revisar. La
persona usuaria conserva la decisión de aprobar el plan y aceptar el resultado.

El flujo debe hacer visibles los artefactos, las transiciones, los hallazgos de
revisión y la evidencia de verificación. Un agente no puede aprobar su propio
trabajo ni avanzar una fase que requiera aprobación humana.

## 2. Flujo

```text
petición del usuario
        ↓
el usuario pide explícitamente este flujo y el padre lo inicia
        ↓
padre delega al agente de especificación y planificación
        ↓
spec + plan entregados al padre y mostrados al usuario
        ↓ aprobación explícita del usuario
implementador ejecuta el plan aprobado
        ↓ entrega de diff + comprobaciones + límites conocidos
revisor independiente compara cambio, spec y plan
        ├─ hallazgos → padre delega correcciones al implementador → nueva revisión
        └─ sin hallazgos → padre presenta evidencia y pide aceptación final
```

La aprobación del plan y la aceptación final son decisiones distintas. La
primera aprobación queda ligada al contenido exacto de la spec y el plan que se
presentaron. El padre no inicia implementación antes de esa aprobación ni marca
la tarea como terminada antes de la decisión final del usuario.

## 3. Responsabilidades

### Padre (coordinador)

- Mantiene el objetivo original, el estado de fase y las aprobaciones.
- Delega cada fase y recibe el resultado estructurado del hijo.
- Tras iniciar un hijo, termina el turno y espera el evento de finalización; no
  sondea jobs ni usa pausas de shell.
- Presenta spec/plan antes de implementación y solicita aprobación.
- Si el planner no entrega paths y archivos legibles, registra un fallo y lo
  comunica al usuario. El padre no fabrica ni repara esos artefactos.
- Si hay hallazgos, devuelve al implementador solo los cambios requeridos y
  conserva la referencia a la revisión que los originó.
- Tras revisión limpia, resume diff y evidencia y deja el cierre al usuario.
- No convierte una afirmación del agente («terminado», «tests pasan») en
  evidencia sin comprobar el resultado reportado.

### Agente de especificación y planificación

- Inspecciona el contexto necesario y produce una spec y un plan coherentes con
  las convenciones del repositorio.
- No implementa ni modifica código durante esta fase.
- Devuelve preguntas abiertas, supuestos y criterios verificables junto con los
  documentos.

### Implementador

- Trabaja únicamente sobre la spec y el plan aprobados.
- Puede ser un perfil con Ponytail: reduce alcance accidental, no elimina
  validación, seguridad, manejo de errores ni requisitos explícitos.
- Informa qué cambió, qué comprobó y qué quedó sin verificar.
- No se autoaprueba ni altera silenciosamente la spec aprobada; una desviación
  sustancial vuelve al padre para decisión.

### Revisor

- Recibe la petición original, spec aprobada, plan, diff y comprobaciones.
- Evalúa de forma independiente cumplimiento, defectos y evidencia; no hereda
  las instrucciones de alcance del implementador.
- Devuelve hallazgos accionables con severidad, ubicación y criterio incumplido,
  o una recomendación de aceptación sin hallazgos.
- No edita el cambio ni acepta la tarea en nombre del usuario.

## 4. Estados y transiciones

Estados mínimos: `specifying`, `awaiting_plan_approval`, `implementing`,
`reviewing`, `changes_requested`, `awaiting_final_acceptance`, `completed`,
`cancelled` y `failed`.

- Solo una respuesta afirmativa explícita a través del mecanismo de pregunta
  existente mueve `awaiting_plan_approval` a `implementing`; silencio, timeout,
  cancelación o respuesta ambigua no aprueban.
- `reviewing` con hallazgos lleva a `changes_requested`; el padre puede delegar
  la corrección al implementador original.
- Paths ausentes pueden pedirse de nuevo a partir del informe del planner; paths
  que este no reportó o archivos inexistentes llevan a `failed`. Nunca se lanza
  el implementador sin los dos artefactos legibles y aprobados.
- Se permiten como máximo dos rondas automáticas de corrección. Cada corrección
  produce una nueva revisión; la anterior no se sobrescribe. Si persisten los
  hallazgos, el padre presenta el bloqueo y pregunta al usuario si corrige el
  plan, continúa manualmente o cancela.
- Revisión sin hallazgos lleva a `awaiting_final_acceptance`, no a `completed`.
- La aprobación final explícita mueve a `completed`; rechazo o cancelación se
  conserva como decisión del usuario.
- Un cambio de alcance durante implementación suspende la fase y vuelve al padre
  para actualizar spec/plan y pedir aprobación cuando cambien los criterios.

Los resultados y transiciones deben estar ligados a la sesión/árbol de
subagentes existente y ser observables en la conversación. La aprobación se
asocia a una huella del contenido presentado; si spec o plan cambian después,
la aprobación queda invalidada y vuelve a pedirse. No se requiere en esta
primera versión un motor genérico de workflows.

## 5. Reintentos y límites

- El padre vuelve a delegar al mismo implementador cuando esté disponible y el
  arreglo pertenezca al alcance aprobado.
- El usuario puede detener, editar el plan o aceptar un riesgo documentado.
- Tras dos rondas de corrección, un fallo repetido, bloqueo, desacuerdo entre
  agentes o cambio de alcance, el flujo vuelve al usuario para decidir.
- Los límites de profundidad y concurrencia existentes siguen aplicando; el
  revisor no crea subagentes.

La interacción reutiliza `ask_user` y las respuestas de control existentes; la
TUI debe presentar la fase y el artefacto al que se refiere la pregunta. Los
esquemas de resultado estructurado se limitan a lo necesario para enlazar
artefactos, diff, comprobaciones y hallazgos; no se define un DSL de workflow.

## 6. Skills y perfiles

- Superpowers sigue siendo un conjunto de instrucciones de proceso cargable por
  los agentes que planifican y coordinan; no es por sí mismo el workflow runtime.
- Ponytail puede formar parte del perfil del implementador para gobernar el
  tamaño de la solución.
- El revisor usa un contexto de evaluación independiente y no hereda Ponytail
  ni las instrucciones de implementación, salvo reglas obligatorias del repo.
- Los perfiles son agentes registrados por el mecanismo existente; no se
  codifican nombres o proveedores fijos en el núcleo.

## 7. Fuera de alcance

- Implementación automática sin aprobación del plan.
- Aceptación final automática por consenso o puntuación de modelos.
- Cambios al formato de skills, clasificación automática de skills, o nuevo
  framework genérico de workflow.
- Ejecución remota con permisos implícitos: cada perfil conserva el transporte y
  límites de herramientas aprobados en su registro.
- Los perfiles remotos no forman parte de la primera versión; se incorporarán
  cuando el transporte pueda cumplir las mismas puertas y el aislamiento de
  solo lectura del revisor.

## 8. Criterios de aceptación

1. Se puede producir spec y plan sin iniciar implementación.
2. Sin aprobación explícita del plan, el padre no delega implementación.
3. El implementador recibe referencias identificables a los artefactos
   aprobados y comunica evidencia y límites de verificación.
4. El revisor compara el resultado contra petición, spec y plan, y entrega
   hallazgos accionables o recomendación sin hallazgos.
5. Los hallazgos permiten hasta dos rondas de corrección y revisión sin perder
   el historial; después el flujo pide decisión al usuario.
6. Una revisión limpia no completa la tarea: el padre espera la decisión final
   explícita del usuario.
7. Cancelaciones, fallos y desviaciones de alcance no se presentan como éxito.
8. Los eventos y estados se muestran en la conversación/árbol de subagentes y se
   pueden reconstruir desde el historial durable.
9. Cualquier cambio a spec o plan después de la aprobación invalida esa
   aprobación y requiere una nueva.
