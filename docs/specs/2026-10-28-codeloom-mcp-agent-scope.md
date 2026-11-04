# Aplicación de `scope.mcp` por agente

> Estado: primera versión stdio implementada; revisión visual pendiente.
>
> Fecha: 2026-10-28
>
> Relacionado: `2026-10-14-loom-mcp-skills-hot-reload.md`,
> `2026-10-14-loom-tool-skill-discovery.md`,
> `2026-10-15-loom-agent-profile-a2a.md`,
> `2026-10-28-codeloom-agent-card-editor.md` y
> `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.

## 1. Objetivo

Hacer efectiva la declaración local `scope.mcp` de cada perfil: el agente solo puede
descubrir y llamar herramientas pertenecientes a servidores MCP que su perfil autoriza y
que Loomcoding tenga configurados y disponibles.

`scope.mcp` es una regla propia de Loomcoding. No es un campo de A2A ni una capacidad del
protocolo MCP. Configurar el nombre de un servidor en el perfil no lo instala, no lo inicia
y no concede permiso para ejecutar su proceso.

La primera versión añade un cliente MCP stdio en el crate opcional `loomcoding-mcp`. Los
servidores se configuran en `~/.loomcoding/mcp.toml`, que es global del usuario; perfiles de
proyecto solo pueden referenciar sus IDs. La spec existente de hot-reload sigue siendo el
diseño para recarga y otros transportes, que aún no están incluidos aquí.

## 2. Modelo de autorización

Hay tres decisiones separadas:

1. **Configuración de Loomcoding:** qué servidores MCP conoce el runtime y cuáles fueron
   aprobados para iniciarse.
2. **Scope del perfil:** cuáles de esos IDs puede usar este agente (`scope.mcp`).
3. **Permiso de la llamada:** si la persona usuaria debe aprobar una operación concreta,
   según el gate de permisos de herramientas.

La autorización efectiva es la intersección entre servidores configurados en el fichero del
usuario y conectados, y los IDs declarados por el perfil. Editar ese fichero constituye la
configuración explícita del usuario; `scope.mcp` nunca aumenta los permisos de
la herramienta ni reemplaza la aprobación por riesgo.

```text
servidores MCP configurados por el usuario
                    ∩
            perfil.scope.mcp
                    ↓
        tools visibles y ejecutables
                    ↓
         gate normal de permisos
```

## 3. Reglas de `scope.mcp`

- Los valores son IDs estables de `[mcp.servers.<id>]`, no nombres de tools ni URLs.
- Una lista vacía o ausente concede acceso a **ningún** servidor MCP. Es una decisión
  fail-closed; nunca significa «todos».
- Un ID desconocido, desconectado o sin tools no habilita ningún recurso ni causa fallback
  a otros servidores. Los fallos de configuración/conexión se registran como avisos.
- Los servidores MCP no enumerados quedan fuera del snapshot de tools de ese agente.
- La regla se aplica a agentes locales cargados desde perfiles. Los agentes remotos A2A
  conservan su política en su propio host; el scope local nunca se envía al remoto.
- Sin perfil local asociado (incluido el agente raíz si se ejecuta sin perfil), no se
  conceden tools MCP por defecto.
- `scope.tools` y `scope.skills` siguen fuera de este alcance; esta spec no cambia qué
  herramientas integradas o skills puede usar un agente.

## 4. Integración con el registro de herramientas

Loomcoding debe mantener el ID del servidor como parte de la identidad interna de cada tool
MCP. Dos servidores pueden publicar tools con el mismo nombre; sus identidades internas no
deben colisionar. El nombre presentado al provider se genera de forma determinista y se
resuelve al servidor y al nombre MCP originales.

Al preparar un agente local:

1. capturar una instantánea de configuración, perfiles y tools disponibles;
2. seleccionar solo tools de servidores permitidos por `scope.mcp`;
3. construir las definiciones visibles para el provider desde esa selección;
4. conservar la misma selección para resolver las llamadas de ese step.

La dispatch resuelve la llamada contra el registro acotado que recibió ese subagente. No basta
con ocultar la definición al modelo: una tool inventada, manipulada o procedente de una
respuesta tardía se rechaza como desconocida, sin enviar una llamada al servidor MCP.

Los tools MCP pasan por el gate normal de permisos. Conectarlos o incluirlos en el scope no
equivale a aprobar una acción. Ningún cambio de configuración ni `list_changed` autoaprueba
una tool.

## 5. Servidores y confianza

- Los perfiles solo pueden referenciar IDs existentes en `~/.loomcoding/mcp.toml`.
- Solo se soporta transporte `stdio` en esta entrega. El fichero global del usuario contiene
  el comando y sus argumentos. Un perfil, incluido uno de proyecto, nunca puede ordenar por
  sí solo que Loomcoding ejecute un comando.
- El proceso hijo recibe un entorno limpio, `PATH` y variables base del sistema; otras
  variables solo se pasan mediante el mapa `env` que referencia nombres presentes en el
  entorno del proceso Loomcoding. No se leen secretos del `.env` del proyecto.
- Secretos de transporte se resuelven desde los mecanismos de credenciales de Loomcoding; no
  se copian al perfil, prompt, tarjeta A2A, errores ni eventos.
- El contenido recibido del servidor se trata como salida de herramienta no confiable y
  respeta el límite de salida común del Decálogo de contexto y coste.
- Una herramienta MCP conserva su origen (`mcp:<server-id>`) para permisos y diagnóstico.

## 6. Cambios durante una sesión

La selección se fija al crear cada subagente:

- el runtime carga el fichero MCP al crear el agente de proyecto; cambios del fichero MCP
  requieren reiniciar/reconectar ese runtime;
- cada delegación toma el scope de la instantánea vigente del perfil;
- un subagente mantiene el scope con el que fue creado; las llamadas MCP en curso no se
  revocan retroactivamente;
- `tools/list_changed` y la publicación de nuevas generaciones quedan para la integración
  hot-reload especificada aparte;
- un fichero inválido se rechaza y no carga servidores; el agente arranca sin tools MCP.

Cambiar la lista en el editor no modifica retroactivamente una llamada ya iniciada. En un
perfil de proyecto, modificar el archivo cambia su huella y requiere la aprobación existente
antes de volver a ejecutar ese perfil.

## 7. Configuración ilustrativa

La configuración global del usuario usa esta forma mínima:

```toml
[mcp.servers.serena]
command = "uvx"
args = ["mcp-server-serena"]
env = { SERENA_TOKEN = "SERENA_TOKEN" }
```

```yaml
scope:
  mcp: [serena]
