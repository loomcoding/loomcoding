# Plan — Entrada A2A compartida del runtime

> Estado: implementado y verificado el 2026-10-27.
>
> Spec: `docs/specs/2026-10-27-codeloom-runtime-a2a-directory.md`.

1. Leer `[a2a]` de `~/.codeloom/runtime.toml`, independiente de proyectos; sin rol `serve`/`both` no se abre listener.
2. Añadir endpoint directo de tarjeta por tenant y alimentar tarjetas desde el registro vivo.
3. Mantener autenticación, validar el tenant A2A y enrutar por el tenant seleccionado.
4. Las tarjetas runtime viven mientras vive el registro de agentes; no se añade método de listado.
5. `cargo test --target-dir .\target -p codeloom-a2a` pasa: 246 aprobadas, 10 ignoradas.
