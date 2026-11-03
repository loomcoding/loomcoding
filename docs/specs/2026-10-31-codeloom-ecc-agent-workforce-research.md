# CodeLoom — Investigación: catálogo de agentes y workflows de ECC

> Estado: propuesta de investigación. No autoriza cambios de implementación.
> Fecha: 2026-10-31.

Relacionada con:
- 2026-10-25-codeloom-agent-development-workflow.md
- 2026-10-15-loom-agent-profile-a2a.md
- 2026-10-22-codeloom-subagent-tree-design.md
- 2026-10-29-codeloom-development-workflow-interactive-reliability.md
- 2026-10-14-loom-memory.md
- Decálogo de contexto y coste

## 1. Objetivo

Investigar cómo ECC define, distribuye y activa sus agentes, workflows, skills y hooks, y decidir qué ideas pueden mejorar el catálogo de perfiles registrados de CodeLoom.

CodeLoom ya tiene subagentes, perfiles registrados y un workflow de desarrollo con planner, implementador y revisor. La investigación debe aprovechar esos mecanismos y aclarar cómo crear una tropa pequeña de perfiles útiles, descubribles, configurables, segura y medible.

El checkout de ECC consultado declara la versión 2.2.3 y un catálogo de 71 agentes, 314 skills y 98 comandos. Tratar esas cantidades como una fotografía del repositorio, no como un requisito de diseño ni como una medida de calidad.

## 2. Pregunta principal

¿Qué partes del catálogo y del modelo de distribución de ECC se pueden adaptar a CodeLoom, y qué mecanismos hacen falta para que sus perfiles sean descubribles, configurables, seguros y medibles en coste?

Preguntas secundarias:

1. ¿Dónde se definen los agentes en ECC y qué mecanismo hace que cada harness los descubra?
2. ¿Qué diferencia hay entre un rol de agente, una skill, un comando, una regla y un hook?
3. ¿Qué hacen los hooks en los ciclos de prompt, herramienta y sesión? ¿Cuáles bloquean, modifican contexto, persisten estado o solo notifican?
4. ¿Qué transforma cada adaptador o conversor y qué campos se pierden al exportar agentes entre harnesses?
5. ¿Cómo se relacionan esos campos con AgentProfile, AgentCard/A2A, launcher, skills y MCP de CodeLoom?
6. ¿Qué límites se imponen realmente en el runtime y cuáles solo se expresan como instrucciones?
7. ¿Qué coste generan definición, invocación, delegación, contexto aislado y persistencia? ¿Qué datos de memoria son instrucciones y cuáles son estado durable?
8. ¿Qué perfiles cubrirían huecos reales sin duplicar el workflow de desarrollo existente?
9. ¿Qué controles de AgentPatterns sirven como checklist para una auditoría estática del harness de CodeLoom?

## 3. Hipótesis iniciales para validar

Estas observaciones orientan la investigación; se deben confirmar con archivos y comportamiento del harness antes de tratarlas como conclusiones:

- ECC mantiene definiciones de agentes bajo agents/ y documentación de integración específica por harness. La carpeta fuente no implica que cada harness active los mismos agentes.
- Los hooks son una superficie separada del catálogo de roles. ECC documenta eventos de prompt, herramientas y ciclo de sesión para validaciones, automatización, contexto y persistencia; no son por sí mismos un selector universal de agentes.
- Las integraciones varían. La documentación del plugin de Codex cubre skills, MCP y hooks; los agentes TOML bajo .codex/agents/ son una configuración distinta de los agentes fuente de ECC.
- pi/core parece ser una selección curada para Pi, no una copia completa de ECC; confirmar sus límites y exclusiones en pi/core/README.md y pi/core/CURATION.md.
- unified-memory describe instrucciones para usar Memory Vault. Separar esa skill del runtime de memoria y de la persistencia de sesión.
- CodeLoom ya aplica su workflow de planner, implementador y revisor desde el registro existente. ECC debe aportar evidencia sobre catálogo, distribución, hooks o ahorro, sin reemplazar el workflow aprobado.
- El código de CodeLoom declara scope.tools en AgentProfile; verificar qué componente aplica ese alcance a las llamadas del runtime y qué queda solo como metadato.

