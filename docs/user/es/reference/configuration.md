# Referencia de configuración (`loomcoding.toml`)

**Qué es**

Cada clave que Loomcoding lee del fichero de proyecto `loomcoding.toml`, y de dos ficheros del perfil, `providers.toml` y `runtime.toml`. Cada clave tiene su tipo, su valor por defecto y si un cambio requiere reiniciar. La página sigue al código, y no al revés.

**Para qué**

- Encontrar la clave que cambia un comportamiento, con su rango permitido.
- Consultar el valor por defecto de una clave que no has escrito.
- Saber qué cambios surten efecto al momento y cuáles requieren reiniciar.

**Cómo**

1. Abre `loomcoding.toml` en la raíz del proyecto. Loom lo crea en la primera apertura si falta.
2. Escribe solo las claves que quieras cambiar. Una clave que no escribas toma su valor por defecto.
3. Reinicia Loom para aplicar el cambio, salvo que la tabla indique otra cosa en las notas.

## Ejemplo

```toml
model = "gpt-4o-mini"
max_steps = 12
effort = "high"

[provider]
kind = "openai_compat"
base_url = "https://api.example.invalid/v1"
api_key_env = "EXAMPLE_API_KEY"

[limits]
temperature = 0.2

[agents]
confirm_delegation = "always"
```

## Cómo se leen los valores

- Loom lee `loomcoding.toml` al arrancar y al abrir el agente. Un cambio hecho a mano mientras Loom está en marcha no afecta a la sesión en curso. Por eso, cada clave de abajo requiere reiniciar.
- `/settings` guarda sus cambios y los aplica al momento. Al guardar, Loom vuelve a leer también `[provider] prompt_cache`, `request_timeout_secs` y `[limits]`.
- Un proyecto nuevo toma el proveedor y el modelo de la entrada por defecto de `providers.toml`, o de la primera entrada si no hay una por defecto. Su `max_steps` es 8.
- `/flow odd on` y `/flow odd off` guardan `[flows.odd] enabled`. El cambio se aplica al reiniciar.
- `/verbose` cambia solo la sesión. No escribe el fichero.
- Los servidores MCP se configuran en `mcp.toml` del perfil, no en este fichero.

## Nivel superior

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `model` | cadena | sin valor (obligatoria) | Sí | Identificador del modelo que se envía al proveedor. |
| `max_steps` | entero | 30 cuando la clave no aparece | Sí | Número máximo de pasos del agente en un turno. |
| `effort` | cadena | sin valor (el del modelo) | Sí | Esfuerzo de razonamiento. Usa uno de los niveles que ofrece el modelo; `/effort` los lista. |

## `[provider]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `kind` | enumeración | sin valor (obligatoria) | Sí | Tipo de API: `openai_compat`, `openai_responses`, `anthropic` o `gemini`. |
| `base_url` | cadena | sin valor (obligatoria) | Sí | URL base de la API. `/settings` acepta HTTPS, o HTTP para localhost, y rechaza credenciales dentro de la URL. |
| `api_key_env` | cadena | vacía | Sí | Nombre de la variable de entorno que guarda la clave. Vacía para un servidor local que no necesita clave. |
| `prompt_cache` | booleano | sin valor (activada) | Sí | Puntos de caché de prompts del adaptador de Anthropic. `LOOMCODING_PROMPT_CACHE` la sustituye. |
| `context_window` | entero, mayor que 0 | sin valor (decide el catálogo del modelo) | Sí | Tokens por petición que el servidor procesa dentro del plazo. |
| `request_timeout_secs` | entero, de 1 a 3600 | 120 | Sí | Tiempo total de cada petición en el adaptador compatible con OpenAI. |
| `efforts` | lista de cadenas, como máximo 16 | vacía (decide el catálogo del modelo) | Sí | Niveles de esfuerzo que admite un modelo local, en el orden en que la interfaz los muestra. Ejemplo: `["none", "low", "xhigh"]`. |

Búsqueda de la clave para `api_key_env`: el entorno del proceso, después el fichero `.env` del proyecto, y después el almacén de credenciales del sistema con el mismo nombre. Loom nunca escribe la clave en el fichero.

## `[limits]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `max_read_steps` | entero | sin valor (4 veces `max_steps`, como mínimo 1) | Sí | Número máximo de pasos de lectura en una respuesta del modelo. |
| `max_parallel_tools` | entero | 8 (0 equivale a 8) | Sí | Número máximo de llamadas paralelas a herramientas en una respuesta del modelo. |
| `max_concurrent_requests` | entero | 4 (0 equivale a 4) | Sí | Número máximo de peticiones simultáneas al proveedor. |
| `max_output_tokens` | entero | 4096 (0 equivale a 4096) | Sí | Límite de tokens de salida de cada petición, razonamiento incluido. Tope de 32768. |
| `temperature` | número, de 0.0 a 2.0 | 0.0 | Sí | Temperatura de muestreo. Un valor fuera del rango es un error. |

