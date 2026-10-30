# Plan: E2E de agente padre y agente local registrado

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

**Spec:** `docs/specs/2026-10-27-codeloom-agent-e2e.md`

1. Extender `crates/codeloom-cli/tests/e2e.rs` con un perfil temporal `reviewer`,
   `AgentRegistry`, tool `subagent`, provider Wiremock, store SQLite temporal y
   espera acotada del estado terminal del hijo.
2. Afirmar que el padre anuncia el perfil disponible, lo invoca, y que el hijo
   termina con la respuesta mock bajo el nombre elegido.
3. Ejecutar `cargo test -p codeloom-cli --test e2e` y la suite exigida por
   `AGENTS.md` si el espacio de disco lo permite.
