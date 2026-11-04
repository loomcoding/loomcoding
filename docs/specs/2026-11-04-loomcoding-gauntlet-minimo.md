# Loomcoding — Gauntlet mínimo: constructor y crítico ciego

> Estado: **aprobada** (2026-11-04) con las decisiones de «Decisiones cerradas». Sin código aún. Recorta a un primer corte implementable la idea
> `2026-10-14-loom-gauntlet-loop.md` (que sigue siendo la fuente conceptual y queda como «idea»).
>
> Relacionado: `2026-10-25-codeloom-agent-development-workflow.md` (modelo que se imita: herramienta
> del padre, agentes registrados, estados persistidos, decisión final humana),
> `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md` (toda salida y todo bucle con cota),
> `2026-10-22-codeloom-a2a-roadmap` (no se inventan estados de tarea: cada hijo sigue siendo una tarea A2A).
> Referencia externa: `G:\loom-references\gentle-shell` (`docs/delegated-verification.md`: verificador
> independiente por riesgo; política de prompt, no límite de runtime).

## 1. Objetivo

Dar al agente padre un método **opcional y explícito** para pulir una pieza de trabajo con barra de calidad
inspeccionable: un **constructor** produce o mejora el artefacto y un **crítico independiente**, de solo
lectura y con contexto limpio, lo compara **a ciegas** con la barra y devuelve la **mayor brecha**. Se repite
hasta que el crítico no encuentra brecha, se agotan las rondas o la persona decide.

No es un modo por defecto: es el flujo más caro del producto. Sirve donde la calidad es de conjunto (interfaz,
redacción, diseño) y no donde basta una puerta barata (tests, compilación, lint).

## 2. Decisiones de este corte (a validar)

Cada una responde a una pregunta abierta de la idea original.

| Pregunta de la idea | Decisión |
|---|---|
| ¿Modo del agente o tipo de job? | **Herramienta del padre**, hermana de `development_workflow` (`gauntlet`), no un modo nuevo. |
| ¿Cómo se define la barra? | **Rutas** (una o varias) a artefactos o a un `bar.md`, pasadas en `start`. Sin formato nuevo. |
| ¿Métrica de parada? | **Veredicto del crítico** (hay brecha / no hay brecha) más **tope de rondas**. Sin métrica numérica en este corte. |
| ¿`/gauntlet` en la TUI? | **No**. Se activa porque la persona pide el método y el padre llama a la herramienta. La TUI solo lo muestra (§6). |
| ¿Jev como filtro? | **Fuera de alcance**. Depende del hub de decisiones, que es idea. |
| ¿Se miden falsos aprobados y desacuerdo? | Se **registran** el veredicto, el orden A/B y la decisión final humana; el análisis queda fuera. |

## 3. Flujo

```text
persona pide el método → padre llama gauntlet.start { goal, artifact, bar[], builder, critic, max_rounds }
        ↓
constructor (agente local, escribe) trabaja la pieza          ← ronda n
        ↓
el coordinador sortea el orden y presenta al crítico «A» y «B» sin decir cuál es la barra
        ↓
crítico (agente local, solo lectura, contexto limpio) → veredicto estructurado
        ↓
 sin brecha ──► awaiting_final_acceptance ──► persona acepta ──► completed
 con brecha ──► ronda < max_rounds ? vuelve al constructor con SOLO la brecha : stopped (límite) → decide la persona
```

El crítico recibe rutas, nunca el resumen del constructor. El constructor recibe la brecha del crítico y el
objetivo, nunca los nombres de las posiciones A/B.

## 4. Reglas

1. **Tres identidades distintas**: constructor y crítico deben ser nombres de agente local distintos y
   distintos del padre. Un agente no puntúa su propio trabajo.
2. **Crítico de solo lectura**, lanzado con `start_read_only_review` (aplicación real en el lanzador, no
   instrucción de prompt). No crea subagentes.
3. **A/B ciego con orden aleatorio**: el orden se sortea por ronda, se guarda y se invierte en la comprobación
   de sesgo (§5). El crítico no sabe cuál es la barra.
