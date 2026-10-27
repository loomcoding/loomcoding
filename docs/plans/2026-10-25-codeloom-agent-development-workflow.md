# Plan — Flujo de desarrollo coordinado por agentes

> Estado: **listo para aprobación**, ligado a
> `docs/specs/2026-10-25-codeloom-agent-development-workflow.md`.
> No ejecutar hasta aprobar la spec.

## Alcance

Implementar un flujo mínimo de fases sobre el runtime de subagentes, perfiles,
aprobaciones y eventos ya existentes. No añadir un motor general de workflows.

## Fases

1. **Auditar contratos existentes**: verificar perfiles, delegación, `ask_user`,
   control de subagentes, estados durables y visualización en TUI. Identificar
   qué piezas de la spec ya están cubiertas y las brechas concretas.
2. **Diseñar contratos mínimos**: enlazar spec y plan con una huella del
   contenido aprobado; definir resultados de implementación/revisión y
   transiciones. Reutilizar `ask_user`; no aprobar con silencio, timeout ni
   respuesta ambigua. Limitar a dos rondas de corrección automática.
3. **Implementar planificación con pausa**: delegar especificación/planificación,
   presentar artefactos y requerir aprobación explícita antes de continuar.
4. **Implementar ejecución y revisión**: pasar artefactos aprobados al
   implementador; entregar diff y evidencia al revisor; representar hallazgos o
   revisión limpia sin cerrar automáticamente. El revisor queda limitado a
   lectura y comprobaciones; no modifica el diff.
5. **Implementar corrección acotada**: reenviar hallazgos al implementador,
   retener historial de revisión y volver al usuario tras dos rondas, ante
   bloqueo o cambio de alcance.
6. **Integrar estado y UI**: mostrar fase, agente responsable y decisión
   pendiente en la conversación existente; conservar los eventos necesarios
   para reconstrucción.
7. **Verificar**: cubrir transiciones, aprobaciones ausentes, fallos/cancelación,
   dos rondas de corrección, invalidación de aprobación y cierre final; ejecutar los cuatro checks del repo antes del
   commit: `cargo build`, `cargo test`, `cargo fmt --check`,
   `cargo clippy -- -D warnings`.

## Orden de trabajo

Cada fase debe dejar el repositorio compilable. No empezar la siguiente fase si
la anterior revela un cambio de arquitectura o una decisión abierta que afecte
al contrato. Revisar los perfiles locales y la guía de contexto antes de
introducir herramientas o texto nuevo en prompts.

## Riesgos a resolver en el diseño

- La aprobación debe ser una decisión inequívoca y quedar asociada a la versión
  exacta de spec/plan.
- El revisor necesita acceso al diff y a la evidencia sin recibir instrucciones
  sesgadas del implementador.
- El historial debe distinguir rondas de revisión para evitar aprobar un diff
  distinto del inspeccionado.
- Los flujos remotos A2A pueden no exponer los mismos controles que un hijo
  local; no asumir paridad hasta verificar el contrato.
