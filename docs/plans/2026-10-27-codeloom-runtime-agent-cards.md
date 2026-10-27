# Plan — Tarjetas A2A de los agentes del runtime

> Estado: implementado y verificado el 2026-10-27.
>
> Spec: `docs/specs/2026-10-27-codeloom-runtime-agent-cards.md`.

1. `AgentEntry` registra una AgentCard única por ejecución y mantiene `internal` fuera del protocolo.
2. El runtime publica la tarjeta directa solo cuando el listener A2A está activo y autenticado.
3. Attach repone la tarjeta y la desconexión conserva al agente vivo y sus tareas.
4. El supervisor lee un snapshot interno acotado; no se creó método JSON-RPC propio.
5. El host compartido resuelve `AgentInterface.tenant` y usa operaciones A2A existentes.
6. Los cuatro checks del workspace pasan; el test suite mock cubre fallback del supervisor y rutas de AgentCards.
