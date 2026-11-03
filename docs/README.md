# Documentación de CodeLoom

Este es el índice de `docs/`. El [README del proyecto](../README.md) describe
el producto, el estado funcional y la puesta en marcha.

![CodeLoom teje señales de código](assets/codeloom-weave.gif)

Previsualización GIF capturada de la animación Canvas original.

[Animación Canvas independiente](assets/codeloom-self-weaving.html) (descarga
el archivo y ábrelo en el navegador; GitHub muestra el código HTML en vez de
ejecutarlo).

## Por dónde empezar

1. [Mapa de arquitectura](specs/2026-10-14-loom-architecture-overview.md): cómo
   se relacionan las piezas y dónde está su diseño.
2. [Diseño base](specs/2026-10-13-loom-design.md): límites del núcleo,
   concurrencia, turnos, herramientas y permisos.
3. [Decálogo de contexto y coste](specs/2026-10-22-codeloom-decalogo-de-contexto-y-coste.md):
   límites obligatorios al cambiar prompts y herramientas.
4. [Contribuir](../CONTRIBUTING.md): flujo de cambios y comprobaciones requeridas.

## Carpetas

- [`specs/`](specs/): intención, contratos y propuestas de diseño que siguen
  vivos: pendientes, en exploración o parcialmente implementados. Una spec no
  demuestra por sí sola que la funcionalidad esté implementada; consulta su
  estado y el plan relacionado.
- [`plans/`](plans/): alcance, fases y seguimiento de tareas aún abiertas o
  con algún cabo suelto.
- [`specs/hechos/`](specs/hechos/) y [`plans/hechos/`](plans/hechos/): lo ya
  desarrollado y verificado. Es historial, no una cola de trabajo.
- [`visual-e2e.md`](visual-e2e.md): cómo ver y probar la TUI con un PTY en `cargo test` o
  con `ttyd` y Chrome.
- [`INDEX.md`](INDEX.md): estado cotejado con el código de todos los
  specs y planes.
- [`references/`](references/): investigación y observaciones externas,
  incluidas las comparativas de UI y contexto.
- [`reference/`](reference/analisis-herdr-crush-dsh.md): análisis histórico de
  herdr, Crush y dsh. La carpeta singular se conserva porque ya tiene enlaces.
- [`status/`](status/2026-10-18-codeloom-functional-status.md): snapshots con
  fecha. Para el estado actual del producto, empieza por el README raíz.

## Temas actuales

- [Dashboard de capacidades runtime](specs/2026-10-29-codeloom-runtime-capabilities-dashboard.md)
  y [su plan](plans/2026-10-29-codeloom-runtime-capabilities-dashboard.md): mostrar
  estado real de MCP, LSP y skills junto al uso de tokens.
- [Widgets desplazables de capacidades](specs/2026-10-30-codeloom-scrollable-capability-widgets.md)
  y [su plan](plans/2026-10-30-codeloom-scrollable-capability-widgets.md): mostrar
  cada servidor MCP, el estado LSP y las skills bajo Activity con scroll independiente.
- [Coste informativo MCP por agente](specs/hechos/2026-10-30-codeloom-coste-informativo-mcp-por-agente.md)
  y [su plan](plans/hechos/2026-10-30-codeloom-coste-informativo-mcp-por-agente.md): estimar
  el coste repetido de las definiciones MCP en las peticiones, separado del total
  informado por el provider; la carga de skills queda pendiente.
- [Runtime local acotado de skills](specs/hechos/2026-10-30-codeloom-runtime-local-skills.md)
  y [su plan](plans/hechos/2026-10-30-codeloom-runtime-local-skills.md): cargar solo los
  `SKILL.md` permitidos por scope local, con límites y coste informativo.
- [Adjuntar imágenes desde el chat](specs/2026-10-29-codeloom-image-attachments.md)
  y [su plan](plans/2026-10-29-codeloom-image-attachments.md): pegar imágenes con Ctrl+V
  y envío multimodal de imágenes locales.

- [Roadmap A2A](plans/2026-10-22-codeloom-a2a-roadmap.md): índice de tareas y
  decisiones del protocolo.
- [Servidor local CodeLoom y clientes TUI](specs/2026-10-26-codeloom-local-server-client-runtime.md)
  y [su plan](plans/2026-10-26-codeloom-local-server-client-runtime.md): propuesta
  para que CodeLoom comparta su propio runtime entre proyectos; Herdr es opcional.
- [Flujo de desarrollo coordinado por agentes](specs/2026-10-25-codeloom-agent-development-workflow.md)
  y [su plan](plans/2026-10-25-codeloom-agent-development-workflow.md):
  implementación realizada; quedan tareas de cierre y verificación.
