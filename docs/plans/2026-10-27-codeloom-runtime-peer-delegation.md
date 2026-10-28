# Plan — Delegación A2A entre proyectos del runtime

> Spec: `docs/specs/2026-10-27-codeloom-runtime-peer-delegation.md`.

1. Añadir endpoint de tarjeta remota directa a `fetch_card`, manteniendo intacta la resolución
   anterior de tarjetas raíz.
2. Añadir perfiles efímeros por proyecto/generación al registro existente; mantenerlos fuera del
   repositorio y excluir el agente de su propio registro.
3. Regenerar los perfiles cuando se abre un agente y refrescar tarjetas en los registros activos.
4. Reutilizar la fábrica A2A y aprobación existentes; resolver la credencial en Credential
   Manager o entorno, generándola en Credential Manager al activar serving sin secreto configurado.
5. Hacer que la TUI cliente refresque sus tarjetas locales al iniciar y al usar `/refresh-agents`.
6. Añadir pruebas de URL directa y perfiles; comprobar compilación del CLI sin acceso a red ni
   provider real.
7. Mostrar los peers con el nombre normalizado de su carpeta y desambiguar duplicados con un
   sufijo corto del tenant.

## Avance

- Implementados perfiles de peers por proyecto/generación, refresco servidor/TUI, credencial
  segura y tarjeta directa por tenant.
- Runtime activado con `[a2a] role = "both"` y auth Bearer; el token se genera y guarda en
  Credential Manager cuando arranca el daemon.
- Binario `loom.exe` compilado y copiado al proyecto animation; SHA-256 coincidente con la build.
- `cargo check -p codeloom-cli --target-dir .\target`: correcto. No ejecuté tests ni la prueba
  manual de dos TUI; el `fmt --check` global sigue señalando dos líneas previas en `render.rs`.
