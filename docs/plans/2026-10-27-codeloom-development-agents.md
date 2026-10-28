# Plan: agentes locales para desarrollo

Spec: `docs/specs/2026-10-27-codeloom-development-agents.md`.

1. Añadir proveedor opcional al perfil local, validarlo y conservarlo fuera de
   `AgentCard`.
2. Resolver el proveedor del perfil al delegar, solo después de comprobar su
   aprobación; heredar el proveedor del padre cuando no se declara.
3. Crear tres perfiles con modelos OpenRouter gratuitos que acepten tools.
4. Comprobar selección, carga del perfil y herencia del proveedor con tests sin
   red real; ejecutar formato y tests de crates afectados.
5. Revisar `/agents` y aprobación manual al abrir la TUI; no autoaprobar.
