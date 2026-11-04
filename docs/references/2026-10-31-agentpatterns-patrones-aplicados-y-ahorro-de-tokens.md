# Patrones que Loomcoding sigue y ahorro de tokens (agentpatterns.ai)

**Fecha de corte:** 31 de octubre de 2026
**Fuente:** [agentpatterns.ai](https://agentpatterns.ai), licencia [CC BY 4.0](https://github.com/agentpatterns-ai/website/blob/main/LICENSE).
Resumen propio con interpretación; cada fila enlaza la página original. Complementa
`2026-10-31-agentpatterns-estudio-codeloom.md` (matriz de delegación, aprobación y modos).

## 1. Cobertura y límites

- Se leyeron, truncadas, 30 páginas en total entre este documento y el anterior. Para tokens se leyeron ocho (Prompt Caching,
  Static Content First, Mask Tools, Four Terms, Harness Token Economics, Copilot Cost Levers, Token-Efficient Tool Design y
  Graceful Truncation). **Cualquier otra página enlazada aquí se conoce solo por su título y su resumen de una línea**,
  incluidas las de compactación, enrutado y delegación en línea.
- Las cifras son las que cada página atribuye a un estudio o informe. **No se han contrastado con las fuentes
  primarias, y ninguna es medida de Loomcoding.** Varias ejemplifican (la propia página lo dice) o proceden de un solo
  modelo o un solo banco de pruebas.
- Para el código se hicieron lecturas puntuales. «No veo» significa que no lo encontré, no que no exista.

## 2. Registro: patrones que Loomcoding ya sigue

Estado: **Sí** (mecanismo en el código) o **Parcial**. Los patrones de delegación, aprobación y permisos están
en la matriz del documento anterior; aquí van los de contexto, herramientas y coste.

| Patrón (página original) | Dónde se ve en Loomcoding | Estado |
|---|---|---|
| [Context Compression Strategies: Offloading and Summarization](https://agentpatterns.ai/context-engineering/context-compression-strategies/) | Volcado a disco de salidas largas (`tools/spill.rs`) y resumen del historial al 80 % de la ventana, conservando 10.000 tokens finales (`agent/compact.rs`) | Sí |
| [Graceful Tool-Output Truncation](https://agentpatterns.ai/tool-engineering/graceful-tool-output-truncation/) | Recorte con cabeza y cola, marcador de bytes omitidos y ruta del volcado completo (`tools/mod.rs`, `agent/mod.rs`). Hay prefijo útil y manejador de continuación | Parcial: el marcador va en línea y la ruta al final, y la página advierte de que un aviso final se ignora |
| [Progressive Spend Threshold Alerting](https://agentpatterns.ai/patterns/agent-design/progressive-spend-threshold-alerting/) | Avisos al 80 % y al 100 % del presupuesto de sesión (`budget.rs`) | Parcial: avisa, pero «no detiene el turno» según su propio comentario |
| [Circuit Breakers for Agent Loops](https://agentpatterns.ai/observability/circuit-breakers/) | Tope de pasos (`max_steps = 30`), de compactaciones por turno (3) y de rondas de corrección (2) | Parcial: topes fijos; no hay corte por errores repetidos |
| [Static Content First](https://agentpatterns.ai/context-engineering/static-content-first-caching/): enumerar las herramientas en orden determinista | El registro de herramientas es un `BTreeMap` (`tools/mod.rs`), de modo que el orden es estable | Sí, en esa condición |
| [Subagent Schema-Level Tool Filtering](https://agentpatterns.ai/patterns/multi-agent/) y [Prompt-Only Tool Access Control](https://agentpatterns.ai/patterns/anti-patterns/prompt-only-tool-access-control/) | Herramientas bloqueadas ocultas al proveedor y bloqueadas en el despacho | Sí (ver nota en 3.2 sobre su efecto en la caché) |

## 3. Ahorro de tokens: lo que dicen las fuentes y lo que muestra el código

### 3.1 Por qué importa el orden de magnitud

[The Four Terms That Decide What an Agent Task Costs](https://agentpatterns.ai/token-engineering/four-term-agent-cost-model/)
descompone el coste en turnos, proporción de lectura de caché, tokens de salida y precio del modelo. Los tres de la
entrada se multiplican. Su ejemplo ilustrativo (la página insiste en que son cifras de ejemplo): el mismo tráfico de
entrada cuesta 11,20 $ sin caché, 1,62 $ con un 90 % de aciertos y unos 0,99 $ con un 96 %.
[Harness-Controlled Token Economics](https://agentpatterns.ai/token-engineering/harness-token-economics/) cita un
estudio con modelo fijo en el que cambiar solo el arnés bajó el coste por tarea un 41 %, sirviendo el 99,9 % de los tokens
del prompt desde caché. La propia página acota la condición: carga dominada por entrada, prefijo estable y arnés bajo
tu control. El arnés es vuestro; que la carga y el prefijo cumplan la condición en Loomcoding no está comprobado.

### 3.2 Hallazgos del código, por impacto potencial

1. **No se mide la caché.** `UsageUpdate` (`provider/mod.rs`) solo lleva tokens de entrada, de salida y coste
   comunicado. Sin tokens leídos ni escritos de caché no se puede saber qué se ahorra hoy ni qué se ahorraría.
   Es el requisito previo de todo lo demás. Aviso de la página anterior: una escritura de caché cuesta varias veces una
   lectura, de modo que una sesión con pausas largas puede pagar más.
2. **El adaptador de Anthropic envía `system` como texto plano**, sin puntos de caché (`provider/anthropic.rs`), y no
   encontré ninguna referencia a `cache_control` ni a tokens cacheados en el código de producción. Lo que no he
   comprobado es si el proveedor (directo o vía OpenRouter) aplica caché por su cuenta. **Comprobarlo exige una llamada
   real con medición de tokens**, que necesita tu autorización.
3. **Tensión entre dos patrones.** Ocultar herramientas bloqueadas al proveedor es el filtro de descubrimiento que
   recomienda *Prompt-Only Tool Access Control*. [Mask Tools Instead of Removing Them](https://agentpatterns.ai/context-engineering/mask-tools-instead-of-removing/)
   y [Prompt Caching: Architectural Discipline](https://agentpatterns.ai/context-engineering/prompt-caching-architectural-discipline/)
   advierten de que cambiar la lista de herramientas a mitad de sesión invalida el prefijo en caché. Mientras haya un
   flujo de desarrollo activo, Loomcoding cambia esa lista (`active_workflow_hides_blocked_tools_from_the_provider`).
   Las dos cosas pueden convivir si la restricción va en un control por petición en lugar de editar el array, pero eso
   depende de que el proveedor lo ofrezca. La propia página dice que enmascarar solo compensa si la disponibilidad
   cambia a menudo; con una o dos transiciones por sesión, quitar puede salir mejor. No hay que cambiar nada sin medir.
4. **`view` antepone números de línea** (`tools/view.rs`), mientras que `edit` localiza por texto exacto.
   [GitHub Copilot Cost Levers](https://agentpatterns.ai/token-engineering/copilot-cost-levers-fixed-quality/) informa de
   que quitar ese prefijo ahorró en torno al 5 % sin más fallos de edición, con una cautela relevante para nosotros:
   su efecto no es detectable sin mucho tráfico. Aquí los números de línea también sirven para `offset` y `limit`, así
   que no es un cambio gratuito sin probar.
5. **Recorte de salida.** La página citada pide que la señal de recorte esté en un lugar estructuralmente distinto
   (cabecera o campo propio) y no en una línea final. Loomcoding la deja en línea y la ruta al final. Es un cambio
   pequeño y comprobable.
6. **Estrategia de compactación.** Hoy se resume al 80 %. Hay variantes con evidencia que merece mirar:
   [Stage Elision Before Summarization](https://agentpatterns.ai/context-engineering/stage-elision-before-summarization/)
   (eliminar por reglas con umbral blando y resumir solo con el duro, el más barato en siete de ocho paneles) y
   [Half-Life Tool-Result Truncation](https://agentpatterns.ai/context-engineering/half-life-tool-result-truncation/).
   Ambas dicen medirse en bancos concretos; no hay garantía de que sirvan aquí.
7. **Delegar cuesta más que acotar.** [Subtask Boundaries Over Delegation](https://agentpatterns.ai/context-engineering/subtask-boundaries-over-delegation/)
   afirma que los ámbitos en línea igualaron la puntuación de la delegación con un 73,7 % menos de coste en un banco de
   pruebas. Enlaza con los modos del documento anterior: un modo «directo» no sería solo seguridad, sino ahorro.
8. **Enrutado por modelo.** Loomcoding enruta agentes, no modelos. [Routing Break-Even](https://agentpatterns.ai/patterns/agent-design/routing-break-even/)
   avisa de que solo compensa si el juez cuesta poco frente a la diferencia de precio.

### 3.3 Lo que no hay que hacer sin medir

Las fuentes coinciden en un patrón de fallo: ahorros que parecen obvios y regresan la calidad.
[Validating Token-Optimized Formats](https://agentpatterns.ai/context-engineering/validate-token-optimized-formats-in-agentic-loops/)
cita hasta un 27 % menos de entrada con una pérdida de 9 a 14 puntos de precisión, y
[Source Code Minification](https://agentpatterns.ai/context-engineering/source-code-minification-trade-off/) un 42 %
menos de tokens con un descenso de resolución del 50 % al 38 %. Regla del proyecto: medir cada recorte antes de adoptarlo
(decálogo de contexto y coste).

## 4. Propuesta de orden de trabajo

1. Registrar tokens de caché leídos y escritos en el registro de uso (sin cambiar comportamiento).
2. Con tu autorización y un presupuesto fijado, una llamada real mínima que diga si hay caché hoy y con qué proveedor.
3. Solo entonces, decidir sobre puntos de caché en el adaptador, el efecto de ocultar herramientas y los números de línea.
4. El cambio del marcador de recorte puede ir aparte: es pequeño y se prueba con tests.

## 5. Convención para referenciar patrones

Propuesta, sin implementar: cada spec que adopte un patrón añade una línea `Patrón:` con el enlace a la página original
y su estado (aplicado, parcial, descartado y por qué), y este registro se mantiene como índice. No se añaden enlaces en
los comentarios del código salvo en los puntos donde una decisión dependa del patrón (por ejemplo, el orden determinista
de las herramientas). Dime si prefieres otra forma.
