# Plan — Tareas A2A sobre agentes vivos

> Estado: implementado y verificado el 2026-10-27.
>
> Spec: `docs/specs/2026-10-27-codeloom-runtime-a2a-live-tasks.md`.

1. Implementar `TaskHost` del runtime sobre `AgentManager` y la cola existente de cada `AgentEntry`.
2. Asociar cada tarea a tenant, sesión aislada, cancelación y estados terminales; mantener consultas limitadas por tenant.
3. Conectar envío, consulta, cancelación, Subscribe y streaming a operaciones estándar A2A.
4. El worker conserva provider, presupuesto y permisos del agente; la aprobación sigue en la TUI, con concurrencia configurada (máximo 64), duración máxima de 4 h y resultados de 32 KiB.
5. Tests A2A con mock pasan y el workspace completo pasa: 1642 aprobadas, 13 ignoradas.
