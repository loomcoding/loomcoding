# Plan: expansión inicial de herramientas internas

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

**Spec:** `docs/specs/hechos/2026-10-27-codeloom-expanded-tools.md`

## Cambios

1. Añadir `ls`, respetando gitignore, excluyendo carpetas pesadas y acotando
   resultados.
2. Añadir `write` sin sobrescritura y con permiso antes de crear.
3. Añadir `multiedit` con validación completa previa, permiso y escritura
   exclusiva.
4. Añadir búsqueda web y lectura de páginas con validación de URL/destino,
   límites y timeout; probar mediante servidor HTTP de fixture.
5. Añadir cliente LSP stdio para Rust (`rust-analyzer`) con diagnósticos,
   definición, referencias y símbolos; no instalarlo automáticamente.
6. Registrar las herramientas en `build_project_tools`; respetar cambios
   existentes sin relación en el árbol de trabajo.

## Verificación

Tests unitarios de límites, rutas, permisos, atomicidad, parsing y protocolos
con fixtures locales. Al final ejecutar los cuatro comandos exigidos por
`AGENTS.md`: `cargo build`, `cargo test`, `cargo fmt --check` y
`cargo clippy -- -D warnings`.
