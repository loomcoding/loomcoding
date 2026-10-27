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

- [`specs/`](specs/): intención, contratos y propuestas de diseño. Una spec no
  demuestra por sí sola que la funcionalidad esté implementada; consulta su
  estado y el plan relacionado.
- [`plans/`](plans/): alcance, fases y seguimiento de tareas. Los planes
  completados son historial; no forman una cola única de trabajo pendiente.
- [`references/`](references/): investigación y observaciones externas,
  incluidas las comparativas de UI y contexto.
- [`reference/`](reference/analisis-herdr-crush-dsh.md): análisis histórico de
  herdr, Crush y dsh. La carpeta singular se conserva porque ya tiene enlaces.
- [`status/`](status/2026-10-18-codeloom-functional-status.md): snapshots con
  fecha. Para el estado actual del producto, empieza por el README raíz.

## Temas actuales

- [Roadmap A2A](plans/2026-10-22-codeloom-a2a-roadmap.md): índice de tareas y
  decisiones del protocolo.
- [Servidor local CodeLoom y clientes TUI](specs/2026-10-26-codeloom-local-server-client-runtime.md)
  y [su plan](plans/2026-10-26-codeloom-local-server-client-runtime.md): propuesta
  para que CodeLoom comparta su propio runtime entre proyectos; Herdr es opcional.
- [Flujo de desarrollo coordinado por agentes](specs/2026-10-25-codeloom-agent-development-workflow.md)
  y [su plan](plans/2026-10-25-codeloom-agent-development-workflow.md):
  propuesta lista para aprobación; todavía no describe comportamiento del
  runtime.
- [Marco agent-first y evaluación de skills](specs/2026-10-25-codeloom-agent-first-development-skills.md)
  y [estudio comparativo](references/2026-10-25-agent-development-skills-study.md):
  documentación de evaluación; las skills candidatas no están adoptadas.

## Cómo navegar y mantenerla

- Usa una spec para el contrato y su plan para el trabajo y el estado; enlázalos
  entre sí en vez de copiar el mismo contenido.
- Conserva las rutas existentes: varios documentos se enlazan desde el README,
  otras specs y planes.
- Las propuestas antiguas siguen siendo útiles como contexto. Comprueba su
  estado antes de tratarlas como trabajo pendiente o capacidad disponible.