## `[agents]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `global_dir` | ruta | `~/.agents/agents` | Sí | Carpeta de agentes globales. Un `~/` inicial se expande a la carpeta personal. |
| `project_dir` | ruta | `.agents/agents` dentro del proyecto | Sí | Carpeta de agentes del proyecto. Una ruta relativa parte de la raíz del proyecto. |
| `trust_project_agents` | booleano | false | Sí | Aprueba todos los agentes del proyecto sin preguntar. |
| `confirm_delegation` | enumeración | `doubt` | Sí | Cuándo se pregunta antes de delegar. `doubt` pregunta solo si encajan varios agentes; `always` pregunta siempre; `never` no pregunta. |
| `routing` | enumeración | `off` | Sí | Elección automática de agente. `off` no elige; `rule` usa las etiquetas de las tarjetas, sin llamar a un modelo; `classifier` aplica primero la regla y después un modelo barato. |
| `routing_model` | cadena | el modelo del padre | Sí | Modelo del clasificador. Solo se usa con `routing = "classifier"`. |
| `delegation` | enumeración | sin valor (el comportamiento actual) | Sí | `off` quita las herramientas de subagentes al modelo. `on_request` delega solo si lo pides. `proactive` delega las subtareas independientes. Solo `off` lo garantiza el código; los otros dos son reglas del prompt. |
| `max_active_subagents` | entero, de 1 a 64 | 8 | Sí | Número de subagentes que corren a la vez. |
| `max_depth` | entero, de 1 a 3 | 1 | Sí | Profundidad de la delegación. 1 significa sin delegación anidada. Los valores fuera de rango se ajustan al límite. |
| `stall_threshold_secs` | entero, de 10 a 3600 | 120 | Sí | Segundos sin latido antes de que un subagente se marque como atascado. |
| `lost_threshold_secs` | entero, de 30 a 21600 | 300 | Sí | Segundos sin latido antes de que un subagente se marque como perdido. Un valor menor sube al umbral de atasco más 30 segundos. |
| `lost_action` | enumeración | `notify` | Sí | Qué pasa con un subagente perdido. `notify` solo lo comunica; `fail` marca su tarea como fallida y detiene su trabajo. |
| `catalog_roots` | lista de rutas, como máximo 8 | vacía | Sí | Raíces adicionales con `.agents/agents` y `.agents/skills`. Sus agentes vienen de terceros, así que necesitan aprobación. |

## `[a2a]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `role` | enumeración | `off` | Sí | `off` desactiva A2A. `serve` expone agentes locales a otros clientes, en modo de solo lectura. `consume` llama a agentes remotos que hayas aprobado. `both` hace ambas cosas. |
| `request_timeout_secs` | entero, de 1 a 600 | 30 | Sí | Plazo de cada petición HTTP a un agente remoto. |
| `max_card_bytes` | entero, de 1 a 8388608 | 65536 | Sí | Tamaño máximo de una tarjeta de agente remoto. |
| `max_result_bytes` | entero, de 1 a 8388608 | 262144 | Sí | Tamaño máximo del cuerpo de una respuesta remota. |
| `card_refresh_secs` | entero | 3600 | Sí | Segundos entre descargas de las tarjetas. 0 desactiva la actualización. Los valores por debajo de 60 pasan a 60. |
| `ca_file` | ruta | sin valor | Sí | Fichero PEM con autoridades de certificación adicionales para los agentes remotos. Se suma a las raíces públicas. |
| `trusted_jwks` | ruta | sin valor | Sí | Fichero JWKS con las claves que firman las tarjetas. Si se define, toda tarjeta remota necesita una firma válida. |

## `[a2a.serve]`

Solo se usa cuando `[a2a] role` es `serve` o `both`. Una clave desconocida en esta sección es un error.

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `listen` | cadena | `127.0.0.1:7077` | Sí | Dirección en la que escucha el servidor A2A local. |
| `agent` | cadena | `default` | Sí | Nombre del agente local que se ofrece a los clientes. Hasta 200 caracteres. |
| `auth` | tabla | sin valor | Sí | Autenticación de los clientes. Consulta `[a2a.serve.auth]`. |
| `public_url` | cadena | sin valor | Sí | Dirección pública que se anuncia en la tarjeta del agente. Se valida con las mismas reglas que `base_url`. |
| `max_concurrent_tasks` | entero, de 1 a 64 | 4 | Sí | Número de tareas que se atienden a la vez. |
| `max_wait_secs` | entero, de 1 a 300 | 30 | Sí | Límite superior, en segundos, para esperar una tarea. |
| `max_task_secs` | entero, de 1 a 14400 | 600 | Sí | Límite superior, en segundos, para una tarea. |

