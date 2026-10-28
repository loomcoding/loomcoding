# Plan — Editor de perfiles y tarjetas A2A

> Estado: pendiente de aprobación del plan; no iniciar implementación todavía.
>
> Spec: `docs/specs/2026-10-28-codeloom-agent-card-editor.md`.

## Objetivo

Editar perfiles locales desde `/agents` con secciones A2A y CodeLoom claramente separadas,
ver las tarjetas reales en `/a2a` y registrar al supervisor como agente local disponible por
defecto. No mezclar configuración MCP/local con el protocolo A2A.

## Decisiones de implementación propuestas

1. `name` sigue siendo el ID estable de CodeLoom y se proyecta como `AgentCard.name`; el
   editor muestra esa relación, sin mantener dos copias.
2. MCP y scope se pueden inspeccionar y editar en la sección CodeLoom. Como el parser actual
   los marca como no aplicados y este cambio no implementa un runtime MCP, se mostrarán
   claramente como **declarados, aún no aplicados**; no se afirmará que limitan acceso.
3. Las capacidades A2A se anuncian solo si el runtime las soporta. La edición de skills A2A
   no activa capacidades de transporte. Las capabilities de transporte no se habilitan por
   una casilla del perfil.
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
- `codeloom-tui/src/app.rs`, `run.rs`, `render.rs`: `/agents` ya lista y aprueba/revoca
  perfiles, pero solo muestra origen y prompt; sus eventos refrescan tarjetas remotas.
- `codeloom-cli/src/server.rs`: el supervisor ya responde por tenant `supervisor`, mantiene
  fallback sin LLM y tiene configuración opcional. Falta incorporarlo a la vista/directorio
  de tarjetas locales según el modelo existente.
- `card.rs` ya modela `AgentSkill` y `AgentCapabilities` A2A; los booleanos opcionales
  distinguen no declarado de falso.

## Fases

### 1. Perfil y proyección A2A

- Añadir al perfil los campos A2A editables que hoy no existen: lista `AgentSkill` y
  declaraciones permitidas de `AgentCapabilities`/extensiones.
- Mantener MCP, persona, modelo, herramientas y límites en el lado CodeLoom.
- Parsear y validar campos; conservar tolerancia a perfiles existentes y campos desconocidos.
- Proyectar solo nombre, descripción, versión, skills A2A y capacidades realmente
  soportadas. No proyectar `scope`, persona, provider ni secretos.
- Mantener advertencia visible para `scope` no aplicado.

### 2. Edición segura del perfil

- Añadir acción de editar a `/agents` para perfiles locales; los remotos quedan de solo
  lectura y el agente integrado no se convierte en fichero editable.
- Presentar dos secciones rotuladas: **A2A — ficha pública** y **CodeLoom — ejecución local**.
  Identidad compartida se explica como proyección; `AgentCard.skills` y `scope.skills` se
  presentan como listas diferentes.
- Reutilizar controles de texto/selección existentes, validar antes de guardar y conservar
  valores desconocidos y el cuerpo Markdown.
- Guardar atómicamente en la ruta de origen. Tras éxito, refrescar el registro; si cambia un
  agente de proyecto, su huella nueva deja la aprobación anterior inválida.
- Mostrar disponibilidad MCP separada de la declaración del scope; no indicar enforcement
  cuando no lo hay.

### 3. Supervisor y observatorio

- Incluir la tarjeta A2A del supervisor en el directorio local de `/a2a` con marca local
  `internal` fuera de `AgentCard`.
- Mostrarlo habilitado por defecto aunque falte configuración de provider/modelo.
- Derivar interfaces y capacidades de la implementación activa. No anunciar campos no
  soportados y mantener la tarjeta del supervisor no editable como perfil.
- Conservar `AgentManager::start_for` y su fallback sin LLM, sin añadir autorización ni
  acciones implícitas.

### 4. Revisión de límites

- Revisar que los campos A2A y CodeLoom se mantengan separados en perfil, proyección, preview
  y persistencia.
- Confirmar que el guardado inválido/no exitoso conserva fichero y entrada anterior.
- Confirmar que no se filtran secretos, prompts, transcript, rutas absolutas ni `internal` a
  la tarjeta A2A.

## Archivos previstos

- `crates/codeloom-core/src/registry/profile.rs`
- `crates/codeloom-core/src/registry/source.rs`
- `crates/codeloom-core/src/card.rs` (solo si la proyección requiere soporte ya modelado)
- `crates/codeloom-tui/src/app.rs`
- `crates/codeloom-tui/src/run.rs`
- `crates/codeloom-tui/src/render.rs`
- `crates/codeloom-cli/src/server.rs` y el puente de datos existente para `/a2a`
- `docs/specs/2026-10-28-codeloom-agent-card-editor.md` para registrar decisiones finales y
  límites observados.

No tocar los cambios locales preexistentes fuera de los puntos necesarios; antes de editar
cualquier archivo ya modificado, inspeccionar y preservar su diff actual.

## Fuera de este plan

- Implementar transporte o servidores MCP y enforcement de `scope.mcp`.
- Cambiar el protocolo A2A, permitir editar tarjetas remotas o activar endpoints desde el
  editor.
- Hacer configurable la disponibilidad por defecto del supervisor.
- Reordenar, limpiar o revertir los demás cambios presentes en el árbol.

## Criterio de cierre

La experiencia cumple los criterios de aceptación de la spec; el diff queda limitado a los
archivos previstos y los cambios locales anteriores permanecen intactos. La revisión visual
en terminal real y cualquier verificación ejecutable se informan según lo que se haya podido
hacer, sin presentarlas como realizadas si no lo fueron.