Fuentes locales de ECC a revisar: .claude-plugin/PLUGIN_SCHEMA_NOTES.md, .claude-plugin/plugin.json, .codex-plugin/README.md, .codex-plugin/plugin.json, .codex/agents/, hooks/README.md, skills/unified-memory/SKILL.md, pi/core/README.md, pi/core/CURATION.md, scripts/agent-convert.js y scripts/lib/agent-ir.js.

## 4. Alcance

### Incluye

- Inventario de agentes, skills, comandos, reglas, hooks y formatos de integración de ECC.
- Comparación de al menos planner, implementer/generator, code reviewer, security reviewer, build resolver y un perfil de investigación.
- Trazado de definición, instalación, descubrimiento, selección, dispatch, ejecución, persistencia y revisión.
- Clasificación de hooks por evento y efecto: guardrail, automatización, persistencia, contexto, notificación o telemetría.
- Mapeo de ECC a AgentProfile, AgentCard/A2A, perfiles locales, launcher, tools, skills y MCP de CodeLoom.
- Análisis de coste y memoria: carga de instrucciones, invocaciones hijas, contexto, tokens, latencia, estado persistido y límites de confianza.
- Recomendación de un catálogo inicial pequeño y criterios para añadir, retirar o versionar perfiles.
- Uso selectivo de los checks de AgentPatterns para localizar brechas de enforcement y verificación en CodeLoom.

### Fuera de alcance

- Copiar en bloque los 71 agentes de ECC o importar sus prompts como política confiable.
- Cambiar A2A, el runtime de subagentes o el workflow aprobado de CodeLoom.
- Implementar hooks nuevos, carga automática de perfiles o una integración de ECC en el producto.
- Tratar una definición Markdown como garantía de permisos. Los límites efectivos deben verificarse en el launcher y el registro de herramientas.
- Presentar estimaciones de ahorro de tokens como resultados medidos.
- Instalar la colección completa de skills de AgentPatterns o aplicar automáticamente las correcciones propuestas por una auditoría.

## 5. Método

1. Inventariar agentes y superficies de ECC. Registrar nombre, responsabilidad, herramientas, modelo, metadatos, reglas asociadas y harness compatible.
2. Trazar cada integración desde manifest o catálogo hasta instalador/adaptador, archivo resultante y mecanismo de runtime.
3. Clasificar los hooks por evento, comando, datos leídos/escritos, efecto, política de fallo y si bloquean o solo informan.
4. Comparar una muestra de perfiles y registrar solapamientos, alcance de herramientas, tamaño de instrucciones y dependencia del harness.
5. Seguir los campos relevantes hasta CodeLoom: AgentProfile, AgentCard/A2A, perfiles locales, launcher, skills, MCP y workflow.
6. Separar los límites declarados de los controles que realmente se aplican. No inferir seguridad a partir de instrucciones.
7. Analizar coste por definición, invocación y delegación; identificar qué métricas existen y cuáles requerirían instrumentación. Distinguir coste de contexto de ahorro observado.
8. Analizar memoria por separado: instrucciones durables, memoria recuperable, estado de sesión, permisos y política de expiración.
9. Usar checks de auditoría pertinentes de AgentPatterns en modo de lectura. Pasar las rutas de CodeLoom explícitamente y registrar checks no aplicables.
10. Sintetizar correspondencias, incompatibilidades, riesgos y preguntas que necesitarían una prueba local acotada.

No ejecutar instaladores, hooks ni acciones de publicación de ECC durante la investigación. No usar credenciales reales. Cualquier prueba posterior con un harness debe ser local, acotada y separarse de esta spec.

## 6. Entregables

1. Inventario de agentes ECC: responsabilidad, herramientas, modelo, harness, formato y limitaciones de conversión.
2. Diagrama del ciclo de vida: definición → instalación/descubrimiento → selección/dispatch → ejecución → persistencia/revisión.
3. Matriz de hooks: evento → propósito → efecto en ejecución/contexto/estado → relación con agentes.
4. Mapa ECC → CodeLoom con correspondencias, incompatibilidades y evidencia de permisos efectivos.
5. Catálogo inicial recomendado para CodeLoom, justificando cada perfil por un hueco observado y evitando duplicar roles.
6. Análisis de coste y memoria con métricas observables, límites y datos que faltan.
7. Auditoría selectiva de controles del harness usando checks de AgentPatterns, con ruta, evidencia, severidad y recomendación.
8. Decisión recomendada: adoptar patrones, adaptar conceptos o no trasladar componentes; lista de incógnitas que requieren prueba.
9. Si la recomendación implica cambios de producto, crear una spec de implementación separada y un plan. Esta investigación no aprueba esa fase.