## `[a2a.serve.auth]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `scheme` | enumeración | sin valor (obligatoria con `auth`) | Sí | Esquema de autenticación: `bearer` o `apikey`. |
| `name` | cadena | sin valor | Sí | Nombre de la cabecera de la clave. Solo se usa con `apikey`. |
| `env` | cadena | sin valor (obligatoria) | Sí | Variable de entorno que guarda el secreto. El secreto nunca se escribe en el fichero. |

## `[ui]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `verbose_provider_notices` | booleano | false | Sí | Muestra en la transcripción los avisos de inicio y fin de cada petición al proveedor. Sin él, mientras corre una petición aparece un indicador de carga. |

## `[compaction]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `background` | booleano | false | Sí | Genera el resumen en segundo plano cuando el contexto se acerca a su límite. Apagada, usa la vía síncrona. |

## `[budget]`

Los avisos aparecen al 80 % y al 100 % de cada límite.

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `session_tokens` | entero, mayor que 0 | sin valor | Sí | Límite de tokens de entrada más salida de la sesión. |
| `session_cost_usd` | número, mayor que 0 | sin valor | Sí | Límite en dólares estadounidenses para la sesión, según precios conocidos. |
| `hard_stop` | booleano | false | Sí | Si es true, el agente deja de pedir respuestas al modelo cuando la sesión llega al 100 % de un límite. Si no, el presupuesto solo avisa. |

## `[output_filter]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `enabled` | booleano | false | Sí | Compacta la salida de `bash` y `job_output` con reglas para cargo, git y gestores de paquetes. La salida íntegra se conserva en `tool-output`. `LOOMCODING_NO_FILTER=1` lo desactiva. |

## `[flows.<kind>]`

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `enabled` | booleano | true para `odd` | Sí | Activa o desactiva un modo de flujo. Hoy solo existe `odd`. Un kind tiene de 1 a 32 caracteres: letras minúsculas, dígitos, `_` o `-`. |

## `providers.toml`

Este fichero vive en la carpeta del perfil, no en el proyecto. Loom lo crea en su primer uso con tres entradas: `llama` (un `llama-server` local), `strata` y `polystrata`. Las claves nunca se guardan aquí, solo el nombre de la variable que las contiene.

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `default` | cadena | `llama` en el fichero inicial | Sí | Identificador de la entrada de la que toma sus ajustes iniciales un proyecto nuevo. |
| `id` | cadena | sin valor (obligatorio) | Sí | Nombre corto que escribes en `/provider`. Un identificador repetido se ignora; gana la primera entrada. |
| `name` | cadena | sin valor (obligatorio) | Sí | Etiqueta que se muestra en la interfaz. |
| `kind` | cadena | sin valor (obligatorio) | Sí | Tipo de API. Hoy solo se acepta `openai_compat`. |
| `base_url` | cadena | sin valor (obligatoria) | Sí | URL base, con `/v1`. |
| `api_key_env` | cadena | sin valor | Sí | Variable que guarda la clave, si el servidor la necesita. |
| `model` | cadena | sin valor (obligatorio) | Sí | Modelo de reserva, que se usa cuando el servidor no responde a la sonda. |
| `context_window` | entero | sin valor | Sí | Ventana de contexto declarada. |

## `runtime.toml`

Este fichero opcional vive en la carpeta del perfil. Loom no lo crea. El servidor local lo lee al arrancar, así que un cambio requiere reiniciar ese servidor. Contiene una sección `[a2a]` con las mismas claves de arriba, y una sección `[supervisor]`.

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `supervisor` | tabla | sin valor | Sí | Arranca el agente supervisor, con los ajustes de la sección siguiente. |

## `[supervisor]` en `runtime.toml`

El supervisor responde con un plazo de 15 segundos, fijado en el código. Si sus ajustes no se pueden usar, no arranca ningún supervisor.

| Clave | Tipo | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|---|
| `provider` | tabla | sin valor (obligatoria) | Sí | Proveedor del supervisor, con las claves `kind`, `base_url` y `api_key_env` de `[provider]`. |
| `model` | cadena | sin valor (obligatorio) | Sí | Modelo del supervisor. |

## Siguientes pasos

- [Variables de entorno](environment.md)
- [Carpetas del perfil](profile-folders.md)
- [Comandos](commands.md)
- [ODD y flujos (`/flow`)](../concepts/flows-odd.md)
- [A2A](../concepts/a2a.md)
