# Editor de perfiles y tarjetas A2A

> Estado: implementado; pendiente de revisión final.
>
> Fecha: 2026-10-28
>
> Relacionado: `2026-10-22-codeloom-a2a-agent-registry-design.md`,
> `2026-10-27-codeloom-a2a-observatory.md`,
> `2026-10-27-codeloom-runtime-agent-cards.md`,
> `2026-10-27-codeloom-runtime-supervisor.md` y
> `2026-10-15-loom-agent-profile-a2a.md` y
> `2026-10-28-codeloom-mcp-agent-scope.md` (enforcement de `scope.mcp`).

## 1. Objetivo

Permitir revisar y editar desde la TUI los perfiles locales que generan tarjetas de agente,
mostrar el supervisor del runtime en el directorio A2A local y declarar sus capacidades
reales. La separación entre A2A y CodeLoom es un principio central: aunque compartan un
editor, cada dato debe tener una sección, etiqueta y destino de guardado inequívocos.

La edición opera sobre perfiles fuente (`.agents/agents/<name>.md` o el directorio global),
no sobre una copia desconectada de la tarjeta. La tarjeta A2A se sintetiza desde el perfil y
el estado real del runtime.

## 2. Principio: dos contratos en un editor

El editor presenta dos secciones visibles y separadas, con títulos y ayuda breve:

1. **A2A — tarjeta pública y protocolo:** describe al agente para otros sistemas y cómo
   comunicarse con él. Incluye identidad, skills y capacidades definidas por A2A, y las
   interfaces activas que ofrece.
2. **CodeLoom — configuración local de ejecución:** determina cómo ejecuta CodeLoom al
   agente, qué recursos locales puede usar y las reglas de confianza. Incluye MCP, skills
   cargadas localmente, herramientas, persona, modelo, límites y aprobación.

No se mezclan campos entre secciones aunque sus nombres cotidianos se parezcan. Cada campo
se escribe solo en su propiedad del perfil o se deriva del runtime; no se duplica en YAML
bajo una clave ambigua. La vista de tarjeta A2A consume únicamente la primera sección y los
datos A2A generados por el runtime. Los campos locales nunca se serializan como extensiones
inventadas ni se publican en la tarjeta.

Cuando una propiedad del perfil sirve de fuente para ambas vistas, la UI lo indica como una
proyección explícita y no crea una segunda copia. Por ejemplo, el `name` estable del perfil
CodeLoom se proyecta como `AgentCard.name`; el ID local y el nombre público comparten esa
fuente. En cambio, `scope.skills` de CodeLoom y `AgentCard.skills` de A2A son listas distintas
y se editan por separado.

## 3. Modelo y límites

### 3.1 A2A — tarjeta pública y protocolo

La sección A2A edita, con validación del esquema A2A:

- identidad pública: `name`, `description`, `version`;
- skills A2A (`AgentSkill`), con sus tags, ejemplos y modos de entrada/salida.

Las capacidades A2A del protocolo (`streaming`, `pushNotifications`, `extendedAgentCard` y
extensiones) se muestran en solo lectura y proceden del servicio que está activo. El perfil
no puede activarlas. Si el agente no expone interfaz A2A, la tarjeta muestra esas capacidades
como no declaradas.

### 3.2 CodeLoom — configuración local

La primera versión edita las declaraciones del scope local:

- asignación MCP (`scope.mcp`) y skills de proceso (`scope.skills`), usando IDs ya conocidos
  por los registros locales;
- herramientas locales declaradas (`scope.tools`). Persona, modelo, límites, origen y
  aprobación se conservan en el perfil pero no se editan desde esta pantalla.

Un agente local delegado solo recibe tools de servidores MCP que aparecen tanto en su perfil
como en la configuración global `~/.codeloom/mcp.toml`. Sin scope no recibe MCP. El runtime
actual soporta stdio; cada llamada MCP sigue pasando por el permiso normal. Los fallos e IDs
no disponibles aparecen como avisos locales en la TUI, sin un panel de estado por servidor.
`scope.skills` y `scope.tools` siguen declarados, pero todavía no se aplican.

### 3.3 Campos derivados o protegidos

- `supportedInterfaces` y tenant se generan desde interfaces A2A activas del runtime.
  No se pueden declarar manualmente si el agente no es realmente enrutable.
- `capabilities` solo se anuncian cuando la implementación efectiva las ofrece. Las
  declaraciones no activan por sí mismas streaming, notificaciones o endpoint extendido.
- El editor no muestra ni guarda secretos. Credenciales, rutas absolutas, transcript y
  prompt privado no pasan a la tarjeta pública.
- Los campos A2A desconocidos se preservan al guardar cuando sea posible; campos de protocolo
  desconocidos no se inventan ni se publican sin soporte.

### 3.4 Supervisor

