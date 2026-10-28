# Llamadas programáticas de herramientas por provider

> Estado: propuesta para revisión; no implementar hasta aprobar alcance y decisiones abiertas.
>
> Fecha: 2026-10-28
>
> Relacionado: `2026-10-13-loom-design.md` (§4, Provider y bucle de tools),
> `2026-10-14-loom-mcp-skills-hot-reload.md` y las implementaciones en
> `crates/codeloom-core/src/provider/` y `crates/codeloom-core/src/agent/`.
>
> Referencias de proveedores, consultadas el 2026-10-28:
> [Anthropic: Programmatic tool calling](https://platform.claude.com/docs/es/agents-and-tools/tool-use/programmatic-tool-calling),
> [OpenAI: Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling),
> [Google: Code execution](https://ai.google.dev/gemini-api/docs/code-execution) y
> [Google: combinar herramientas](https://ai.google.dev/gemini-api/docs/tool-combination).
> Notas de contraste local: `docs/references/2026-10-28-programmatic-tool-calling-providers.md`.

## 1. Objetivo

Permitir que un modelo coordine varias llamadas a herramientas mediante código ejecutado
dentro de un entorno del provider. El modelo puede usar bucles, condiciones y procesamiento
intermedio, y devolver a su contexto solo el resultado resumido. La meta es reducir viajes al
modelo, tokens y datos intermedios en tareas que requieren varias consultas.

La hipótesis de producto es que menos viajes y menos resultados intermedios en contexto
reducirán latencia y coste y podrían mejorar algunas respuestas al reservar contexto para la
decisión final. No se considera garantía: el código generado puede ser incorrecto y el
resultado puede ser peor. El efecto se debe medir por provider/modelo y tipo de tarea.

CodeLoom ofrece una única idea de producto —**orquestación programática de herramientas**—
pero no presupone un protocolo común. Cada adapter declara y traduce las funciones nativas
que soporta. La función se determina por la combinación **provider + modelo + API/endpoint +
herramientas elegibles**; no por el nombre del provider solamente.

## 2. Qué significa y qué no

Una llamada de herramienta normal pide al modelo una tool call, CodeLoom la ejecuta y luego
envía el resultado al modelo. La llamada programática añade una ejecución de código que puede
invocar una o más herramientas y procesar los resultados entre llamadas, sin pedir al modelo
una nueva decisión después de cada una.

Esto no es una capacidad de A2A, una propiedad de `AgentCard`, ni una garantía general de
que «el modelo ejecuta tools». Es una extensión específica de la API del proveedor, con su
propio entorno, formato de mensajes, catálogo de herramientas permitidas y continuidad.
Function calling estándar y tool calls en paralelo tampoco implican soporte programático.

## 3. Evidencia por provider

| Provider | Función documentada | Diferencia relevante para CodeLoom |
|---|---|---|
| Anthropic | `code_execution` con `allowed_callers` para permitir llamadas a herramientas desde el contenedor; `caller` enlaza cada llamada con la ejecución que la originó. Requiere versión compatible de la herramienta y un modelo que lo admita. | La respuesta puede pausar el código esperando `tool_result`; hay que continuar con el contenedor y el mismo conjunto de tools. Las herramientas de MCP Connector no se pueden llamar programáticamente. |
| OpenAI | Programmatic Tool Calling ejecuta JavaScript en un runtime V8 hospedado; tools habilitadas declaran `allowed_callers: ["programmatic"]`. La API Responses y Agents tienen contratos distintos. | El adapter actual de CodeLoom usa compatibilidad OpenAI Chat Completions; no equivale a la API Responses ni demuestra soporte de esta función. No declarar soporte en un endpoint compatible sin evidencia explícita. |
| Google Gemini | Code Execution permite ejecutar Python; determinadas versiones/modelos combinan esa herramienta integrada con function calling. | Es un flujo nativo diferente; no comparte `allowed_callers`, `caller` ni el protocolo de continuación de Anthropic. Debe tener adapter y soporte verificado propios. |

El inventario anterior es evidencia de que hay mecanismos parecidos en más de un proveedor,
no prueba de paridad ni de disponibilidad para todos sus modelos, endpoints, regiones o
planes. CodeLoom no anuncia soporte si el adapter no puede confirmarlo.

## 4. Contrato de CodeLoom

### 4.1 Capacidad consultable

La capa de Provider debe poder responder, para el modelo y la petición concretos, si se
admite orquestación programática, qué protocolo usa, qué tools son elegibles y qué
restricciones aplica. El contrato actual `Provider::capabilities()` solo declara
`tool_use`, `reasoning` y `streaming`, sin modelo ni conjunto de tools; no basta para inferir
esta capacidad.

La respuesta de capacidad debe distinguir al menos:

- **soportada**: adapter y modelo pueden usarla en esta petición;
- **no soportada**: se usa el bucle normal de CodeLoom;
- **desconocida/no verificable**: se usa el bucle normal y se informa sin adivinar.

No se fija aquí un enum ni una abstracción para homogeneizar formatos incompatibles. Cada
adapter conserva sus campos de protocolo, su parser de eventos y su forma de continuar el
turno.

### 4.2 Selección y fallback

- La ejecución programática es opt-in por configuración de CodeLoom o por petición; queda
  apagada si no hay decisión explícita.
- CodeLoom resuelve la capacidad antes de construir la solicitud y selecciona solo tools
  compatibles con esa vía.
- Si el modelo, endpoint o una tool no admite esta vía, CodeLoom mantiene el flujo actual de
  llamadas directas. La razón queda observable en la TUI/log, sin fallar el turno.
- La ruta normal sigue disponible para tools que solo pueden invocarse directamente.
- El comportamiento nunca cambia por un fallback silencioso que amplíe el conjunto de
  herramientas o sus permisos.

### 4.3 Ejecución y continuidad

- El código generado por el modelo se ejecuta únicamente en el entorno aislado del provider
  que ofrece la función. CodeLoom no ejecuta ese código en el host ni lo convierte en una
  shell local.
- Las llamadas a tools propiedad de CodeLoom regresan a CodeLoom para aplicar validación,
  `PermissionService`, `ExecutionMode` y registro de eventos. El código orquestador no
  aprueba ni sortea esas puertas.
- La respuesta conserva la relación entre invocación de código, tool calls y resultados,
  usando IDs del provider. El adapter conserva el estado opaco de continuación que requiera
  ese proveedor (por ejemplo, el contenedor Anthropic) sin meterlo en contenido visible al
  modelo como texto fabricado.
- Un resultado denegado se entrega como error de herramienta a la ejecución pendiente; no
  se transforma en aprobación. Si el protocolo requiere responder solo con resultados de
  herramientas pendientes, el adapter debe respetar esa forma exactamente.

## 5. Seguridad, límites y observabilidad

- La allowlist de herramientas programáticas se calcula en el runtime. Ninguna capacidad
  provider autoriza por sí sola herramientas.
- Cada llamada conserva las validaciones y permisos actuales. Las tools de escritura siguen
  siendo `Exclusive`; el runtime no ejecuta llamadas exclusivas concurrentemente aunque el
  código generado las solicite a la vez.
- La ejecución de código tiene límites explícitos de tiempo, número total de tool calls,
  concurrencia, bytes de entrada/salida y consumo del turno. No hay bucles ilimitados ni
  resultados intermedios ilimitados.
- Las respuestas de herramienta se acotan antes de entregarlas al entorno del provider; el
  resultado de la orquestación se acota antes de volver al modelo.
- IDs de contenedor/sesión del proveedor se tratan como datos opacos y se aíslan por sesión
  y provider. No se guardan credenciales en los mensajes.
- La observabilidad distingue tool calls directas de llamadas hechas desde código e incluye
  el provider, modelo, protocolo, relación de IDs, resultado, duración y denegaciones. No
  guarda el código generado ni contenido sensible por defecto.
- Errores, timeout, expiración o pérdida del estado de continuación terminan la ejecución
  con resultado de error y permiten que el loop siga por su ruta acotada o cierre el turno;
  no repiten ejecución con efectos laterales sin una nueva decisión del modelo.

## 6. Integración actual y etapas

Hoy `ProviderRequest` envía `ToolSpec { name, description, input_schema }`; los adapters
Anthropic y OpenAI Compat mapean tool calls ordinarias a `StreamChunk::ToolCallComplete`.
`AgentLoop` pide permisos y luego ejecuta el lote por el `ToolRegistry`. No hay modelo de
llamadas hechas desde código, eventos de ejecución de código, estado de continuación ni
capacidad específica por modelo. La implementación requiere cambios coordinados en adapter,
loop y observabilidad; no basta con añadir un campo a `ToolSpec`.

### Medición de la hipótesis

Comparar la ruta directa y la programática en tareas equivalentes por provider/modelo y
categoría de tools. Registrar los tokens de entrada/salida informados por el provider, tiempo
total y tiempo en provider, número de decisiones del modelo, bytes que vuelven al contexto,
resultado de la tarea y errores/denegaciones. La calidad se evalúa con criterios de tarea
predefinidos; menos tokens o menor latencia no se presentan como mayor calidad por sí solos.
La comparación debe separar modelos, APIs y configuraciones: un resultado de Anthropic no se
transfiere a OpenAI Responses ni Gemini.

Propuesta de fases:

1. **Contrato y adapter Anthropic:** soportar el flujo Anthropic documentado para un modelo y
   versión de `code_execution` verificados, con allowlist explícita, pausa/continuación y
   estado opaco del contenedor.
2. **Integración del loop:** enrutar llamadas programáticas por los permisos y ejecución
   existentes, mantener las barreras `Exclusive` y acotar ciclos y salidas.
3. **Observabilidad y fallback:** distinguir el modo de invocación y cubrir modelo no
   compatible, tool no elegible, denegación, error y expiración.
4. **Adapters futuros:** evaluar OpenAI Responses y Gemini por separado cuando CodeLoom tenga
   esos adapters/endpoints. Un adapter compatible OpenAI no hereda PTC automáticamente.

Las fases no autorizan a activar el modo por defecto. La fecha exacta de disponibilidad por
modelo/API se consulta o se configura por provider; no se fija una lista permanente de
modelos en el núcleo.

## 7. Fuera de alcance

- Ejecutar código generado en el equipo local o implementar un sandbox local.
- Prometer una API común de proveedores o modificar A2A para representar esta función.
- Tratar el code interpreter de un proveedor como acceso a shell, archivos locales, red o
  MCP de CodeLoom.
- Habilitar MCP programático en Anthropic: la documentación consultada excluye herramientas
  provistas por MCP Connector; cualquier integración MCP local requiere su propio diseño de
  elegibilidad y permisos.
- Añadir provider OpenAI Responses o Gemini como parte del primer adapter Anthropic.

## 8. Criterios de aceptación

1. Un provider/modelo/API sin soporte confirmado usa el loop actual sin perder ninguna tool
   directa ni cambiar permisos.
2. El adapter no declara soporte por provider genérico: la capacidad depende del modelo, API
   y tools de la petición.
3. Las llamadas programáticas conservan IDs y continuidad del protocolo nativo, y sus
   tools pasan por validación, permisos y barreras de ejecución de CodeLoom.
4. Una denegación, timeout o error no se convierte en aprobación ni produce un reintento
   automático de una tool con posibles efectos laterales.
5. El número de invocaciones, tiempo, concurrencia y bytes están acotados.
6. La TUI distingue llamadas directas y programáticas sin mostrar código/credenciales por
   defecto.
7. Los límites y compatibilidades observados se documentan por adapter; OpenAI Compat no se
   presenta como OpenAI Responses API.
8. La evaluación registra tokens, latencia, contexto devuelto y calidad por provider/modelo;
   cualquier mejora de calidad se reporta solo si la métrica de tarea la confirma.

## 9. Preguntas para aprobar antes del plan

1. ¿El primer corte debe ser Anthropic nativo solamente, dejando OpenAI Responses y Gemini
   como adapters futuros?
2. ¿El modo debe poder habilitarse por agente/perfil, por sesión, o solo en configuración
   global al principio?
3. ¿Se aprueba el diseño estrictamente basado en sandbox alojado por el provider, sin código
   generado ejecutado localmente?

## 10. Nota de proceso

Esta spec es el diseño revisable. Después de acordar las decisiones se prepara un plan de
implementación con puntos concretos de integración, migración del protocolo de conversación,
permisos y verificación. La aprobación del plan precede al código.
