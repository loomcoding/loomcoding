# Plan — Documentar CodeLoom agent-first y evaluar skills de desarrollo

> Estado: marco y estudio comparativo documentados; evaluación en runtime pendiente.
> Spec: `docs/specs/2026-10-25-codeloom-agent-first-development-skills.md`.

## Alcance

Crear un punto canónico en la arquitectura para el principio agent-first y las
skills de desarrollo que lo hacen operable. Registrar candidatos y método de
evaluación; no instalar, copiar ni implementar skills en esta tarea.

## Trabajo realizado

1. Separar perfiles de agente, skills, coordinador y aprobaciones del usuario.
2. Enlazar el flujo con aprobación de spec/plan, implementación, revisión y
   cierre del usuario.
3. Registrar proyectos externos, estrellas aproximadas fechadas y contribución
   potencial sin asumir que popularidad valida calidad.
4. Añadir el punto de entrada desde el mapa de arquitectura y enlazar la spec y
   el plan de workflow ya existentes.
5. Comparar los cinco catálogos solicitados y documentar aportes, redundancias,
   conflictos, límites de evidencia y combinación recomendada en
   `docs/references/2026-10-25-agent-development-skills-study.md`.

## Próxima evaluación

Prototipar una evaluación local controlada de activación y calidad, siguiendo
el diseño del estudio. Seleccionar perfiles concretos de Agency Agents para
lectura adicional antes de considerar su adaptación. No instalar ni adoptar
skills externas como workflow nativo sin pasar esa evaluación.

## Criterios de aceptación

- El mapa de arquitectura declara explícitamente el principio agent-first y
  enlaza el marco dedicado.
- El marco diferencia agentes, skills, coordinación y aprobación humana.
- Los candidatos están fechados y clasificados por su utilidad potencial.
- Queda claro que no están adoptados y que las estrellas no sustituyen una
  evaluación.
- El informe comparativo explicita la cobertura: catálogo completo de Agency,
  con lectura profunda selectiva y esa limitación documentada.
- No se cambia código, configuración de agentes ni dependencias.
