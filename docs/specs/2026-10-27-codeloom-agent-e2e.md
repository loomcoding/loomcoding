# E2E de agentes locales

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

## Objetivo

Comprobar el flujo integrado de un agente padre que delega en un agente local
registrado, ejecutando ambos bucles contra un provider HTTP mock y persistiendo
sesiones en SQLite temporal.

## Cubre

- El padre recibe del registro un perfil global local y lo invoca por nombre con
  la tool `subagent`.
- El hijo hereda el provider configurado, recibe el cuerpo del perfil y produce
  una respuesta final.
- El padre completa su turno; el estado, identidad y mensajes del hijo quedan
  persistidos.
- El provider mock responde según el contenido de cada request; no hay servicios
  externos ni credenciales reales.

## No cubre

Agentes remotos A2A, TUI interactiva, LSP/web, permisos humanos, ni calidad
semántica de un modelo real. El test prueba el cableado/runtime, no la calidad
de las respuestas generadas.