4. **Estado fresco por ronda**: cada ronda lanza un crítico nuevo; no se reutiliza su sesión.
5. **Secuencial**: un constructor, un crítico, una ronda cada vez. No hay fan-out (la evidencia de la idea
   original es que el paralelismo empeora la calidad).
6. **Cotas**: `max_rounds` por defecto 2 y máximo 5; el informe del crítico y la brecha se acotan en bytes
   (decálogo). Un valor fuera de rango no se acepta.
7. **Decisión humana final**: nunca pasa a `completed` por veredicto del crítico; hace falta una respuesta
   afirmativa explícita por el mecanismo de pregunta existente, ligada a la huella del artefacto presentado.
8. **Sin cambio de permisos**: el registro del gauntlet no concede herramientas, allowlists ni modo de
   ejecución. Cada perfil conserva sus límites.

## 5. Veredicto del crítico

Salida estructurada acotada: `verdict` (`gap` | `no_gap`), `biggest_gap` (texto corto, solo si hay brecha),
`evidence` (rutas y líneas inspeccionadas) y `preferred` (`A` | `B`).

Comprobación de sesgo de posición: en la primera ronda con veredicto `no_gap`, el coordinador **repite el
juicio con el orden invertido** una vez. Si el crítico se contradice, el resultado es `inconclusive` y decide
la persona. Es la única métrica de calidad obligatoria en este corte.

Un veredicto malformado cuenta como `inconclusive`, no como `no_gap`.

## 6. Observabilidad

- Cada hijo (constructor o crítico) sigue siendo una tarea A2A normal del árbol de subagentes; el gauntlet no
  añade estados de tarea. Los estados propios (`building`, `judging`, …) se rotulan como **propios de
  Loomcoding** y la TUI nombra entre paréntesis el estado A2A del hijo cuando los agrupa.
- Cada transición y cada veredicto son visibles en la conversación (diario) con el actor que los produce.
- Se persiste el estado y el historial de eventos igual que en `development_workflow`, para sobrevivir a un
  reinicio y poder auditar rondas, orden A/B y decisión final.

## 7. Fuera de alcance

- Activación por clasificador, `/gauntlet`, filtro Jev, métricas numéricas o benchmarks, pase de suavizado,
  descomposición en piezas, paralelismo, perfiles remotos, anidamiento (un subagente que abre un gauntlet).
- Un motor genérico de workflows: se replica el patrón de `development_workflow`, no se abstrae todavía.
- Perfiles concretos de constructor y crítico: viven en `loom-agent-workforce-poc`, no en el núcleo.

## 8. Criterios de aceptación

1. `start` rechaza identidades repetidas, rutas inexistentes o no textuales y `max_rounds` fuera de rango
   (ausente = 2).
2. El crítico se lanza solo de lectura y no recibe el resumen del constructor.
3. El orden A/B se sortea por ronda, se persiste y se invierte en la comprobación de sesgo.
4. Con `no_gap` y comprobación coherente se llega a `awaiting_final_acceptance`; con contradicción, a
   `inconclusive`.
5. Con `gap` se vuelve al constructor hasta `max_rounds`; al agotarlo se detiene y decide la persona.
6. Ni el silencio, el tiempo agotado ni la cancelación aceptan el resultado.
7. Estado y eventos sobreviven a un reinicio del almacén.
8. No hay red real en las pruebas: proveedor simulado.

## Decisiones cerradas (2026-11-04, por la persona usuaria)

- `max_rounds` por defecto **2** (coste).
- La comprobación de sesgo se aplica **solo al veredicto `no_gap`**: un `gap` falso cuesta una ronda de más;
  un `no_gap` falso da por bueno un trabajo malo.
- La barra son **solo rutas** a ficheros de texto. Capturas o imágenes quedan para una spec posterior, cuando
  el hijo de solo lectura admita imágenes y exista forma de generar la captura.

## Preguntas abiertas

Ninguna.
