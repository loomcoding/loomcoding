# Plan — Editor de perfiles y tarjetas A2A

> Estado: implementado el 2026-10-28; pendiente de revisión final.
>
> Spec: `docs/specs/2026-10-28-codeloom-agent-card-editor.md`.

## Objetivo

Editar perfiles locales desde `/agents` con secciones A2A y Loomcoding claramente separadas,
ver las tarjetas reales en `/a2a` y registrar al supervisor como agente local disponible por
defecto. No mezclar configuración MCP/local con el protocolo A2A.

## Decisiones aplicadas

1. `name` sigue siendo el ID estable de Loomcoding y se proyecta como `AgentCard.name`; el
   editor muestra esa relación, sin mantener dos copias.
2. `scope.mcp` se inspecciona y edita en Loomcoding y ya filtra herramientas de servidores
   stdio configurados globalmente para agentes locales delegados. El gate de permisos normal
   sigue activo. `scope.skills` y `scope.tools` aún no se aplican.
3. Las capacidades A2A de transporte se derivan del runtime y son de solo lectura en el
   editor. Editar skills A2A no activa capacidades de transporte.
4. La ficha del supervisor se registra localmente siempre; su LLM continúa opcional y el
   handler existente conserva el fallback determinista.
5. Guardar mantiene claves YAML desconocidas y el cuerpo Markdown; se escribe de forma
   atómica y la recarga/huella existente gobierna actualización y confianza.

## Integración auditada

- `registry/profile.rs`: `AgentProfile` interpreta identidad y ejecución, pero actualmente
  `scope` y `a2a` (para perfiles locales) se declaran no aplicados; no existe modelo para
  skills/capacidades A2A editables.
- `registry/source.rs`: proyecta el perfil a `AgentCard`; `AgentEntry` ya separa tarjeta y
  ejecución local. Aquí debe vivir la proyección, sin filtrar persona/MCP/modelo.
- `loomcoding-tui/src/app.rs`, `run.rs`, `render.rs`: `/agents` ya lista y aprueba/revoca
  perfiles, pero solo muestra origen y prompt; sus eventos refrescan tarjetas remotas.
- `loomcoding-cli/src/server.rs`: el supervisor ya responde por tenant `supervisor`, mantiene
  fallback sin LLM y tiene configuración opcional. Falta incorporarlo a la vista/directorio
  de tarjetas locales según el modelo existente.
- `card.rs` ya modela `AgentSkill` y `AgentCapabilities` A2A; los booleanos opcionales
  distinguen no declarado de falso.

## Resultado

- `/agents` permite editar perfiles locales mediante secciones separadas A2A y Loomcoding;
  perfiles remotos y tarjetas integradas no se editan.
- El guardado valida el perfil, conserva el cuerpo Markdown y las claves desconocidas,
  comprueba la huella original y recarga el registro al finalizar.
- El directorio `/a2a` incluye el supervisor como `internal` y habilitado localmente; su
  tarjeta no anuncia interfaz ni capacidades no activas.
- `scope.mcp` filtra herramientas MCP stdio para agentes locales delegados; `scope.skills` y
  `scope.tools` siguen declarados y sin enforcement.

## Archivos previstos

- `crates/loomcoding-core/src/registry/profile.rs`
- `crates/loomcoding-core/src/registry/source.rs`
- `crates/loomcoding-core/src/card.rs` (solo si la proyección requiere soporte ya modelado)
- `crates/loomcoding-tui/src/app.rs`
- `crates/loomcoding-tui/src/run.rs`
- `crates/loomcoding-tui/src/render.rs`
- `crates/loomcoding-cli/src/server.rs` y el puente de datos existente para `/a2a`
- `docs/specs/2026-10-28-codeloom-agent-card-editor.md` para registrar decisiones finales y
  límites observados.

No tocar los cambios locales preexistentes fuera de los puntos necesarios; antes de editar
cualquier archivo ya modificado, inspeccionar y preservar su diff actual.

## Fuera de este plan

- Añadir transportes MCP HTTP o recarga en caliente; tienen specs propias.
- Cambiar el protocolo A2A, permitir editar tarjetas remotas o activar endpoints desde el
  editor.
- Hacer configurable la disponibilidad por defecto del supervisor.
- Reordenar, limpiar o revertir los demás cambios presentes en el árbol.

## Verificación y límites conocidos

`cargo check --workspace` terminó correctamente. `cargo fmt --all -- --check` reportó formato
pendiente en los cambios de esta implementación y en `crates/loomcoding-cli/src/server.rs`, que
ya tenía cambios locales. No se ejecutaron tests ni se hizo revisión visual en terminal real.
