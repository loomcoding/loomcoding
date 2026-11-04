# Estudio comparativo de skills y agentes para desarrollo

**Fecha de corte:** 25 de octubre de 2026  
**Ámbito:** Superpowers, Ponytail, Matt Pocock Skills, Addy Osmani Agent Skills y Agency Agents.  
**Propósito:** decidir qué patrones conviene adoptar en Loomcoding y cuáles se solapan o deben mantenerse separados.

## Resumen ejecutivo

Las cinco iniciativas no son cinco sistemas equivalentes. Superpowers y Addy Osmani describen sobre todo procesos de trabajo; Ponytail impone una disciplina de alcance y simplicidad; Matt Pocock ofrece flujos de ingeniería pequeños y conectables; Agency Agents es principalmente una biblioteca de perfiles de especialistas. Mezclarlas como un paquete único repetiría entrevistas, especificación, planificación, TDD y revisión, aumentaría el contexto y podría producir instrucciones contradictorias.

La combinación con mejor encaje para Loomcoding es una sola orquestación propia, con aprobaciones explícitas y artefactos persistentes: proceso inspirado en Superpowers, control de alcance de Ponytail, modelado de dominio y descomposición de Matt, evaluación de skills inspirada en Addy, y perfiles especializados de Agency como material de referencia sujeto a permisos del runtime. El padre debe conservar la autoridad para cambiar de fase, detenerse y devolver trabajo. La aprobación de un plan debe delimitar exactamente qué puede implementar el hijo.

## 1. Método y límites

Se revisaron README, metadatos y estructura de los repositorios oficiales, además de las descripciones y el contenido de las skills de ingeniería relevantes. Se contabilizaron 15 skills de Superpowers, 6 de Ponytail, 38 de Matt Pocock, 25 de Addy Osmani y 305 perfiles de Agency Agents. Los catálogos pueden cambiar en upstream; las cifras son una fotografía de la fecha de corte.

En Agency Agents se inventariaron los 305 perfiles y se leyeron en profundidad perfiles representativos de ingeniería, orquestación, revisión, pruebas, documentación, workflow y producto. No se leyó línea por línea el texto completo de los 305 perfiles: los perfiles de dominios ajenos a Loomcoding se clasificaron por su catálogo. Por tanto, la cobertura del catálogo es completa, pero la evaluación semántica individual de cada perfil no lo es.

Este informe compara las instrucciones publicadas; no instala ni ejecuta las skills en Loomcoding, y no afirma que mejoren su rendimiento sin una evaluación local. Las cifras de estrellas son variables y no se usan como medida de calidad técnica.

## 2. Inventario

| Proyecto | Tamaño del catálogo revisado | Tipo principal | Licencia publicada |
|---|---:|---|---|
| Superpowers | 15 skills | Flujo de desarrollo con gates y delegación | MIT |
| Ponytail | 6 skills | Disciplina de simplicidad, revisión y deuda | MIT |
| Matt Pocock | 38 skills | Flujos de ingeniería y productividad componibles | MIT |
| Addy Osmani | 25 skills | Ciclo de producto/ingeniería y evaluación de skills | MIT |
| Agency Agents | 305 perfiles | Biblioteca de roles/personas especializadas | MIT |