- [Investigación sobre agentes ECC y CodeLoom](specs/2026-10-31-codeloom-ecc-agent-workforce-research.md): catálogo de perfiles, carga por harness y papel de los hooks.
- [Fiabilidad interactiva del workflow de desarrollo](specs/2026-10-29-codeloom-development-workflow-interactive-reliability.md)
  y [su plan](plans/2026-10-29-codeloom-development-workflow-interactive-reliability.md):
  correcciones al turno del coordinador y al uso de herramientas del planner en Windows.
- [Uso de tokens preciso y supervisión en vivo de subagentes](specs/2026-10-29-codeloom-precision-usage-live-subagent-supervision.md)
  y [su plan](plans/2026-10-29-codeloom-precision-usage-live-subagent-supervision.md):
  propuesta para conteo confirmado/provisional durante streams, visibilidad del chat
  de los hijos y parada directa desde la TUI.
- [Error HTTP de Anthropic y resultado duplicado de subagente](specs/2026-10-29-codeloom-anthropic-400-subagent-result-duplication.md)
  y [su plan](plans/2026-10-29-codeloom-anthropic-400-subagent-result-duplication.md):
  diagnóstico de errores de API y presentación del resultado del hijo sin duplicarlo.
- [Respuesta final vacía de un subagente](specs/2026-10-29-codeloom-subagent-empty-final-response.md)
  y [su plan](plans/2026-10-29-codeloom-subagent-empty-final-response.md):
  evitar que un hijo sin respuesta final figure como completado.
- [Atribución de mensajes internos](specs/2026-10-29-codeloom-user-attribution-internal-messages.md)
  y [su plan](plans/2026-10-29-codeloom-user-attribution-internal-messages.md):
  evitar que tareas y avisos del sistema aparezcan como `You`.
- [Composer multilínea del chat](specs/2026-10-29-codeloom-multiline-chat-composer.md)
  y [su plan](plans/2026-10-29-codeloom-multiline-chat-composer.md): crecer hasta
  tres líneas y conservar el pegado multilínea dentro del prompt.
- [Marcador para pegados largos](specs/2026-10-29-codeloom-large-paste-placeholder.md)
  y [su plan](plans/2026-10-29-codeloom-large-paste-placeholder.md): colapsar a
  `[Pasted Content N chars]` en el composer y expandir el payload íntegro al enviar.
- [Aplicación de `scope.mcp` por agente](specs/2026-10-28-codeloom-mcp-agent-scope.md)
  y [su plan](plans/2026-10-28-codeloom-mcp-agent-scope.md): implementación stdio
  parcial; quedan pruebas y cierre de seguridad.
- [Llamadas programáticas de herramientas por provider](specs/2026-10-28-codeloom-programmatic-tool-calling.md)
  y [su plan de tareas](plans/2026-10-28-codeloom-programmatic-tool-calling.md), además de
  [referencias oficiales comparadas](references/2026-10-28-programmatic-tool-calling-providers.md):
  adapters separados para Anthropic, OpenAI Responses y Gemini, condicionados al contrato
  real de cada API.
- [Marco agent-first y evaluación de skills](specs/2026-10-25-codeloom-agent-first-development-skills.md)
  y [estudio comparativo](references/2026-10-25-agent-development-skills-study.md):
  documentación de evaluación; las skills candidatas no están adoptadas.
- [Decálogo agent-first (borrador) y diseño de evaluación](specs/2026-10-31-codeloom-decalogo-agent-first.md)
  y [fuentes externas con auditoría del flujo](references/2026-10-31-agent-first-fuentes-externas-y-auditoria.md):
  propuesta de diseño sin evidencia propia todavía.
- [Estudio de agentpatterns.ai aplicado a CodeLoom](references/2026-10-31-agentpatterns-estudio-codeloom.md):
  primera pasada (22 páginas de 1.926) con matriz patrón → código y borrador de modos; CC BY 4.0, resumen propio.
- [Patrones aplicados y ahorro de tokens (agentpatterns.ai)](references/2026-10-31-agentpatterns-patrones-aplicados-y-ahorro-de-tokens.md):
  registro de lo que CodeLoom ya sigue, lo que no mide (la caché) y orden de trabajo propuesto.
- [Candidatas a implementar (agentpatterns.ai)](references/2026-10-31-agentpatterns-candidatas-a-implementar.md):
  lista priorizada con criterios, descartadas con su motivo y orden propuesto; nada está adoptado.

## Cómo navegar y mantenerla

- Usa una spec para el contrato y su plan para el trabajo y el estado; enlázalos
  entre sí en vez de copiar el mismo contenido.
- Conserva las rutas existentes: varios documentos se enlazan desde el README,
  otras specs y planes.
- Las propuestas antiguas siguen siendo útiles como contexto. Comprueba su
  estado antes de tratarlas como trabajo pendiente o capacidad disponible.
