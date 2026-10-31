# CodeLoom — Control de la delegación: aprobación por código y mando de agresividad

> Estado: **borrador para implementar**.
>
> Fecha: 2026-10-31
>
> Origen: candidatas C3 y C1 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
> Decisiones del usuario (2026-10-31): C3 pide permiso por cada delegación, y sin usuario interactivo o con
> `--yolo` se deniega; C1 se limita al mando de delegación, elegido por clave en `codeloom.toml`.
>
> Patrones afines (agentpatterns.ai, CC BY 4.0):
> [Enforcement Modes in Spec-First Frameworks](https://agentpatterns.ai/patterns/agent-design/spec-first-framework-enforcement-modes/),
> [Delegation Threshold Calibration](https://agentpatterns.ai/patterns/agent-design/delegation-threshold-calibration/),
> [Minimum-Sufficient Control Ladder](https://agentpatterns.ai/patterns/agent-design/minimum-sufficient-control-ladder/).

## 1. Problema

- **C3.** `confirm_delegation = always` solo cambia una frase del prompt: el modelo puede saltarse la regla y lanzar un
  subagente sin que nadie lo apruebe. Es un control por persuasión, no por código.
- **C1.** No hay un mando para decidir cuánto delega el agente. Solo existe la regla de confirmación y el uso libre de la
  herramienta `subagent`.

## 2. No objetivos

- No hay elección automática de modo (queda apagada hasta tener datos propios).
- No se implementan los cuatro preajustes (directo, asistido, riguroso, desatendido): solo el mando de delegación.
- No se tocan las aprobaciones del flujo de desarrollo, que tienen las suyas.
- Concepto propio de CodeLoom; no representa estados ni semántica A2A.

## 3. Diseño

### 3.1 C3: aprobación por código

Con `confirm_delegation = always`, el lanzador de subagentes pregunta al usuario **antes** de crear el hijo: agente,
etiqueta y un extracto acotado de la tarea, con las opciones «Approve» y «Cancel». Solo lanza si la respuesta es
exactamente «Approve». Sin usuario interactivo, con `--yolo` (que concede permisos pero no responde preguntas) o si se
descarta la pregunta, **se deniega**: no se aprueba nada solo. La denegación devuelve un error al modelo y no deja
sesión hija ni tarea registrada. Los demás modos de confirmación no cambian.

### 3.2 C1: mando de delegación

`[agents] delegation = "off" | "on_request" | "proactive"`. Sin valor, el comportamiento actual.

- `off`: control real. El modelo no recibe las herramientas `subagent`, `subagent_control` ni `subagent_list`, no se le
  muestra la sección de agentes y la herramienta no dispone de lanzador.
- `on_request`: persuasión. Añade al prompt la regla de delegar solo si la persona usuaria lo pide expresamente.
- `proactive`: persuasión. Añade la regla de delegar subtareas independientes y autocontenidas de forma proactiva.

Se rotula como persuasión lo que lo es: solo `off` y la aprobación de C3 son garantías de código.

## 4. Criterios de aceptación

1. Con `always` y una respuesta «Approve», el subagente se lanza.
2. Con `always` y «Cancel», una pregunta descartada o sin usuario (incluido `--yolo`), no se lanza, el error lo dice y no
   queda sesión hija ni tarea.
3. Con los otros modos de confirmación, el lanzamiento no pregunta.
4. `delegation` se lee de `[agents]`, es opcional y rechaza valores desconocidos.
5. Con `off`, el modelo no recibe las tres herramientas ni la sección de agentes.
6. Con `on_request` o `proactive`, el prompt lleva la regla correspondiente; sin valor, no cambia.
7. Las cuatro comprobaciones del proyecto pasan.

## 5. Riesgos y preguntas abiertas

- La pregunta bloquea el turno del padre hasta que la persona responda; en una sesión desatendida eso equivale a no
  delegar. Es el comportamiento pedido.
- `on_request` y `proactive` dependen de que el modelo obedezca. Sin medición propia, no se afirma su eficacia.
- Un cambio de configuración exige reconstruir el lanzador, como con el resto de opciones.
