# CodeLoom — Principio agent-first y skills de desarrollo

> Estado: **marco de producto y evaluación inicial**. Los candidatos no están
> adoptados; evaluar antes de implementar o incorporar instrucciones.
>
> Fecha: 2026-10-25
>
> Relacionado: `2026-10-14-loom-architecture-overview.md` (§5.2),
> `2026-10-14-loom-skills-ponytail-superpowers.md`,
> `2026-10-25-codeloom-agent-development-workflow.md`.

## 1. Principio

CodeLoom es **agent-first**: sus capacidades centrales se organizan alrededor
de agentes con responsabilidad explícita y delegación observable. En desarrollo
de software, las skills de proceso son una capacidad central porque convierten
ese modelo en un flujo controlable y repetible.

Separación de conceptos:

- **Agente/perfil** define quién actúa, su objetivo, instrucciones y herramientas
  permitidas.
- **Skill** define un procedimiento reutilizable que uno o más agentes cargan
  para una tarea.
- **Padre/coordinador** mantiene estado y decisiones entre agentes; una skill no
  sustituye al runtime ni concede permisos.
- **Usuario** conserva la aprobación de artefactos, excepciones y cierre, según
  el flujo elegido.

## 2. Flujo de desarrollo objetivo

1. Un agente prepara spec y plan sin implementar.
2. El padre presenta ambos artefactos y espera aprobación explícita.
3. Un implementador trabaja sobre la versión aprobada.
4. Un revisor independiente compara petición, spec, plan, diff y evidencia.
5. Hallazgos vuelven al implementador en rondas acotadas; revisión limpia lleva
   al padre a presentar evidencia para aceptación final.

El contrato detallado está en
`2026-10-25-codeloom-agent-development-workflow.md`. Esta spec fija el principio
y cómo evaluar skills externas; no crea un segundo runtime de workflow.

## 3. Familias de skills

- **Proceso y disciplina**: descubrimiento de requisitos, especificación,
  planificación, implementación por tareas, TDD, depuración y verificación.
- **Alcance y estilo de ingeniería**: simplicidad, cambios quirúrgicos, YAGNI,
  evitar código no pedido sin degradar seguridad ni validación.
- **Revisión y calidad**: auditoría independiente con hallazgos accionables,
  criterios de aceptación y evidencia.
- **Contexto y orquestación**: delegación, límites de contexto, roles, herramientas
  y evaluación de sistemas multiagente.

Las familias pueden componerse por rol, pero no apilarse sin necesidad. La
instrucción del usuario y las restricciones del proyecto prevalecen; el revisor
mantiene independencia de las preferencias del implementador.

## 4. Candidatos públicos

Estrellas observadas en GitHub el 2026-10-25. Son una señal de adopción, no una
medida de eficacia ni una recomendación de instalar el conjunto completo.

| Repositorio | Estrellas aprox. | Aporte que merece evaluar | Relación con CodeLoom |
|---|---:|---|---|
| [obra/superpowers](https://github.com/obra/superpowers) | 305k | Flujo spec aprobada → plan → desarrollo dirigido por subagentes, TDD y revisión | Referencia principal de proceso; ya estudiado en la spec de skills |
| [mattpocock/skills](https://github.com/mattpocock/skills) | 303k | Skills composables para aclarar requisitos, spec, plan, implementación y review | Comparar cómo mantiene al usuario en control del proceso |
| [multica-ai/andrej-karpathy-skills](https://github.com/multica-ai/andrej-karpathy-skills) | 224k | Pensar antes de codificar, simplicidad, cambios quirúrgicos y criterios verificables | Comparar con Ponytail; alto riesgo de duplicar reglas |
| [msitarzewski/agency-agents](https://github.com/msitarzewski/agency-agents) | 164k | Perfiles especializados con personalidad, proceso y entregables | Referencia para perfiles de planificador, implementador y revisor; no reemplaza workflow |
| [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) | 176k | Solución mínima, revisión de código y auditoría de alcance | Gobernanza del implementador; ya usado como skill |
| [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills) | 110k | Ciclo definir, planificar, construir, verificar, revisar y entregar; incluye evaluación de skills | Comparación prioritaria por su ciclo explícito y puertas de aprobación |
| [muratcankoylan/Agent-Skills-for-Context-Engineering](https://github.com/muratcankoylan/Agent-Skills-for-Context-Engineering) | 18k | Ingeniería de contexto, arquitecturas multiagente y evaluación | Referencia técnica para CodeLoom como framework de agentes |

El estudio comparativo solicitado de Superpowers, Ponytail, Matt Pocock,
Addy Osmani y Agency Agents está en
`docs/references/2026-10-25-agent-development-skills-study.md`. Incluye
catálogos, aportes individuales, solapamientos, conflictos, combinaciones y
propuesta de validación local. La cobertura del inventario de Agency Agents es
completa; no se hizo lectura semántica individual de sus 305 perfiles.

Los conteos cambian. Las fuentes son los repositorios enlazados; se registran
como fotografía de fecha, no como métrica persistente del producto.

## 5. Cómo evaluar antes de adoptar

Para cada candidato relevante:

1. Leer las instrucciones fuente y su licencia; separar procedimiento de texto
   promocional o específico de otro harness.
2. Mapear cada regla a un rol CodeLoom y a una fase del flujo.
3. Detectar contradicciones con instrucciones de usuario, Ponytail, Superpowers,
   perfiles existentes y el decálogo de contexto/coste.
4. Medir contexto añadido y señalar herramientas, scripts o hooks requeridos.
5. Seleccionar habilidades concretas; no adoptar un paquete solo por estrellas.
6. Evaluar conducta con tareas comparables: cumplimiento de aprobación, calidad
   de spec/plan, bugs del diff, verificaciones reales, tokens, llamadas a agentes
   y rondas de corrección.

Una habilidad se adopta cuando aporta una mejora observable al flujo de CodeLoom
y conserva límites, evidencia y control humano. No se copian perfiles de
terceros como agentes aprobados sin revisión explícita.

## 6. Decisiones abiertas

- Si las skills se cargan manualmente por perfil, por configuración del padre o
  por un resolver automático.
- Cómo exponer aprobación y estado de fase en TUI/CLI.
- Qué conjunto mínimo de tareas constituye una evaluación representativa.
- Qué skills o fragmentos externos se adoptan, adaptan o se descartan.
