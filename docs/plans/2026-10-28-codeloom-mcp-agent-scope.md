# Plan — Aplicación de `scope.mcp` por agente

> Estado: implementado; pruebas de protocolo local y checks completos pasados. Revisión visual pendiente.
>
> Spec: `docs/specs/2026-10-28-codeloom-mcp-agent-scope.md`.

## Objetivo

Aplicar la allowlist MCP del perfil de agente en el registro y la dispatch, conservando la
separación entre configuración de servidor, scope y aprobación de cada llamada.

## Decisiones aplicadas

1. Scope ausente/vacío significa cero servidores MCP.
2. Los perfiles solo referencian IDs; no pueden configurar ni iniciar servidores.
3. Filtrado al construir el registro y segunda comprobación al despachar.
4. El gate de permisos actual sigue aplicándose a cada tool-call MCP.
5. El agente remoto A2A conserva su política local; nunca se transmite `scope.mcp`.
6. Configurar servidores desde `~/.loomcoding/mcp.toml`; perfiles no inician procesos.
7. Solo transporte stdio en esta primera versión; no recarga MCP en caliente.
8. El agente raíz sin perfil no recibe MCP; las tools se entregan a agentes locales delegados.

## Fases

### 1. Cliente y tools stdio

- Añadir `loomcoding-mcp` con el SDK RMCP para ejecutar servidores stdio configurados por el
  usuario; limitar la configuración y fallar cerrado.
- Mantener tool-registry y dispatcher del core; no añadir transporte MCP al core.
- Namespacing estable por servidor y tool; cap de tools/esquemas y resultados.
- Comprobar cada llamada con el servicio de permisos normal.

### 2. Hacer cumplir el scope

- Aplicar `scope.mcp` antes de construir las definiciones que recibe el provider.
- Mantener identidad namespaced servidor/tool y rechazar colisiones inválidas.
- Construir el registro de cada subagente con tools integradas y solo MCP permitidos; las
  tools ausentes se rechazan por dispatch como desconocidas antes de contactar servidor.
- Pasar el scope del perfil al agente local sin filtrarlo al prompt o a datos A2A.

### 3. Estado y confianza

- Servidores definidos solo en `~/.loomcoding/mcp.toml`; entorno limpio, secretos referenciados
  por nombre de variable y ningún comando desde perfiles.
- Mostrar los scopes como servidores permitidos; enviar avisos locales a la TUI para IDs
  desconocidos y fallos de configuración/conexión, sin copiar errores crudos del proceso.
- La configuración MCP se recarga al reiniciar el runtime; scope de perfil se toma en cada
  delegación.

### 4. Verificación

- Añadir unit tests para intersección, defaults deny, colisiones y validación de dispatch.
- Añadir integración con servidor falso que demuestre que un tool fuera de scope no recibe
  una llamada.
- Probar perfiles cambiados y llamadas en curso; sin red real.
- Revisar ausencia de secretos y de scope en la tarjeta A2A, prompts y eventos.

## Archivos previstos

- `crates/loomcoding-core/src/registry/profile.rs`
- `crates/loomcoding-core/src/registry/source.rs`
- `crates/loomcoding-core/src/registry/` (lanzador/dispatcher, a determinar al iniciar)
- `crates/loomcoding-core/src/tools/` (adaptador MCP/registro, según la spec del runtime)
- `crates/loomcoding-mcp/` (cliente RMCP stdio y adapter de Tool)
- `crates/loomcoding-tui/src/app.rs` y `render.rs` (estado visible)
- pruebas unitarias e integración en los crates correspondientes
- docs MCP existentes si el descubrimiento requiere aclarar su ciclo de vida

Inspeccionar y preservar los cambios locales antes de editar archivos que ya estén
modificados.

## Fuera de este plan

- Cambiar el protocolo A2A o publicar metadata local.
- Implementar scopes para herramientas integradas o skills.
- Autoaprobar servidores, tools o perfiles de proyecto.
- Crear un MCP supervisor nuevo si la integración MCP existente ya provee uno.

## Resultado y límites

El scope se aplica a las tools MCP stdio disponibles en cada delegación local; la raíz y los
perfiles sin scope no reciben MCP. Cada tool pide el permiso normal. La integración local usa
un servidor RMCP falso sobre transporte en memoria para probar listado, invocación, scope,
permisos, desconexión y límites de salida; no lanza un proceso hijo MCP durante el test. La
configuración de servidores es global y estática en esta versión; HTTP, hot-reload y un panel
detallado de estado en la TUI quedan pendientes.

## Tareas para cerrar esta spec

- [x] Añadir tests base de configuración: fichero ausente, IDs y variables de entorno.
- [x] Añadir test de scope por defecto deny e intersección por servidor permitido.
- [x] Añadir servidor RMCP falso local para demostrar que solo se llama la tool permitida y
  que desconexiones/errores fallan cerrados; sin red real ni proceso hijo.
- [x] Probar límites de configuración y salida, default deny, gate de permisos y error de
  transporte sanitizado. El dispatch de tools no registradas conserva su test común en core.
- [x] Ejecutar los cuatro checks del contrato del repo; resultados anotados tras el cierre.

## Verificación 2026-10-28

`cargo build` correcto (aviso existente: `main.rs` comparte los bins `loom` y `loomcoding`);
`cargo test`: 1.712 pasaron y 13 ignorados; `cargo fmt --check` y
`cargo clippy -- -D warnings` correctos. Se usó el target temporal
`C:\Users\negrero\AppData\Local\Temp\loomcoding-final-target`.