El supervisor del runtime aparece siempre en el directorio local y queda habilitado por
defecto como agente disponible para solicitudes explícitas. Su registro y respuesta
determinista no dependen de un LLM. Sin provider/modelo, o ante error/timeout, el runtime
continúa sin razonamiento asistido.

La tarjeta del supervisor es generada por el runtime. Sus interfaces y capacidades reflejan
solo las funciones activas; la TUI la muestra, pero no permite editarla como perfil. La
configuración del LLM del supervisor sigue siendo independiente y opcional. «Habilitado» no
implica que el LLM esté configurado ni concede permisos para cambiar agentes o aprobar
acciones.

## 4. Interacción

- `/agents` lista perfiles locales y permite abrir el editor del seleccionado. El editor
  mantiene las secciones A2A y CodeLoom separadas de forma visible durante lectura y edición.
  Origen, aprobación del perfil de proyecto y disponibilidad MCP aparecen en la sección
  CodeLoom.
- Guardar valida el perfil completo antes de escribirlo. Si falla la validación o escritura,
  se conserva el archivo y la última versión válida del registro, y se informa del error.
- Tras guardar, la recarga existente actualiza el registro. Un agente de proyecto sigue
  requiriendo aprobación; cualquier cambio de contenido invalida su aprobación ligada a
  huella.
- `/a2a` muestra la tarjeta del supervisor y las tarjetas de agentes conocidas, separando
  campos públicos A2A de estado y política locales. El supervisor se distingue como
  `internal` mediante metadatos locales, nunca mediante un campo A2A inventado.
- En agentes remotos el editor es de solo lectura para los campos de su tarjeta obtenida
  remotamente. Las credenciales y el endpoint conservan sus controles actuales.

La edición usa un formulario pequeño sobre los controles existentes de la TUI; no requiere
un editor YAML genérico ni un editor de JSON arbitrario.

## 5. Seguridad y compatibilidad

- Aplicar la misma validación de `parse_profile` y las reglas de nombres del registro.
- Guardar de forma atómica para que un cierre durante escritura no deje un perfil truncado.
- Respetar precedencia proyecto > global > integrado y mostrar siempre el origen.
- El contenido de perfiles de proyecto continúa marcado como no confiable y sujeto a
  aprobación; editar no equivale a aprobar.
- No cambiar el esquema A2A ni publicar MCP, scope o marcas internas como campos propios.
- La separación A2A/CodeLoom se mantiene al editar, guardar, recargar y mostrar la tarjeta;
  ningún campo local se promociona silenciosamente a dato público.
- Clientes y perfiles existentes que no declaren capacidades conservan la semántica de
  «no declarado» (`None`); no se convierte en `false` ni en `true` por defecto.

## 6. Criterios de aceptación

1. Un perfil local válido puede editar sus datos A2A y sus datos CodeLoom en un mismo editor,
   pero en secciones visuales y propiedades claramente separadas; las proyecciones compartidas
   se indican y no se duplican.
2. Un perfil inválido o un error de escritura no reemplaza el fichero válido ni elimina la
   entrada vigente del registro.
3. La edición respeta precedencia, origen y aprobación de agentes de proyecto; modificar un
   perfil aprobado invalida la aprobación anterior.
4. Los datos A2A editados se reflejan en la tarjeta sintetizada; MCP, herramientas, persona,
   modelo, límites y aprobación permanecen en CodeLoom y no aparecen en la tarjeta.
5. Una capacidad no se anuncia por configurar un booleano si el runtime no soporta la
   función correspondiente.
6. El supervisor aparece siempre en el directorio local y está habilitado por defecto aunque
   falte el LLM; sus campos protegidos reflejan el runtime real.
7. Ninguna tarjeta incluye secretos, contenido de conversación ni rutas absolutas; la
   clasificación interna del supervisor permanece fuera de `AgentCard`.
8. Las tarjetas remotas se mantienen de solo lectura y no se confunden con perfiles locales.
9. En lectura, edición y tarjeta de vista previa queda claro qué pertenece a A2A y qué a
   CodeLoom; no hay campos con propiedad ambigua.

## 7. Fuera de alcance

- Implementar un supervisor MCP ni incorporar servidores MCP por esta pantalla.
- Hacer que un agente local sirva A2A entrante solo por editar su tarjeta.
- Editar libremente tarjetas remotas, interfaces activas, firmas o datos de seguridad.
- Cambiar cómo se aprueban perfiles, cómo se ejecutan permisos o cómo razona el LLM.
- Añadir un formato nuevo de perfil o un editor genérico de YAML/JSON.

## 8. Decisiones de implementación

1. `scope.mcp` es una allowlist efectiva para agentes locales delegados; `scope.skills` y
   `scope.tools` siguen declarados y no aplicados.
2. Las capacidades de transporte se derivan de la interfaz activa y son de solo lectura.

## 9. Nota de proceso

El plan ligado a esta spec está en `docs/plans/2026-10-28-codeloom-agent-card-editor.md`.