```

El perfil no contiene comandos, endpoints, cabeceras ni valores secretos. El valor de
`SERENA_TOKEN` se lee del entorno del proceso Loomcoding y no se serializa ni registra.

## 8. Errores y comportamiento visible

| Situación | Comportamiento |
|---|---|
| `scope.mcp` ausente o vacío | Ninguna tool MCP disponible para ese agente |
| ID del scope desconocido | No se carga; mostrar aviso con el ID, sin fallback |
| Configuración MCP inválida | No se inicia ningún servidor MCP; aviso local en la TUI |
| Servidor stdio desconectado o fallido | Sus tools no se ofrecen; aviso local en la TUI |
| Llamada a tool MCP fuera de scope | Rechazo local; no invocar el servidor ni gastar otro step de modelo |
| Nombre duplicado entre servidores | Exponer identidades namespaced; nunca elegir un servidor por orden |
| Scope/config inválidos | Conservar el último snapshot válido; nunca abrir el acceso |

El editor muestra el scope declarado como «servidores MCP permitidos». Los IDs desconocidos
y fallos de arranque aparecen como avisos locales de la TUI; todavía no hay un panel con la
lista y estado de cada servidor.

## 9. Criterios de aceptación

1. Un perfil con `scope.mcp: [serena]` solo recibe tools del servidor `serena`, si existe,
   está aprobado y conectado.
2. Un perfil sin scope o con lista vacía no recibe tools MCP.
3. Un servidor presente en configuración pero ausente del scope no aparece en las
   definiciones del provider y no se puede invocar mediante dispatch.
4. Una llamada fuera de scope se rechaza antes de contactar el servidor.
5. Dos servidores que publican el mismo nombre no colisionan ni cruzan la llamada.
6. Agregar un ID al scope no instala, ejecuta ni aprueba el servidor.
7. Los permisos de tools, aislamiento de agentes, límites de salida y credenciales siguen
   aplicándose; no se filtran secretos ni scopes a A2A.
8. Una actualización de scope afecta los nuevos steps de forma consistente y no revoca de
   manera insegura una llamada ya iniciada.
9. El editor separa la lista permitida de la configuración global; los errores de conexión
   se muestran como avisos locales sin incluir valores secretos ni conceder acceso alternativo.

## 10. Pruebas requeridas al implementar

- Unitarias: intersección de configuración y scope; ausencia de scope; IDs desconocidos;
  namespacing y colisiones; rechazo de dispatch fuera de scope.
- Integración con servidor MCP falso: solo se invocan las tools permitidas; llamadas no
  permitidas no llegan al servidor; errores y desconexiones no abren acceso.
- Cambio de perfil: las delegaciones nuevas usan el scope actualizado; las existentes
  conservan su snapshot.
- Seguridad: config de proyecto no inicia procesos MCP sin aprobación; secretos ausentes de
  logs, errores, prompts y tarjetas.
- Sin red real en tests; usar el servidor MCP de prueba/mock conforme a las reglas del repo.

## 11. Fuera de alcance

- Diseñar o reemplazar el protocolo MCP, OAuth o transportes; usar la spec del runtime MCP.
- Implementar allowlists para `scope.tools` o `scope.skills`.
- Enviar el scope al modelo como autoridad o propagarlo a agentes A2A remotos.
- Introducir descubrimiento dinámico que conceda acceso automáticamente.
- Hacer que una aprobación de servidor MCP apruebe también sus llamadas de herramientas.

## 12. Relación con el diseño existente

Esta spec concreta el enforcement de `scope.mcp` descrito como allowlist en
`2026-10-15-loom-agent-profile-a2a.md` §3. Los transportes HTTP y la recarga deben seguir el
diseño de `2026-10-14-loom-mcp-skills-hot-reload.md`; no se debe añadir un supervisor paralelo.
La integración del editor de perfiles está en `2026-10-28-codeloom-agent-card-editor.md`.
