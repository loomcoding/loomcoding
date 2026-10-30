# Plan — Aislamiento y ciclo de vida del runtime

> Estado: implementado y verificado en Windows el 2026-10-27.
>
> Spec: `docs/specs/hechos/2026-10-27-codeloom-runtime-isolation-lifecycle.md`.

1. El singleton de Windows reserva la named pipe con `first_pipe_instance` antes de abrir SQLite; la prueba confirma que una segunda instancia no puede adquirirla.
2. Configuración y `.env` se resuelven por agente sin mutar el entorno global; tests cubren la separación de proyectos y sesiones.
3. El handshake valida token y versión; attach valida ID y cwd. La desconexión conserva el trabajo activo, y los permisos pendientes se deniegan al perder la TUI.
4. La interfaz A2A runtime se configura en el perfil de usuario, separada del A2A consume por proyecto; credenciales inválidas se rechazan antes de crear el listener.
5. En Windows pasaron 1648 tests (13 ignorados), build, fmt, clippy y `git diff --check`. El test `cfg(unix)` de carrera del socket está implementado, pero requiere CI Unix para ejecutarse.
