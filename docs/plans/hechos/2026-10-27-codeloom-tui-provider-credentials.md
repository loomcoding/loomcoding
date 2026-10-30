# Plan: configuración inicial de provider y API key en la TUI

Spec: `docs/specs/hechos/2026-10-27-codeloom-tui-provider-credentials.md`.

Estado: implementado y compilado.

- [x] Crear la configuración de proyecto predeterminada cuando falte.
- [x] Leer/guardar credenciales de Windows Credential Manager; mantener el
  entorno y `.env` como fallbacks existentes.
- [x] Abrir la TUI sin clave y mostrar cómo llegar a `/settings`; los prompts
  fallan localmente hasta configurar un provider.
- [x] Campo de clave enmascarado, envío por IPC local y aplicación inmediata.
- [x] `cargo check -p codeloom-cli` y build release de `loom` correctos.
- [x] Copiar el binario al laboratorio y verificar `--version` y SHA-256.
- [ ] `cargo fmt --all --check` conserva dos avisos de formato previos en
  `crates/codeloom-tui/src/render.rs` (1490 y 1583).
- [ ] No se llamó a OpenRouter ni se ejecutaron suites de pruebas.