## 7. Criterios de cierre

- Se explica con evidencia cómo ECC define y distribuye agentes por cada harness estudiado.
- Se demuestra qué papel tienen los hooks y se distingue de la definición, selección y ejecución de agentes.
- Se separa el catálogo fuente de ECC de los agentes que cada integración hace disponibles.
- La comparación incluye las piezas actuales de CodeLoom y evita duplicar el workflow aprobado.
- La recomendación trata permisos efectivos, contexto, coste de delegación y límites de memoria.
- Cada afirmación técnica relevante enlaza a su archivo fuente o evidencia reproducible; las limitaciones quedan marcadas.
- La auditoría de AgentPatterns cita checks aplicados, rutas inspeccionadas y límites del método estático.
- El resultado no incluye cambios de código ni convierte recomendaciones en decisiones aprobadas.

## 8. Riesgos y decisiones abiertas

- Los formatos y capacidades de harness cambian; validar la versión concreta antes de concluir compatibilidad.
- Un catálogo grande puede aumentar selección y mantenimiento aunque no todas las instrucciones se inyecten en cada llamada.
- Delegar puede mejorar especialización, pero añade invocaciones, tokens y latencia; no presumir ahorro neto.
- Los hooks pueden ejecutar comandos y acceder a eventos; revisar cada efecto y comportamiento ante fallos.
- Las instrucciones y prompts externos requieren atribución, revisión de licencia y adaptación antes de reutilizarse.
- Decidir qué perfiles merecen mantenerse, dónde versionarlos y qué evidencia debe persistir por delegación.
- Definir cómo se comprobará que tools, MCP y otras capacidades de perfil están restringidas en el punto de ejecución.

## 9. Cruce preliminar con AgentPatterns

> Estado: evidencia inicial para orientar la investigación; no es una evaluación independiente de ECC ni una medición de calidad o coste.

### 9.1 Patrones que ayudan a leer ECC