Fuentes primarias: [obra/superpowers](https://github.com/obra/superpowers), [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail), [mattpocock/skills](https://github.com/mattpocock/skills), [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills), [msitarzewski/agency-agents](https://github.com/msitarzewski/agency-agents). La licencia debe volver a comprobarse antes de redistribuir contenido o copiar archivos concretos.

### 2.1 Superpowers

Catálogo: brainstorming, diagnosing-superpowers, dispatching-parallel-agents, executing-plans, finishing-a-development-branch, receiving-code-review, requesting-code-review, subagent-driven-development, systematic-debugging, test-driven-development, using-git-worktrees, using-superpowers, verification-before-completion, writing-plans y writing-skills.

**Aporta:** un recorrido coherente desde entender el problema hasta integrar el cambio; especificación y plan antes de trabajo no trivial; delegación por tarea; revisión de implementación y de calidad; TDD estricto; depuración sistemática; verificación con evidencia reciente; uso de worktrees y cierre de rama.

**Lo valioso para Loomcoding:** fases explícitas, artefactos revisables, delegación por unidades pequeñas, revisión en el ciclo y una puerta de verificación antes de declarar terminado. Su flujo de subagentes permite que, una vez autorizado un plan, el padre haga avanzar las tareas y recoja resultados sin pedir permiso para cada paso rutinario.

**Costes y tensiones:** el descubrimiento y el diseño pueden resultar demasiado ceremoniales en solicitudes triviales; aplicar la skill universal en cada turno añade contexto; las convenciones de git/worktree pueden no coincidir con el entorno; y ejecutar un plan en cadena no debe interpretarse como permiso para ampliar su alcance. La aprobación del usuario debe quedar ligada a una versión concreta de spec y plan.

### 2.2 Ponytail

Catálogo: ponytail, ponytail-audit, ponytail-debt, ponytail-gain, ponytail-help y ponytail-review.

**Aporta:** minimizar código y complejidad sin eliminar requisitos, validación, seguridad ni manejo de errores; inspeccionar llamadores y el alcance real antes de editar; revisión que pregunta si el cambio es correcto, seguro y mínimo; y registro explícito de atajos/deuda. Tiene niveles de intensidad.

**Lo valioso para Loomcoding:** encaja como política transversal del implementador y revisor, no como otro proceso de planificación. Puede ayudar a reducir diffs y dependencias, siempre subordinado a requisitos, contratos y normas de arquitectura del repositorio.

**Evidencia del autor:** el repositorio publica una comparación propia de 39 tareas con cinco ejecuciones por tarea, bajo un modelo y harness concretos. Informa reducciones en líneas, tokens, coste y tiempo, con resultados cercanos en sus verificaciones. Es evidencia interesante, pero autofinanciada/publicada por el autor, no una replicación independiente ni una predicción de resultados en Loomcoding. La mezcla de tareas, verificaciones ocultas y evaluación por modelo limita la generalización. Véase [el benchmark publicado](https://github.com/DietrichGebert/ponytail/blob/main/benchmarks/results/2026-10-07-agentic.md).

### 2.3 Matt Pocock Skills

38 skills organizadas en ingeniería, productividad, en progreso y miscelánea. Los flujos de ingeniería incluyen investigación, definición de spec, implementación de spec o ticket, TDD, revisión, triage, modelado de dominio, mejora de arquitectura y prototipos. Productividad incluye entrevistas tipo grill, enseñanza, handoff y documentación para agentes.

**Aporta:** habilidades pequeñas que se pueden invocar por separado; extracción de glosarios, decisiones y modelos de dominio; descomposición en tickets verticales y dependencias; implementación por ticket con TDD, revisión y entrega; y varias formas de entrevistar al usuario según la fase.

**Lo valioso para Loomcoding:** separar “definir qué significa el dominio” de “describir tareas”; producir tareas verticales verificables; hacer que el implementador trabaje desde un ticket acotado; y guardar decisiones que eviten volver a discutir el vocabulario en cada sesión.

**Costes y tensiones:** algunos flujos esperan tracker externo, etiquetas, ramas, worktrees, commits o un agente merger. No deben asumirse como servicios disponibles en Loomcoding. `grill-me`, brainstorming y otras entrevistas se pisan; se debe escoger una técnica y un dueño de la entrevista. Los flujos de implementación autónoma requieren los mismos límites de alcance y permisos que los de Superpowers.

### 2.4 Addy Osmani Agent Skills

25 skills que cubren definición, planificación, construcción, verificación, revisión y envío; incluyen especificación, planificación, implementación incremental, TDD, revisión, simplificación, seguridad, rendimiento, documentación, CI/CD y contexto.

**Aporta:** un mapa claro del ciclo completo; implementación en rebanadas verticales verificadas; restricciones de calidad medibles; revisión escéptica con contexto fresco; uso de fuentes primarias; y un marco explícito para probar si las skills se activan y se comportan como se espera.

**Lo valioso para Loomcoding:** separar pruebas de activación/ruteo de pruebas de calidad del resultado; fijar límites de contexto y criterios de aceptación; revisar decisiones dudosas mientras aún se pueden corregir; y no dar por buena una skill porque su archivo parezca razonable.

**Costes y tensiones:** su modo de construcción automática puede ejecutar un plan aprobado de forma extensa, así que necesita gates para alcance nuevo, fallos repetidos y acciones riesgosas. Algunas skills dependen de herramientas externas (por ejemplo, Chrome DevTools MCP). Las evaluaciones léxicas de activación son aproximadas; las evaluaciones conductuales pueden ser estocásticas y costosas. Los resultados de evaluación del repositorio no equivalen a una garantía con el runtime de Loomcoding. [Evaluaciones publicadas](https://github.com/addyosmani/agent-skills/tree/main/evals).

### 2.5 Agency Agents

Biblioteca de 305 perfiles distribuidos en áreas como ingeniería, seguridad, testing, producto, diseño, marketing, ventas, finanzas y sectores especializados. Un perfil expresa identidad, misión, workflows y entregables; no constituye por sí mismo una skill ejecutable o un permiso del runtime.

**Aporta:** ejemplos de especialización, vocabulario y listas de comprobación por rol; perfiles de revisión, pruebas, documentación, arquitectura de sistemas multiagente y cambios mínimos; y patrones para definir resultados esperados de un agente.

**Lo valioso para Loomcoding:** usar el formato de perfil como material para diseñar agentes especialistas bajo control del padre. Los perfiles de reviewer, evidence collector, technical writer, Rust refactoring specialist y multi-agent architect son candidatos para evaluación selectiva.

**Riesgos:** los perfiles son de calidad y alcance desiguales; algunos son muy largos, imponen herramientas/rutas/frameworks que Loomcoding no tiene, o incorporan preferencias personales como si fueran reglas universales. Un perfil de orquestador puede ejecutar reintentos automáticos y bucles de PM→arquitecto→implementador→QA, lo cual puede chocar con aprobación humana y límites de iteración. La personalidad no acredita experiencia real ni debe conceder herramientas. Código, mensajes externos, shell, red y escritura deben estar definidos por las políticas del runtime, no por texto de perfil.

## 3. Comparación por capacidad

| Capacidad | Superpowers | Ponytail | Matt Pocock | Addy Osmani | Agency Agents |
|---|---|---|---|---|---|
| Descubrimiento/entrevista | Fuerte, formal | No es su centro | Fuerte, modular | Incluye ideación/entrevista | Depende del rol |
| Spec y plan | Flujo prescriptivo | Reduce alcance | Spec, tickets, dominio | Skills específicas para ambos | Algunos perfiles lo describen |
| Implementación | Delegada por tareas | Regla de tamaño | Implementación por spec/ticket | Incremental/autónoma tras aprobación | Depende del perfil |
| TDD/verificación | Muy prescriptivo | Check pequeño y evidencia | TDD por flujo | TDD + evaluación | Roles de testing/evidence |
| Revisión | Skill antes/después de tareas | Revisión minimalista | Revisión paralela | Revisión por ejes + duda | Perfiles de reviewer |
| Multiagente | Proceso central | No orquesta | Flujos que lo presuponen | Automatización por fases | Muchos perfiles, algunos orquestan |
| Simplicidad/alcance | Por plan y tarea | Núcleo | Flujos cortos, no siempre política | Simplificación dedicada | Existe perfil, calidad variable |
| Evaluación del propio catálogo | Skill authoring/TDD | Benchmark del autor | Menos central | Más explícita | No es el centro |

## 4. Solapamientos y conflictos

### Solapamientos que conviene resolver con una sola autoridad

1. **Entrevista y descubrimiento:** Superpowers brainstorming, Matt grill/grill-with-docs y Addy interview/idea refine pueden preguntarle lo mismo al usuario. Loomcoding debe tener un solo agente padre entrevistador; una skill de apoyo no inicia otra entrevista si ya hay respuestas suficientes.
2. **Spec, plan y tickets:** Superpowers produce diseño y plan; Matt separa spec, dominio y tickets; Addy tiene spec y planificación. Loomcoding debe definir sus propios artefactos canónicos y permitir que un paso genere los tres relacionados, sin mantener versiones competidoras.
3. **TDD y pruebas:** los tres sistemas de proceso lo recomiendan con distinta rigidez. La política del repo y los criterios aprobados determinan qué se prueba; no ejecutar suites o comandos extra por una skill si el repo no lo permite.
4. **Revisión:** Superpowers, Matt, Addy y Agency contienen revisores. Usar varios revisores idénticos puede duplicar ruido. Un reviewer independiente con rubric común y evidencia basta; llamar un especialista solo cuando el riesgo lo justifique.
5. **Simplicidad:** Ponytail, Addy code-simplification y Agency minimal-change engineer comparten intención, pero pueden discrepar entre mínimo diff literal y salud general del diseño. La precedencia correcta es: requisitos/seguridad/contratos del repo; luego solución más simple completa.
6. **Orquestación automática:** Superpowers subagent-driven development, Matt implement-spec y Addy build auto automatizan tareas después de una aprobación. Sus políticas de reintento, merge, commit y parada no son idénticas; no deben mezclarse sin un único controlador.

### Conflictos previsibles

- **Autonomía vs. consentimiento:** un plan aprobado permite ejecutar ese plan, no modificar objetivos, ampliar archivos sensibles, publicar, enviar mensajes o desplegar. Los bloqueos y el cambio de alcance vuelven al padre.
- **Ejecución continua vs. checkpoints:** puede omitirse confirmación tarea por tarea solo si el usuario autorizó la ejecución del plan exacto y las herramientas permitidas son claras. Fallos, hallazgos de seguridad, ambigüedad material o intentos agotados detienen la cadena.
- **TDD rígido vs. política del repositorio:** aplicar el método donde haya seams y pruebas útiles; preservar los comandos y restricciones explícitos de Loomcoding. La skill no redefine arquitectura ni añade pruebas prohibidas.
- **Perfiles de Agency vs. herramientas:** un personaje puede recomendar una acción, pero el motor decide permisos. No inferir acceso a shell, navegador, red, escritura, git remoto o secretos.
- **Atajos/reintentos:** perfiles que prescriben muchos pasos o reintentos ilimitados deben reducirse a un límite configurable del orquestador. Un máximo de dos ciclos implementador→revisión puede ser una base conservadora para el flujo propuesto; el padre reporta los fallos persistentes al usuario.

## 5. Combinaciones recomendadas

| Combinación | Qué gana Loomcoding | Condición |
|---|---|---|
| Superpowers + Ponytail | Proceso con fases y delegación, cambio acotado | Ponytail como regla del implementador/reviewer, no otro workflow |
| Superpowers + Matt | Flujo de ejecución más modelo de dominio/tickets verticales | Un solo formato de spec/plan/tarea y sin asumir tracker externo |
| Addy + Ponytail | Evaluación de skills más control del tamaño del cambio | Métricas locales; no trasladar claims del benchmark |
| Matt + Addy | Artefactos de dominio/tareas más evaluación y aceptación | El padre consolida outputs y elimina duplicados |
| Agency + cualquiera | Perfiles especialistas como lentes concretas | Perfil adaptado, permisos externos al perfil, activación justificada |
| Los cinco | Cobertura amplia de patrones | No cargar todo simultáneamente; sintetizar un workflow Loomcoding y seleccionar capacidades por fase |

La combinación completa no debe ser una concatenación de instrucciones. El coste probable es contexto mayor, activaciones contradictorias y controles redundantes. Debe construirse una política propia a partir de patrones, luego probar ablations.

## 6. Propuesta de flujo nativo para Loomcoding

1. **Padre/clasificador:** identifica alcance, riesgo y si hay información suficiente; solicita aclaraciones solo cuando cambien materialmente la solución.
2. **Planificador:** redacta spec y plan en los formatos del repo; define criterios de aceptación, archivos probables, comandos permitidos, riesgos y límites. No implementa.
3. **Gate humano:** el usuario aprueba el contenido/versiones concretos de spec y plan. Esta autorización habilita la ejecución descrita, no cambios fuera de ella.
4. **Implementador:** recibe una tarea vertical acotada y aplica Ponytail junto con reglas del repo; trabaja con pruebas/checks definidos por el plan.
5. **Revisor independiente:** comprueba diff, criterios, regresiones, seguridad y evidencia. No modifica el código directamente salvo que se le asigne explícitamente ese rol.
6. **Padre:** acepta, manda al mismo implementador una corrección concreta dentro del plan, o detiene y escala. Mantiene el estado y el contador de intentos.
7. **Cierre/verificación:** comandos exigidos por el repositorio y resultados observables; el padre informa qué se ejecutó, qué pasó y cualquier límite. La aceptación final sigue siendo humana si el usuario la requiere.

El ciclo de corrección debe ser finito (propuesta inicial: dos rondas). Un cambio de alcance requiere aprobación actualizada. Los roles describen objetivos; capacidades efectivas se configuran aparte con permisos explícitos y límites de contexto, tiempo y herramientas.

## 7. Plan para validar en Loomcoding

Antes de adoptar skills externas, evaluar un prototipo pequeño con idéntico modelo, herramientas, repositorio, presupuesto y prompts base. Casos: solicitud trivial; bug que afecta llamadores; cambio multi-crate que requiere spec/plan; hallazgo de revisión verdadero y falso; entrada riesgosa; y caso ambiguo. Mantener red y efectos externos deshabilitados.

Comparar: baseline sin skill; cada fuente por separado; pares complementarios; y workflow sintetizado. Medir activación correcta y falsos disparos, cobertura de criterios/artifacts, defectos y checks, precisión/recall de review, rondas, tokens/coste/tiempo, violaciones de herramienta, cambios fuera del alcance y afirmaciones de verificación sin evidencia. Repetir casos estocásticos y reportar dispersión. Separar calidad de resultado de costo y latencia.

Para Agency, seleccionar un conjunto pequeño de perfiles directamente pertinentes y medir primero su utilidad frente a un rol genérico. No cargar los 305 perfiles en el prompt. No convertir estrellas, tamaño del catálogo o una pasada de benchmark del autor en evidencia de calidad para Loomcoding.

## 8. Decisión recomendada

- **Adoptar como patrones:** fases, gates y delegación acotada de Superpowers; simplicidad de Ponytail; modelado de dominio y tareas verticales de Matt; evaluación de activación/conducta de Addy.
- **Usar como biblioteca selectiva:** Agency Agents, adaptando pocos perfiles tras revisión.
- **No adoptar sin adaptación:** comandos/skills completos que asumen Claude Code, tracker externo, worktrees, commits automáticos, MCP específico, rutas o frameworks distintos.
- **Construir una fuente de verdad propia:** un workflow Loomcoding y un rubric de revisión, con precedencia de specs y restricciones del repositorio sobre instrucciones de skills.
- **Siguiente paso:** completar la especificación de Agent-first con este estudio y definir un benchmark local de activación y flujo antes de implementar el runtime.

## Apéndice A. Catálogo de skills de ingeniería revisadas

### Superpowers (15)

`brainstorming`, `diagnosing-superpowers`, `dispatching-parallel-agents`, `executing-plans`, `finishing-a-development-branch`, `receiving-code-review`, `requesting-code-review`, `subagent-driven-development`, `systematic-debugging`, `test-driven-development`, `using-git-worktrees`, `using-superpowers`, `verification-before-completion`, `writing-plans`, `writing-skills`.

### Matt Pocock (38)

**Engineering (20):** `ask-matt`, `code-review`, `codebase-design`, `diagnosing-bugs`, `domain-modeling`, `grill-with-docs`, `implement-spec`, `implement`, `improve-codebase-architecture`, `pr`, `prototype`, `research`, `retro`, `setup-matt-pocock-skills`, `tdd`, `to-spec`, `to-tickets`, `triage`, `wayfinder`, `wizard`.  
**In progress (7):** `chief-of-staff`, `claude-handoff`, `loop-me`, `setup-ts-deep-modules`, `writing-beats`, `writing-fragments`, `writing-shape`.  
**Misc (4):** `git-guardrails-claude-code`, `migrate-to-shoehorn`, `scaffold-exercises`, `setup-pre-commit`.  
**Productivity (7):** `grill-me`, `grilling`, `handoff`, `teach`, `to-questionnaire`, `wait-what`, `writing-for-agents`.

### Addy Osmani (25)

`api-and-interface-design`, `browser-testing-with-devtools`, `ci-cd-and-automation`, `code-review-and-quality`, `code-simplification`, `constraint-driven-development`, `context-engineering`, `debugging-and-error-recovery`, `deprecation-and-migration`, `documentation-and-adrs`, `doubt-driven-development`, `frontend-ui-engineering`, `git-workflow-and-versioning`, `idea-refine`, `incremental-implementation`, `interview-me`, `observability-and-instrumentation`, `performance-optimization`, `planning-and-task-breakdown`, `security-and-hardening`, `shipping-and-launch`, `source-driven-development`, `spec-driven-development`, `test-driven-development`, `using-agent-skills`.

### Ponytail (6)

`ponytail`, `ponytail-audit`, `ponytail-debt`, `ponytail-gain`, `ponytail-help`, `ponytail-review`.

### Agency Agents

305 perfiles Markdown; el catálogo completo se organiza en: academic (6), design (10), engineering (66), finance (5), game-development (21), gis (13), healthcare (3), integrations (1), marketing (37), paid-media (7), product (6), project-management (7), research (1), sales (9), security (12), spatial-computing (6), specialized (59), support (6) y testing (9).

## Apéndice B. Referencias de evaluación

- Repositorios y contenidos oficiales enlazados en el inventario.
- [Ponytail: benchmark agentic del autor](https://github.com/DietrichGebert/ponytail/blob/main/benchmarks/results/2026-10-07-agentic.md).
- [Addy Osmani: evals de skills](https://github.com/addyosmani/agent-skills/tree/main/evals).
- Las cifras de estrellas/metadatos registradas en la investigación se obtuvieron de los repositorios oficiales el 25-10-2026 y son datos mutables; no se usan para puntuar calidad.
