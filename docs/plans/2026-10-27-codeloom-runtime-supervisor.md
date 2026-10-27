# Plan — Agente supervisor del runtime

> Estado: implementado y verificado el 2026-10-27.
>
> Spec: `docs/specs/2026-10-27-codeloom-runtime-supervisor.md`.

1. Configurar provider/modelo en `~/.codeloom/runtime.toml`; resolver la credencial solo desde el entorno del usuario.
2. Guardar mensajes y consumo en una sesión propia del supervisor.
3. Mantener una instantánea determinista acotada; el LLM solo recibe solicitudes explícitas y carece de herramientas.
4. Aplicar timeout de 15 s y fallback clásico; la aprobación humana sigue en la TUI destino.
5. Tests mock sin red cubren config ausente, éxito con uso, error del provider y timeout.