La [Selection Map](https://www.agentpatterns.ai/patterns/selection-map/) compara 14 patrones mediante coste de tokens, latencia, dependencia de modelos avanzados, alcance de impacto, coste de verificación y clase de tarea. El sitio la presenta como ayuda comparativa, no como motor de recomendaciones. Sus etiquetas son relativas: no prueban el coste ni el resultado de ECC.

- **Alineación clara con Harness Engineering:** ECC incluye instrucciones, hooks y configuración del harness. Hay que comprobar qué restricciones aplica realmente el runtime; el texto de una skill no sustituye una política de permisos ni un control mecánico.
- **Plan → implementación → revisión y TDD:** se parece a Research/Plan/Implement y Review-Then-Implement. Reservar esos pasos para incertidumbre o rework costoso; tareas pequeñas y familiares pueden resolverse con menos ceremonia. La página de RPI también recoge QRSPI para trabajos de mayor ambigüedad.
- **Topología declarativa, alineación parcial:** ECC mantiene un catálogo de agentes, pero el catálogo por sí solo no demuestra que las transiciones, handoffs, gates ni permisos sean portables o se apliquen.
- **Riesgos por evaluar, no fallos demostrados:** una selección excesiva de agentes, la orquestación innecesaria y usar el número de agentes como métrica pueden añadir coste. Evaluar por resultado aceptado, correcciones, verificaciones, tokens y latencia.

[Engineering](https://agentpatterns.ai/engineering/), [Tools](https://agentpatterns.ai/tools/) y [Workflows](https://agentpatterns.ai/workflows/) son índices temáticos para localizar controles, herramientas por harness y flujos. No son evaluaciones comparativas de ECC.

### 9.2 Cómo funciona agentpatterns-ai-skills

El repositorio [agentpatterns-ai/skills](https://github.com/agentpatterns-ai/skills) es una colección portable de procedimientos para agentes, no un escáner único ni un plugin que supervise CodeLoom en ejecución.

1. El host descubre las skills por metadatos de SKILL.md, especialmente nombre y descripción, que ayudan a decidir cuándo invocarlas.
2. La skill seleccionada indica al agente qué superficies inspeccionar y cómo aplicar los checks. El agente carga referencias y plantillas auxiliares cuando el procedimiento las necesita.
3. El agente aplica esos pasos al repositorio o configuración que recibe y entrega hallazgos con ubicación, severidad, explicación, evidencia y recomendación.
4. El modo descrito es de auditoría estática y solo lectura: propone controles, no los instala ni corrige automáticamente. Algunos checks requieren juicio del modelo; el escaneo literal de secretos se describe como determinista.
5. Para una revisión general, el README propone una secuencia: exposición de secretos, lethal trifecta, seguridad del harness, sinks de supply chain, ubicación de reglas y hooks, gates de verificación, observabilidad y caché del prompt. Un hallazgo High que exija corrección detiene la secuencia. Auditorías de MCP, tools, memoria, orquestación multiagente, CI e instrucciones son facetas adicionales.

La skill audit-multi-agent-orchestration inspecciona la coordinación de un sistema que ya ejecuta varios agentes: handoffs tipados, superficies de estado compartido, independencia del verificador, posición del gate, límites de loops y fan-out. Su resultado usa checks identificables y referencias del corpus. También evita recomendar ceremonias pesadas para flujos simples y de bajo riesgo.

### 9.3 Aplicación a CodeLoom

Para cubrir CodeLoom hay que pasar sus rutas y el código que ejecuta la coordinación. La skill multi-agent indica como ruta por defecto .claude/agents/*.md; los perfiles de CodeLoom viven bajo .agents/agents. Incluir también el registro, launcher, handoffs A2A y workflow. No asumir que la skill encontrará esas superficies automáticamente.

AgentProfile expone scope.tools, pero el registro no lo aplica como allowlist general de herramientas. Tratarlo como una brecha entre intención y enforcement hasta demostrar qué componente restringe cada llamada; el texto del perfil no es una frontera de seguridad. Referencia: [profile.rs](../../crates/codeloom-core/src/registry/profile.rs).

El checkout local de agentpatterns-ai-skills tiene una diferencia de estructura que debe resolverse antes de automatizar instalaciones: el README describe skills agrupadas en directorios de categoría y da ejemplos con prefijos como audit/, mientras que el MANIFEST local enumera nombres sin esos prefijos y el árbol local tiene directorios de primer nivel. Fijar tag o commit y confirmar las rutas antes de instalar o sincronizar; al actualizar, comparar con MANIFEST para evitar dejar skills antiguas activables.

### 9.4 Uso propuesto en la investigación

Usar la colección como checklist selectivo para auditar CodeLoom, empezando por harness safety, verification gates y multi-agent orchestration. Añadir lethal trifecta, memory retrieval, MCP o tool definition cuando el mapa de datos y permisos indique que aplican. Ejecutar las instrucciones en modo de lectura, citando rutas y líneas; registrar checks no aplicables y el motivo. No instalar la colección completa ni tratar su informe como prueba dinámica de enforcement.

Fuentes de patrones: [Harness Engineering](https://www.agentpatterns.ai/patterns/agent-design/harness-engineering/), [Research-Plan-Implement](https://agentpatterns.ai/workflows/research-plan-implement/), [Review-Then-Implement](https://agentpatterns.ai/code-review/review-then-implement-loop/), [Prefer the simplest agent architecture](https://www.agentpatterns.ai/patterns/anti-patterns/prefer-simplest-agent-architecture/), [Agent sprawl](https://www.agentpatterns.ai/patterns/anti-patterns/agent-sprawl/) y [Agent headcount vanity metric](https://agentpatterns.ai/anti-patterns/agent-headcount-vanity-metric/).

### 9.5 Propuesta de integración: auditor bajo demanda

Mantener un perfil local de CodeLoom, por ejemplo agent-auditor, y tratar AgentPatterns como fuente de skills Markdown seleccionadas. No hace falta una dependencia de runtime en Rust: fijar el commit upstream y copiar al proyecto solo las carpetas de skills aprobadas, junto con su licencia y procedencia. Así las instrucciones quedan revisables, el agente no necesita red para ejecutarlas y una actualización upstream es un cambio explícito.

CodeLoom carga skills desde .agents/skills/<id>/SKILL.md y el perfil enumera los IDs en scope.skills. El loader agrega el contenido de las skills configuradas al prompt del subagente y registra su coste como componentes Skill ([skills.rs](../../crates/codeloom-core/src/skills.rs), [subagent.rs](../../crates/codeloom-core/src/subagent.rs)). Por ahorro y memoria, invocar el perfil solo cuando se pide una auditoría y empezar con un conjunto pequeño, por ejemplo audit-harness-safety y audit-multi-agent-orchestration. Añadir verification-gates u otras facetas cuando la investigación demuestre que hacen falta; no cargar la biblioteca completa en cada auditoría.

El informe debe ser estático, citar rutas y líneas, indicar los checks no aplicables y devolver recomendaciones sin modificar el proyecto. CodeLoom aún no aplica scope.tools como allowlist general ([profile.rs](../../crates/codeloom-core/src/registry/profile.rs)); por tanto, una instrucción de solo lectura no garantiza por sí misma aislamiento. Antes de auditar repositorios no confiables, restringir las herramientas efectivas en el dispatcher/runtime y verificar esa restricción. Mantener al auditor separado del workflow de implementación hasta que exista ese control.

### 9.6 agentpattern-ai-website como corpus de referencia

El checkout local es el espejo público del contenido de agentpatterns.ai, no un SDK ni un runtime de agentes. En la inspección estaba limpio en main, en el release v1.8.96, commit de63242d67e6599d66d59c4a3abbd4b84255ed15; contiene 1.930 archivos Markdown en esta fotografía. El repositorio publica contenido procedente de una fuente privada y no acepta pull requests. El sitio se declara agent-first y propone usar el corpus para auditorías de configuración/seguridad, buscar patrones por problema y fundamentar recomendaciones con referencias.

La relación con agentpatterns-ai/skills queda así: skills aporta el procedimiento y sus checks; website aporta conceptos, límites, evidencia y referencias para explicar por qué se reporta un hallazgo. El auditor debe citar primero evidencia del código/configuración de CodeLoom. Una página de patrones fundamenta el criterio, pero no demuestra que CodeLoom aplique un control.

Patrones candidatos para el perfil auditor:

- [Subagent vs In-Context Skill Execution](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/patterns/agent-design/subagent-vs-in-context-skill-execution.md): decide entre perfil hijo y skill dentro del contexto según contrato de entrada/salida, necesidad de aislamiento y presupuesto. Expone el intercambio entre menor contexto máximo y más tokens totales; estado emerging.
- [Compositional Skill Routing](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/context-engineering/compositional-skill-routing.md) y [Skill Disclosure Controller](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/patterns/agent-design/skill-disclosure-controller.md): elegir y cargar solo procedimientos pertinentes. Son candidatos para el coste de contexto; marcados emerging.
- [Demand-Driven Repository Auditing](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/verification/demand-driven-repo-auditing.md): investigar un flujo o superficie concreta a la vez mediante Initiator-Explorer-Validator y reutilizar resultados específicos de función cuando sigan vigentes; estado established. Es una guía para evitar ingerir todo el repositorio.
- [Context Quality Audit](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/context-engineering/context-quality-audit.md): auditar la calidad del contexto antes de atribuir fallos al modelo; estado emerging.
- [Harness Composition for Scaled Security Audits](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/security/security-audit-harness-composition.md): steering, scaling y stacking para controlar ruido. Adoptar primero verificación, deduplicación y triage; no añadir fan-out hasta medir que el flujo secuencial lo necesita.
- [Treating a Local Agent Session Trace as Audit Evidence](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/patterns/anti-patterns/local-trace-as-audit-evidence.md): no tratar el autoinforme o el trace modificable por el agente como única prueba; conservar rutas, líneas y una identidad del snapshot fuera del texto generado.
- [Tiered Memory Architecture](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/patterns/agent-design/tiered-memory-architecture.md): estudiar solo si aparecen sesiones largas y tareas recurrentes. El propio artículo marca overhead y recomienda no añadir tiers a sesiones cortas cuando un store simple basta.

El corpus tiene 1.930 archivos Markdown en el checkout analizado. No cargar llms-full.txt ni el repositorio completo en cada invocación. Mantenerlo como fuente consultable, buscar por tema y leer las pocas páginas relevantes para la petición. Registrar en cada referencia su estado/maturity y fecha de revisión; las páginas emerging o apoyadas en preprints son hipótesis de diseño, no resultados de CodeLoom.

La licencia local es CC BY 4.0. Si se copian o adaptan páginas, conservar atribución, enlace a licencia e indicar modificaciones; preferir referencias upstream cuando baste con consultar la fuente. Fuentes: [about.md](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/about.md), [README.md](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/README.md) y [LICENSE](https://github.com/agentpatterns-ai/website/blob/de63242d67e6599d66d59c4a3abbd4b84255ed15/LICENSE).

### 9.7 Recomendador de agentes y patrones con recuperación local

Es viable convertir el corpus en una fuente para un recomendador bajo demanda. Separar dos preguntas: qué patrón encaja con la tarea y qué perfil, skill o workflow disponible en CodeLoom puede implementarlo. El sitio de AgentPatterns describe patrones generales; no conoce el catálogo ni los permisos reales de CodeLoom. Mantener una tabla revisada de correspondencias patrón → perfil/skill CodeLoom con justificación y evidencia.

La [Selection Map](https://www.agentpatterns.ai/patterns/selection-map/) ayuda a comparar 14 patrones con ejes de tokens, latencia, dependencia de modelo avanzado, blast radius, coste de verificación y clase de tarea. El propio artículo dice que es una ayuda comparativa, no un motor de recomendación; advierte contra pattern shopping, filas desactualizadas y reducir ejes con distribuciones distintas a una sola nota. Sus etiquetas relativas no son datos medidos en CodeLoom.

Flujo candidato:

1. Resumir la petición y extraer restricciones: clase de tarea, riesgo, herramientas necesarias, presupuesto de tokens/latencia, aislamiento y verificación exigida.
2. Recuperar un conjunto pequeño de artículos por búsqueda textual o semántica, respetando categoría, estado, maturity y fecha de revisión.
3. Aplicar filtros y una rúbrica explícita a los candidatos; no ordenar solo por similitud del embedding.
4. Relacionar los patrones elegidos con perfiles registrados reales y sus capacidades efectivas.
5. Devolver dos o tres opciones con trade-offs, coste relativo, verificación, evidencia enlazada y una opción de abstención. Al principio el usuario o el workflow padre elige; no despachar automáticamente.

CodeLoom ya usa SQLite/SQLx para sesiones, pero la búsqueda vectorial no está implementada en codeloom-core. No acoplaría el índice del corpus al historial de sesiones inicialmente. Para una primera versión pequeña, indexar título, descripción, tags, categoría, maturity, status, last_reviewed, ruta, commit fuente y texto de secciones clave. Probar primero búsqueda léxica/híbrida con FTS5 y recuperar los pocos documentos seleccionados al contexto. Si esa búsqueda falla en tareas de evaluación por paráfrasis, añadir embeddings versionados por modelo y hash del contenido. Las tablas conceptuales serían pattern_source, pattern_document, pattern_chunk, profile_capability y pattern_profile_fit; no son un esquema aprobado.

SQLite publica FTS5 como módulo de búsqueda de texto completo ([documentación](https://www.sqlite.org/fts5.html)); comprobar que la build SQLite de CodeLoom lo habilita antes de depender de ello. Hay extensiones vectoriales como [sqlite-vec](https://github.com/asg017/sqlite-vec), pero su README indica estado pre-v1 y posibles cambios incompatibles. No añadirla hasta demostrar que FTS5 más filtros estructurados no basta y fijar una versión compatible con Windows y los builds de Loom.

No cargar los 1.930 documentos ni llms-full.txt por petición. Mantener un índice local del snapshot upstream y pasar al modelo solo los fragmentos relevantes; registrar modelo de embedding, versión del corpus, hash, fecha y procedencia. Si el embedding se genera mediante un proveedor remoto, enviar únicamente la consulta resumida y fragmentos permitidos, nunca secretos ni el repositorio completo.

Validar el recomendador con tareas representativas y una shortlist de referencia revisada por una persona: utilidad/precisión del top-k, abstenciones correctas, tokens, latencia y estabilidad ante cambios de redacción. El número de patrones encontrados no mide calidad, y el resultado no debe presentarse como ahorro probado hasta comparar contra una baseline textual.

### 9.8 ECC como catálogo de roles concretos

ECC puede ser una segunda fuente para recomendar agentes concretos. El checkout contiene 71 definiciones Markdown con metadatos como name, description, tools y model, además del cuerpo de instrucciones. scripts/lib/agent-ir.js las parsea a ecc.agent-ir.v1 y conserva el frontmatter original; el conversor actual declara Claude → Pi como dirección soportada. Referencias: [catálogo ECC](../../../ECC/agents/), [parser IR](../../../ECC/scripts/lib/agent-ir.js) y [conversor](../../../ECC/scripts/agent-convert.js).

El recomendador combinaría dos tipos de dato: AgentPatterns propone patrones y explica sus trade-offs; ECC ofrece candidatos de rol. Un índice no debe equiparar automáticamente patrón y agente. Mantener correspondencias revisables con evidencia, por ejemplo: petición de código Rust → perfil rust-reviewer para revisión; cambio de autenticación → security-reviewer; trabajo complejo → planner. La selección final debe respetar el workflow aprobado de CodeLoom y sus gates humanos.

No importar los 71 prompts al runtime. Indexar primero identificador, descripción, categoría inferida, modelo declarado, herramientas declaradas, harness de origen, versión y hash. Leer el cuerpo completo solo al adaptar un perfil elegido. Marcar cada campo como compatible, traducible con pérdida o no soportado en CodeLoom.

Los modelos Claude de ECC (haiku/sonnet/opus) y sus tools (Read/Write/Edit/Grep/Glob/Bash) no se deben copiar literalmente a CodeLoom. Mapearlos a capacidades reales de proveedor y herramientas efectivas. En particular, la lista scope.tools de CodeLoom todavía no actúa como allowlist general; no presentar un perfil recomendado como seguro hasta que el runtime aplique sus límites.


### 9.9 Jevalan como capa de decisión y límite de contexto

Tratar Jevalan como el componente que decide sobre una lista pequeña de candidatos; no como otro lector de todo el corpus ni como ejecutor. Su arquitectura declara «Jev juzga, el código actúa»: devuelve respuestas tipadas y deja los efectos al agente consumidor. Referencias locales: [arquitectura](../../../jevalan/docs/es/architecture.md), [flujo de enrutado de modelo](../../../jevalan/config/flows/model-route.yaml) y [motor de flujos](../../../jevalan/src/crates/jevalan-flow/src/run.rs).

Los Markdown de AgentPatterns y ECC son fuentes consultables. Sus enlaces a otras skills o páginas no deben disparar una carga recursiva: un enlace es procedencia, no una orden de invocación. Recuperar solo lo necesario para la decisión y cargar el procedimiento completo únicamente cuando el usuario decida ejecutar ese trabajo.

Flujo acotado propuesto:

1. CodeLoom entrega a Jevalan un resumen de tarea y restricciones tipadas: tipo de cambio, stack, riesgo, presupuesto, verificación necesaria y perfiles realmente disponibles.
2. La recuperación local devuelve como máximo tres patrones y tres perfiles con metadatos y fragmentos breves; registra revisión, commit y procedencia.
3. Jevalan decide entre recommend, ask_user o abstain y devuelve IDs, razón, confianza y restricciones. No recibe prompts completos, repositorios ni credenciales.
4. CodeLoom muestra la recomendación y mantiene sus aprobaciones existentes. No despacha un perfil por el mero resultado de similitud o de una llamada a Jevalan.

Presupuesto inicial para medir, no requisito definitivo: una llamada de decisión por petición; cero expansión recursiva de enlaces; hasta tres patrones, tres perfiles y tres fragmentos de evidencia. Si falta evidencia, abstenerse o pedir una precisión, no seguir cargando documentos. Registrar consumo y latencia en el modo sombra antes de permitir dispatch automático.

La capacidad debe tener esquemas de entrada y salida y salidas explícitas de baja confianza/error. Una política local puede convertir baja confianza en abstención o pregunta al usuario. El flujo model-route existente ilustra nodos score y noul con manejo de error/low-confidence; no es por sí mismo un recomendador de agentes ni una evaluación de calidad de recomendaciones.

Evaluar con peticiones representativas etiquetadas por una persona: calidad del top-k, abstenciones correctas, coste por recomendación y latencia. El acuerdo entre Jevalan y un modelo no es verdad de referencia. Mantener la selección humana hasta que una evaluación independiente justifique más autonomía.
