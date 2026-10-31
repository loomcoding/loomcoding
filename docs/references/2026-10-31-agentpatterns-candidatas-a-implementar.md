# Candidatas a implementar a partir de agentpatterns.ai

**Fecha de corte:** 31 de octubre de 2026
**Fuente:** [agentpatterns.ai](https://agentpatterns.ai), licencia [CC BY 4.0](https://github.com/agentpatterns-ai/website/blob/main/LICENSE);
resumen propio con interpretación. Cada candidata enlaza la página original.
**Relacionado:** `2026-10-31-agentpatterns-estudio-codeloom.md` (matriz) y
`2026-10-31-agentpatterns-patrones-aplicados-y-ahorro-de-tokens.md` (registro y tokens).

## 1. Qué es y qué no es este documento

Es una **lista priorizada de candidatas**, no una decisión. Nada está adoptado. Conforme a AGENTS.md, ninguna se
implementa sin su spec y su plan. La prioridad refleja mi juicio con los criterios de la sección 2; es discutible y la
última palabra es tuya.

Límites de lectura: 46 páginas leídas en total, todas truncadas a las primeras 2.400–4.200 caracteres. Las demás páginas citadas se conocen
por su título y resumen. Las cifras son las que cada página atribuye a un estudio, **no se han contrastado con las fuentes
primarias y ninguna es medida de CodeLoom**.

## 2. Criterios

1. **Alineación** con lo que el proyecto ya prioriza: observabilidad, ahorro de tokens, control del usuario, composer fluido.
2. **Calidad de la evidencia** que aporta la propia página: fuente nombrada, replicación, condiciones de aplicación.
3. **Coste de implementar** en este código.
4. **Riesgo** de regresión o de añadir semántica propia.
5. **Medibilidad:** qué prueba o medida diría si ha servido.
6. Lo que no cumple las condiciones de aplicación que su propia página declara **se descarta**, aunque el titular suene bien.

## 3. Candidatas

### Grupo A — Medir antes de decidir (no cambian el comportamiento)

| # | Candidata | Fuente | Por qué | Coste |
|---|---|---|---|---|
| A1 | **Registrar tokens de caché leídos y escritos y mostrar un desglose del contexto** (sistema, herramientas, MCP, memoria, mensajes) | [Four-Term Cost Model](https://agentpatterns.ai/token-engineering/four-term-agent-cost-model/), [Reducing System-Prompt Bloat](https://agentpatterns.ai/context-engineering/system-prompt-bloat-reduction/) | Hoy `UsageUpdate` solo guarda entrada, salida y coste; no hay desglose en la TUI (solo una fila «pendiente» en `funcionalidad_table.rs`). Es el requisito previo de cualquier ahorro y encaja con el valor central de observabilidad | Bajo |
| A2 (hecho; ya existía medición por petición, se añadió la de conexión: véase `docs/specs/2026-10-31-codeloom-mcp-schema-cost.md`) | **Medir el coste de las definiciones de herramientas por servidor MCP** y mostrarlo en el panel de capacidades | [MCP alwaysLoad: Eager or JIT](https://agentpatterns.ai/tool-engineering/mcp-eager-vs-jit-loading/) | `codeloom-mcp` lista las herramientas al conectar, sin carga diferida. La página cita unos 55.000 tokens para una configuración típica de varios servidores y una caída de la selección pasados 30–50 herramientas visibles; sin medir no sabemos si nos afecta | Bajo |

### Grupo B — Ahorro tras medir

| # | Candidata | Fuente | Condición para hacerla | Riesgo |
|---|---|---|---|---|
| B1 | **Puntos de caché en el adaptador de Anthropic y prefijo estable** | [Prompt Caching](https://agentpatterns.ai/context-engineering/prompt-caching-architectural-discipline/), [Harness Effect](https://agentpatterns.ai/token-engineering/harness-token-economics/) | Que A1 y una llamada real mínima (con tu autorización) muestren que hoy no se cachea. Una escritura de caché cuesta varias veces una lectura, así que las sesiones con pausas largas pueden salir peor | Medio. Tensión con ocultar herramientas al proveedor (ver B2) |
| B2 | **Restringir herramientas por petición en vez de editar la lista** | [Mask Tools Instead of Removing Them](https://agentpatterns.ai/context-engineering/mask-tools-instead-of-removing/) | Que el proveedor ofrezca un control de invocabilidad por petición, y que el flujo cambie la lista con frecuencia; con una o dos transiciones por sesión puede compensar más quitar | Medio |
| B3 | **Carga diferida de herramientas MCP** | [Filesystem-Based Tool Discovery](https://agentpatterns.ai/tool-engineering/filesystem-tool-discovery/) | Que A2 supere el umbral (la página cita 10.000 tokens como punto de partida). La cifra de 98,7 % es de un informe de Anthropic sobre ejecución de código con MCP, no extrapolable a priori | Alto |
| B4 (hecho: véase `docs/specs/2026-10-31-codeloom-truncation-signal.md`) | **Señal de recorte estructurada** (cabecera o campo propio, no línea final) | [Graceful Tool-Output Truncation](https://agentpatterns.ai/tool-engineering/graceful-tool-output-truncation/) | Ninguna previa: es pequeño y se prueba con tests. La página recoge un fallo real en que el agente tomó el prefijo por el documento completo | Bajo |
| B5 | **Quitar números de línea de `view`** | [Copilot Cost Levers](https://agentpatterns.ai/token-engineering/copilot-cost-levers-fixed-quality/) | Experimento propio con tareas fijas. La fuente informa de un 5 % sin más fallos de edición, pero avisa de que su efecto no se detecta sin mucho tráfico. Aquí los números sirven también para `offset` y `limit` | Medio |

### Grupo C — Modos y esfuerzo (valor de producto)

| # | Candidata | Fuente | Notas | Riesgo |
|---|---|---|---|---|
| C1 | **Preajustes de modo con nombre sobre los mandos existentes, más un mando de agresividad de delegación** (`desactivada`, `a petición`, `proactiva`) | [Delegation Threshold Calibration](https://agentpatterns.ai/patterns/agent-design/delegation-threshold-calibration/), [Minimum-Sufficient Control Ladder](https://agentpatterns.ai/patterns/agent-design/minimum-sufficient-control-ladder/), [Subtask Boundaries Over Delegation](https://agentpatterns.ai/context-engineering/subtask-boundaries-over-delegation/) | La elección **automática** de modo queda apagada hasta tener datos propios (lo exige [Difficulty-Aware Topology](https://agentpatterns.ai/patterns/agent-design/difficulty-aware-topology-selection/)). Concepto propio de CodeLoom: rotularlo como tal, no como A2A | Medio: semántica nueva, exige spec |
| C2 | **Esfuerzo de razonamiento por fase** (más alto al planificar y revisar, menor al ejecutar) | [Reasoning Budget Allocation](https://agentpatterns.ai/patterns/agent-design/reasoning-budget-allocation/) | El perfil ya admite `effort`; los perfiles de demostración no lo fijan. Evidencia: una sola comparación en un banco (66,5 % frente a 63,6 % y 53,9 %). Se valida con el benchmark antes de fijar valores por defecto | Bajo |
| C3 | **`confirm_delegation = always` como control de código** | [Enforcement Modes](https://agentpatterns.ai/patterns/agent-design/spec-first-framework-enforcement-modes/) | Hoy solo cambia una frase del prompt, como admite su comentario | Medio |
| C4 (hecho: véase `docs/specs/2026-10-31-codeloom-delegation-limits.md`; `max_depth` se avisa, no se aplica) | **Aplicar o retirar `max_depth` y hacer configurable la concurrencia (8 fijo)** | [Recursive Sub-Agent Delegation](https://agentpatterns.ai/patterns/multi-agent/recursive-sub-agent-delegation-depth/) | Higiene: un campo que se lee y no se usa confunde | Bajo |

### Grupo D — Robustez

| # | Candidata | Fuente | Notas |
|---|---|---|---|
| D1 | **Tope duro opcional de presupuesto** (parar al 100 %) | [Unbounded Agent Feedback Paths](https://agentpatterns.ai/patterns/anti-patterns/unbounded-agent-feedback-paths/) | Hoy `budget.rs` solo avisa. No es la *reserva por ejecución* (ver descartadas). La fuente cita que el 95,6 % de los bucles infinitos agotó el coste |
| D2 | **Cortacircuitos por herramienta** (MCP y web) | [Agent Circuit Breaker](https://agentpatterns.ai/patterns/agent-design/agent-circuit-breaker/) | Evidencia débil: umbrales heurísticos y «a calibrar por carga». Solo si se observan reintentos en bucle |
| D3 | **Registro de intención y resultado antes y después de cada llamada** | [Durable Turn Log](https://agentpatterns.ai/patterns/agent-design/durable-turn-log-crash-recovery/) | Hoy, según el test `reopened_store_marks_non_terminal_children_failed_with_message`, al reabrir los hijos no terminados se marcan como fallidos y no se reanudan. Útil solo si perder un turno resulta caro; coste alto |
| D4 (pruebas 1 y 2 hechas el 2026-10-31: `--yolo` no aprueba ninguna puerta del flujo y una afirmación de «ya aprobado» del implementador no salta al revisor; se halló y corrigió que el prompt del revisor no rotulaba el informe como afirmación a verificar. La 3.ª, reproducir antes de arreglar, exige llamadas reales a un modelo y queda pendiente de autorización y presupuesto; hoy ni el prompt de implementación ni el de corrección lo piden) | **Tres pruebas baratas:** `--yolo` frente a los bloqueos del flujo; un artefacto que afirme «ya aprobado» frente al revisor; el implementador que reproduce antes de arreglar | [bypassPermissions Trap](https://agentpatterns.ai/patterns/anti-patterns/bypass-permissions-overrides-allowlist/), [Trusting Claimed Prior Approval](https://agentpatterns.ai/patterns/anti-patterns/trusting-claimed-prior-approval/), [Premature Completion](https://agentpatterns.ai/patterns/anti-patterns/premature-completion/) | Los dos primeros son tests de comprobación. El tercero cita un estudio donde exigir reproducir primero subió un modelo pequeño del 24 % al 77 % en una tarea concreta: un caso, no una regla. Nota, a partir de la línea del índice de antipatrones «Completion Summary as the Oversight Surface» (página no leída): el revisor recibe el informe del propio implementador (`development_workflow.rs`); conviene comprobar que además inspecciona el resultado |

## 4. Descartadas por ahora, con motivo

| Patrón | Motivo, según las condiciones que declara su propia página |
|---|---|
| [Per-Run Budget Reservation](https://agentpatterns.ai/patterns/agent-design/per-run-budget-reservation/) | Exige coste objetivo por ticket, tráfico síncrono sin streaming y un único proceso; una sesión abierta de desarrollo con streaming no cumple |
| [Deterministic Fast Paths](https://agentpatterns.ai/patterns/agent-design/deterministic-fast-paths/) | Requiere una mayoría de peticiones fáciles medida, umbral calibrado y detección de errores; no hay datos |
| [Injected-Context Cost Attribution](https://agentpatterns.ai/context-engineering/injected-context-cost-attribution/) | Pensada para grafos de tres nodos o más con tokenizador del proveedor; no es nuestra forma |
| [Validating Token-Optimized Formats](https://agentpatterns.ai/context-engineering/validate-token-optimized-formats-in-agentic-loops/), [Source Code Minification](https://agentpatterns.ai/context-engineering/source-code-minification-trade-off/) | Ahorran entrada pero la fuente reporta bajadas de precisión (de 9 a 14 puntos; del 50 % al 38 %) |
| [Semantic Caching](https://agentpatterns.ai/patterns/multi-agent/semantic-caching-multi-agent/) | Añade riesgo de respuestas obsoletas y una pieza grande sin medida previa |
| [Routing Break-Even](https://agentpatterns.ai/patterns/agent-design/routing-break-even/) (enrutado por modelo) | Solo compensa si el juez cuesta poco frente a la diferencia de precio; CodeLoom enruta agentes, no modelos |

Ya cubiertas (no son candidatas): visión con varios clientes sobre una sesión y proceso supervisor
([Multi-Client Attachment](https://agentpatterns.ai/patterns/agent-design/multi-client-session-attachment/),
[Per-User Supervisor](https://agentpatterns.ai/patterns/agent-design/per-user-supervisor-process/)) por la arquitectura demonio y cliente;
flujo de eventos tipado ([Agent Event Streaming](https://agentpatterns.ai/patterns/agent-design/agent-event-streaming-model/)) por el bus de eventos. No se ha comprobado
si hay expulsión de sesiones inactivas.

## 5. Orden propuesto

1. **A1 y A2** (medir). Casi sin riesgo y condicionan todo lo demás.
2. **D4** (tres pruebas), **B4** y **C4**. Pequeñas, con tests, sin semántica nueva.
3. **C2** (esfuerzo por fase) junto con el benchmark del decálogo, y **D1** (tope duro opcional).
4. Con los datos de A1/A2: **B1/B2** (caché) y, si procede, **B3/B5**.
5. **C1** y **C3** (modos), con su spec, apoyados en el benchmark. Es lo de más valor de producto y lo que más diseño pide.
6. **D2/D3** solo si aparece el problema.

## 6. Qué necesito de ti

- Confirmar el orden, o mover alguna candidata.
- Para B1: autorización y presupuesto para una llamada real mínima que mida la caché.
- Para el benchmark (C1/C2): tareas y presupuesto de tokens.
- Si quieres, empiezo por A1 con su spec y plan, que es lo que desbloquea todo.
